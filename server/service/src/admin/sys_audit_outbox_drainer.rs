//! 042 audit-outbox-and-http-mount: 背景 drainer 消化 sys_audit_outbox 寫入 sys_operation_log
//! + publish Redis Stream。
//!
//! per spec FR-002/FR-010/FR-011 + quickstart §4.2 + research R-10：
//! - `SELECT FOR UPDATE SKIP LOCKED LIMIT N` 多 replica 安全（W-F11 場景）
//! - 一輪 batch 完一輪 sleep（empty batch sleep N ms、非空立刻下一輪）
//! - row error → retry_count++ / last_error；retry_count 達 max 後 row 留作 dead-letter
//! - publish_audit_event 為 fire-and-forget（自身失敗 warn、不阻 row 標 published）
//!
//! ## 為何 per-row txn 而非 outer batch txn（correctness 設計選擇）
//!
//! quickstart 範例使用單一 outer txn 處理 batch；但若任一 row 處理失敗、outer txn rollback
//! 會連帶撤銷 retry_count++ UPDATE，導致 retry 計次無法遞增 → infinite retry loop。
//!
//! 改採 per-row txn pattern：
//! 1. **fetch txn**: 短暫 begin → SELECT FOR UPDATE SKIP LOCKED LIMIT N → 收 row 後立即 commit
//!    （釋放 lock；下一輪可由本或他 replica 重新 SELECT 同 batch）。
//! 2. **per-row txn**（對每個 fetched row）：
//!    - publish Redis（在 txn 外、無 rollback 概念）
//!    - begin → re-lock single row WITH `FOR UPDATE` → 讀回 published_at
//!      → 若 `published_at IS NOT NULL` 表他 replica 已處理、rollback skip
//!      → 否則 INSERT sys_operation_log → UPDATE published_at=NOW() → commit
//!    - 失敗：rollback；另開 mini-txn UPDATE retry_count++ + last_error → commit
//!
//! ## 對 W-F11 多 replica 安全性的影響
//!
//! - fetch 階段：SELECT FOR UPDATE SKIP LOCKED 確保 batch 內 row 不被同時 lock；commit 後鎖釋放
//! - process 階段：per-row FOR UPDATE 再 lock 同 row、序列化兩 replica 對同 row 的處理；
//!   先到者執行 INSERT + 標 published_at；後到者 re-lock 後讀到 `published_at IS NOT NULL`
//!   即 skip，保證 sys_operation_log 不雙寫
//! - retry 階段：UPDATE retry_count 用 mini-txn、不受 main process 失敗影響

use std::time::Duration;

use chrono::Utc;
use sea_orm::sea_query::{LockBehavior, LockType};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
};
use server_config::AuditOutboxConfig;
use server_core::web::{
    audit::{AuditEventOwned, AuditSource},
    error::AppError,
};
use server_global::{global::GLOBAL_PRIMARY_DB, project_error, publish_audit_event};
use server_model::admin::{
    audit_log::HttpExtras,
    entities::{
        sys_audit_outbox::{
            self, ActiveModel as OutboxActiveModel, Entity as OutboxEntity,
        },
        sys_operation_log::ActiveModel as OperationLogActiveModel,
    },
};
use ulid::Ulid;

/// drainer 主迴圈：tokio::spawn 起來後常駐執行。
///
/// per spec FR-002 / quickstart §4.2。
pub async fn run_drainer_loop(config: AuditOutboxConfig) {
    tracing::info!(
        target: "[soybean-admin-rust]",
        "audit_outbox_drainer started (batch_size={}, sleep_ms={}, max_retry={}, maxlen={})",
        config.drainer_batch_size,
        config.drainer_sleep_interval_ms,
        config.drainer_max_retry,
        config.redis_stream_maxlen_approx
    );

    loop {
        match drainer_one_batch(&config).await {
            Ok(0) => {
                tokio::time::sleep(Duration::from_millis(config.drainer_sleep_interval_ms)).await;
            }
            Ok(_n) => {
                // 有處理就立刻下一輪、不 sleep（消化 backlog）
            }
            Err(e) => {
                project_error!("audit_outbox_drainer batch error: {:?}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// 一輪 batch：fetch pending IDs → 對每 row 開 per-row txn process。
///
/// Return：本輪實際處理（成功 OR 累計 retry）的 row 數；0 = 無 pending row 可撈。
async fn drainer_one_batch(config: &AuditOutboxConfig) -> Result<usize, AppError> {
    let db = {
        let guard = GLOBAL_PRIMARY_DB.read().await;
        guard.as_ref().cloned().ok_or_else(|| {
            AppError::from(DbErr::Custom("GLOBAL_PRIMARY_DB 未初始化".to_string()))
        })?
    };

    // === fetch 階段：SELECT FOR UPDATE SKIP LOCKED LIMIT N ===
    let fetch_txn = db.begin().await.map_err(AppError::from)?;
    let pending: Vec<sys_audit_outbox::Model> = OutboxEntity::find()
        .filter(sys_audit_outbox::Column::PublishedAt.is_null())
        .filter(sys_audit_outbox::Column::RetryCount.lt(config.drainer_max_retry as i32))
        .order_by_asc(sys_audit_outbox::Column::Id)
        .limit(config.drainer_batch_size)
        .lock_with_behavior(LockType::Update, LockBehavior::SkipLocked)
        .all(&fetch_txn)
        .await
        .map_err(AppError::from)?;
    // 立刻 commit fetch txn 釋放 lock — 後續 per-row txn 各自 lock 處理。
    // 雖然此刻其他 replica 可能 lock 同 row，但 process_one_row 內 UPDATE 含
    // `WHERE published_at IS NULL` 條件保證不會雙寫 sys_operation_log。
    fetch_txn.commit().await.map_err(AppError::from)?;

    // 044 W-F13: pending events gauge — per-batch sample（非絕對 backlog；
    // 若 backlog > batch_size、gauge 會讀到 batch_size 直到 backlog drain）
    metrics::gauge!("outbox_pending_events").set(pending.len() as f64);

    if pending.is_empty() {
        return Ok(0);
    }

    let mut processed = 0usize;
    for row in pending {
        let row_id = row.id;
        match process_one_row(db.as_ref(), &row, config).await {
            Ok(()) => {
                processed += 1;
            }
            Err(e) => {
                // 開 mini-txn 累計 retry_count（與 process txn 隔離、確保 retry 計次能寫入）
                if let Err(retry_err) = bump_retry_count(db.as_ref(), row_id, &e.message).await {
                    project_error!(
                        "audit_outbox_drainer: bump retry_count for row id={} failed: {:?}",
                        row_id,
                        retry_err
                    );
                } else {
                    tracing::warn!(
                        target: "[soybean-admin-rust]",
                        "audit_outbox_drainer: row id={} process failed (will retry): {}",
                        row_id,
                        e.message
                    );
                }
                processed += 1; // count as processed (we touched the row)
            }
        }
    }

    Ok(processed)
}

/// per-row txn 處理：publish Redis → begin → re-lock row → INSERT sys_operation_log
/// + UPDATE published_at → commit。
///
/// Redis publish 在 txn 外（fire-and-forget、不可 rollback）；DB 部分整體 atomic。
///
/// **multi-replica safety**：fetch 階段已釋 lock，本 fn begin 後立刻 `FOR UPDATE` 重 lock
/// 同 row 並重讀 published_at；若 published_at IS NOT NULL 表已被其他 replica 處理 → skip
/// （Ok(())、不重複 INSERT sys_operation_log）。
async fn process_one_row(
    db: &sea_orm::DatabaseConnection,
    row: &sys_audit_outbox::Model,
    config: &AuditOutboxConfig,
) -> Result<(), AppError> {
    // 1. publish Redis（fire-and-forget；自身失敗 warn 不返 error、不阻 DB INSERT）
    publish_audit_event(&row.audit_event_json, config.redis_stream_maxlen_approx).await;

    // 2. deserialize JSONB → owned event + 可選 http_extras
    let full: AuditEventFullOwned = serde_json::from_value(row.audit_event_json.clone())
        .map_err(|e| {
            AppError::from(DbErr::Custom(format!(
                "audit_outbox_drainer: deserialize audit_event_json row id={} failed: {}",
                row.id, e
            )))
        })?;

    // 3. begin per-row txn → re-lock row → idempotency check → INSERT + UPDATE
    let txn = db.begin().await.map_err(AppError::from)?;
    let locked = OutboxEntity::find_by_id(row.id)
        .lock(LockType::Update)
        .one(&txn)
        .await
        .map_err(AppError::from)?;
    let locked = match locked {
        Some(r) => r,
        None => {
            // row 被外部刪除（罕見）
            txn.rollback().await.ok();
            return Ok(());
        }
    };
    if locked.published_at.is_some() {
        // 已被其他 replica 處理過 → skip、不重複 INSERT sys_operation_log
        txn.rollback().await.ok();
        return Ok(());
    }

    let log_row = build_operation_log_active_model(&full)?;
    log_row.insert(&txn).await.map_err(AppError::from)?;

    let mut active: OutboxActiveModel = locked.into();
    active.published_at = Set(Some(Utc::now().into()));
    // 注意：last_error 不清（保留 debug 歷史；published_at IS NOT NULL 已是判 done 條件）
    active.update(&txn).await.map_err(AppError::from)?;

    txn.commit().await.map_err(AppError::from)?;
    Ok(())
}

/// 失敗 row 的 retry_count++ + last_error update（與 process txn 隔離）。
///
/// multi-replica safe：在新 txn 內 `FOR UPDATE` 鎖定該 row 再讀 retry_count、
/// 確保 increment 不會 race lost（同 row 被兩個 replica 同時 bump 只會逐次 +1）。
async fn bump_retry_count(
    db: &sea_orm::DatabaseConnection,
    row_id: i64,
    err_msg: &str,
) -> Result<(), AppError> {
    let txn = db.begin().await.map_err(AppError::from)?;
    let row = OutboxEntity::find_by_id(row_id)
        .lock(LockType::Update)
        .one(&txn)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| {
            AppError::from(DbErr::Custom(format!(
                "audit_outbox_drainer: bump_retry_count row id={} not found",
                row_id
            )))
        })?;
    let next_retry = row.retry_count + 1;
    let mut active: OutboxActiveModel = row.into();
    active.retry_count = Set(next_retry);
    active.last_error = Set(Some(err_msg.to_string()));
    active.update(&txn).await.map_err(AppError::from)?;
    txn.commit().await.map_err(AppError::from)?;
    Ok(())
}

/// drainer 端反序列化 audit_event_json 用的 owned 包裝 — 對應 write side
/// [`server_model::admin::audit_log::AuditEventFull`]（borrowed serialize）。
#[derive(Debug, serde::Deserialize)]
struct AuditEventFullOwned {
    #[serde(flatten)]
    event: AuditEventOwned,
    #[serde(default)]
    http_extras: Option<HttpExtras>,
}

/// 將 AuditEventFullOwned 映射為 sys_operation_log ActiveModel（per data-model.md R-4 mapping）。
fn build_operation_log_active_model(
    full: &AuditEventFullOwned,
) -> Result<OperationLogActiveModel, AppError> {
    let (method, url, ip, user_agent) = match &full.event.source {
        AuditSource::Http {
            method,
            url,
            ip,
            user_agent,
        } => (method.clone(), url.clone(), ip.clone(), user_agent.clone()),
        AuditSource::Internal => ("INTERNAL".to_string(), String::new(), String::new(), None),
        AuditSource::Cleanup => ("CLEANUP".to_string(), String::new(), String::new(), None),
    };

    let now = Utc::now().naive_utc();
    let description = full
        .event
        .description
        .clone()
        .unwrap_or_else(|| format!("{} id={}", full.event.operation.as_str(), full.event.entity_id));

    Ok(OperationLogActiveModel {
        id: Set(Ulid::new().to_string()),
        user_id: Set(full.event.actor.id.clone()),
        username: Set(full.event.actor.username.clone()),
        domain: Set(full.event.actor.domain.clone()),
        module_name: Set(full.event.entity_type.clone()),
        description: Set(description),
        request_id: Set(full.event.request_id.clone().unwrap_or_default()),
        method: Set(method),
        url: Set(url),
        ip: Set(ip),
        user_agent: Set(user_agent),
        params: Set(full.http_extras.as_ref().and_then(|e| e.params.clone())),
        body: Set(full.http_extras.as_ref().and_then(|e| e.body.clone())),
        response: Set(full.http_extras.as_ref().and_then(|e| e.response.clone())),
        start_time: Set(full.http_extras.as_ref().map(|e| e.start_time).unwrap_or(now)),
        end_time: Set(full.http_extras.as_ref().map(|e| e.end_time).unwrap_or(now)),
        duration: Set(full.http_extras.as_ref().map(|e| e.duration).unwrap_or(0)),
        created_at: Set(now),
        operation: Set(full.event.operation.as_str().to_string()),
        entity_id: Set(Some(full.event.entity_id.clone())),
        payload_before: Set(full.event.payload_before.clone()),
        payload_after: Set(full.event.payload_after.clone()),
    })
}

