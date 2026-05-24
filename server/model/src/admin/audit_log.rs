//! 042 audit-outbox-and-http-mount: outbox-first audit writer。
//!
//! 所有 admin write 路徑（service-level + HTTP middleware + F12 cleanup-job）共用此入口、
//! 由 `AuditEvent::source` 區分視角（Http / Internal / Cleanup）。
//!
//! **042 起 内部目標改 sys_audit_outbox**：caller API（`write_in_txn(&txn, event)` 簽名 +
//! Result）不變、030–040 features 30+ callsite 0 改動（per FR-009）。
//! Service-level INTERNAL audit 仍與業務同 transaction commit / rollback（同 txn `insert(txn)`、
//! `caller` 仍負責 commit / rollback）。
//!
//! 新增 `write_outbox_for_http(ctx)` helper 給 HTTP middleware 用：post-execution hook、
//! 業務已 commit、helper 自管 small txn、failure 由 caller wrap tokio::spawn warn log。
//!
//! 新增 pure fn `url_to_entity_type` — Hybrid rule URL → entity_type、unit-test 覆蓋全 path
//! (per data-model.md §E2、tests/url_entity_type.rs)。

use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, DbErr, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
};
use server_global::global::{OperationLogContext, GLOBAL_PRIMARY_DB, REQUEST_ID_TASK_LOCAL};

use crate::admin::entities::sys_audit_outbox::ActiveModel as SysAuditOutboxActiveModel;

// =============================================================================
// AuditEventFull / HttpExtras — 042 audit_event_json JSONB schema
// =============================================================================

/// 042: 序列化 AuditEvent + 可選 HTTP-only extras 為 audit_event_json JSONB top-level
/// (per data-model.md §E1 JSONB schema、quickstart §2.1)。
///
/// Serialize-only、欄位借用 caller 的 AuditEvent；drainer 端反序列化用
/// [`server_core::web::audit::AuditEventOwned`] + [`HttpExtras`]。
#[derive(Serialize)]
pub struct AuditEventFull<'a, 'b> {
    #[serde(flatten)]
    pub event: &'b AuditEvent<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_extras: Option<HttpExtras>,
}

/// 042: HTTP middleware 視角獨有 metadata — params / body / response / 計時三欄。
///
/// `Internal` / `Cleanup` source 不填此欄；drainer 端寫 sys_operation_log 時
/// 若 `http_extras` 為 None 即 `params/body/response = None`、`start_time/end_time = now()`、
/// `duration = 0`（per data-model.md §E1 + R-4 mapping table）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpExtras {
    pub params: Option<JsonValue>,
    pub body: Option<JsonValue>,
    pub response: Option<JsonValue>,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
    pub duration: i32,
}

// =============================================================================
// T009 — write_in_txn 內部目標改 outbox（caller API 不變）
// =============================================================================

/// 在 caller 提供的 transaction 內 INSERT 一筆 `sys_audit_outbox` row。
///
/// 042 起內部目標從 `sys_operation_log` 改為 `sys_audit_outbox.audit_event_json` JSONB；
/// drainer 背景消化後填 sys_operation_log + Redis Stream。**caller API 完全不變**
/// （簽名、Result type 與既有完全一致；030-040 全 callsite 0 改）。
pub async fn write_in_txn(
    txn: &DatabaseTransaction,
    event: AuditEvent<'_>,
) -> Result<(), AppError> {
    // 042: 若 caller 未填 request_id（030-040 service-level callsite 預設 None）、
    // fallback 至 OperationLogLayer 設的 tokio task-local；HTTP / INTERNAL 兩視角 row
    // 因此共用同一 request_id、forensic 可串聯（per data-model §E3）。
    let mut event = event;
    if event.request_id.is_none() {
        if let Ok(rid) = REQUEST_ID_TASK_LOCAL.try_with(|v| v.clone()) {
            event.request_id = Some(rid);
        }
    }

    let event_json = serde_json::to_value(AuditEventFull {
        event: &event,
        http_extras: None,
    })
    .map_err(|e| AppError::from(DbErr::Custom(format!("audit JSON serialize: {}", e))))?;

    let outbox_row = SysAuditOutboxActiveModel {
        audit_event_json: Set(event_json),
        ..Default::default()
    };

    outbox_row.insert(txn).await.map_err(|e| {
        // 保留 DbErr → AppError 細節（unique violation 等可走專屬 code）；只擴 message prefix
        // 標明來自 audit insert，方便 caller / grep 區分業務 INSERT 與 audit row INSERT 失敗
        let mut err = AppError::from(e);
        err.message = format!("audit log insert failed: {}", err.message);
        err
    })?;
    Ok(())
}

// =============================================================================
// T010 — write_outbox_for_http: HTTP middleware 專用、自管 txn
// =============================================================================

/// 042: HTTP middleware post-execution hook 寫 outbox helper。
///
/// middleware 為 post-execution、業務 response 已產生且不可 rollback；此 helper:
/// - 從 ctx.method 推 AuditOperation（GET/HEAD/OPTIONS/TRACE → skip per FR-015）
/// - 從 ctx.url 推 entity_type（hybrid rule per FR-004 / data-model §E2）
/// - 自管 small txn（begin → INSERT outbox → commit）
/// - 任何 error 返 Result 給 caller；caller（middleware）wrap tokio::spawn warn log
///   後不影響 response（per FR-008）
pub async fn write_outbox_for_http(ctx: OperationLogContext) -> Result<(), AppError> {
    // GET / HEAD / OPTIONS / TRACE 等 read path 不寫 audit（per spec FR-015 + Constitution §II）
    let operation = match ctx.method.to_uppercase().as_str() {
        "POST" => AuditOperation::Insert,
        "PUT" | "PATCH" => AuditOperation::Update,
        "DELETE" => AuditOperation::SoftDelete,
        _ => return Ok(()),
    };

    let entity_type = url_to_entity_type(&ctx.url);
    let entity_id = extract_entity_id_from_url(&ctx.url);

    let actor = Actor {
        id: ctx.user_id.clone().unwrap_or_default(),
        username: ctx.username.clone().unwrap_or_default(),
        domain: ctx.domain.clone().unwrap_or_default(),
    };

    let event = AuditEvent {
        actor: &actor,
        operation,
        entity_type,
        entity_id,
        payload_before: None, // middleware 看不到 SQL-level snapshot
        payload_after: None,
        description: Some(format!("HTTP {} {}", ctx.method, ctx.url)),
        source: AuditSource::Http {
            method: ctx.method.clone(),
            url: ctx.url.clone(),
            ip: ctx.ip.clone(),
            user_agent: ctx.user_agent.clone(),
        },
        request_id: Some(ctx.request_id.clone()),
    };

    let event_json = serde_json::to_value(AuditEventFull {
        event: &event,
        http_extras: Some(HttpExtras {
            params: ctx.params.clone(),
            body: ctx.body.clone(),
            response: ctx.response.clone(),
            start_time: ctx.start_time,
            end_time: ctx.end_time,
            duration: ctx.duration,
        }),
    })
    .map_err(|e| AppError::from(DbErr::Custom(format!("audit JSON serialize: {}", e))))?;

    // 取 primary DB connection（不 dep server-service::helper::db_helper、避免反向 dep）
    let db = {
        let guard = GLOBAL_PRIMARY_DB.read().await;
        guard.as_ref().cloned().ok_or_else(|| {
            AppError::from(DbErr::Custom(
                "GLOBAL_PRIMARY_DB 未初始化、HTTP audit outbox 寫入跳過".to_string(),
            ))
        })?
    };

    let txn = db.begin().await.map_err(AppError::from)?;
    let outbox_row = SysAuditOutboxActiveModel {
        audit_event_json: Set(event_json),
        ..Default::default()
    };
    outbox_row.insert(&txn).await.map_err(|e| {
        let mut err = AppError::from(e);
        err.message = format!("audit log insert failed: {}", err.message);
        err
    })?;
    txn.commit().await.map_err(AppError::from)?;
    Ok(())
}

/// best-effort entity_id 抽取：取 URL path 第 2 段（split('/').nth(2)）。
///
/// 注意：rust-api 收到的 URL 已被 front-nginx `proxy_pass http://rust_api/`
/// 剝離 `/api/` 前綴。實際 path 為 `/user/123` 而非 `/api/user/123`。
///
/// 例：`/user/123?x=1` → `["", "user", "123"]`.nth(2) = `"123"`。
/// 無 path segment 時返空字串（drainer / sys_operation_log.entity_id 容許 NULL/空）。
fn extract_entity_id_from_url(url: &str) -> String {
    url.split('?')
        .next()
        .unwrap_or(url)
        .split('/')
        .nth(2)
        .unwrap_or("")
        .to_string()
}

// =============================================================================
// T007 — pure fn url_to_entity_type（Hybrid rule per FR-004 + data-model §E2）
// =============================================================================

/// Hybrid rule URL → entity_type (per spec 003 Clarifications Q2 + 042 FR-004)。
///
/// 順序：systemManage alias 先（避免 prefix 衝突）、native admin router 後、
/// fallback `http_event` 最後（per data-model.md §E2、unit-tested in
/// `server/model/tests/url_entity_type.rs`）。
///
/// 注意：rust-api 收到的 URL 已被 front-nginx `proxy_pass http://rust_api/`
/// 剝離 `/api/` 前綴。本表 match 對應 `/role`、`/auth/login` 等
/// （非 `/api/role`、`/api/auth/login`）。
pub fn url_to_entity_type(url: &str) -> &'static str {
    // remove query string
    let path = url.split('?').next().unwrap_or(url);

    // systemManage alias — sys_user
    if path.starts_with("/systemManage/addUser") || path.starts_with("/systemManage/updateUser") {
        return "sys_user";
    }
    // systemManage alias — sys_menu
    if path.starts_with("/systemManage/addMenu")
        || path.starts_with("/systemManage/updateMenu")
        || path.starts_with("/systemManage/deleteMenu")
        || path.starts_with("/systemManage/batchDeleteMenu")
    {
        return "sys_menu";
    }
    // systemManage alias — sys_role
    if path.starts_with("/systemManage/addRole")
        || path.starts_with("/systemManage/updateRole")
        || path.starts_with("/systemManage/deleteRole")
        || path.starts_with("/systemManage/batchDeleteRole")
        || path.starts_with("/systemManage/assignRoleMenus")
        || path.starts_with("/systemManage/updateRoleHome")
        || path.starts_with("/systemManage/assignRoleEndpoints")
    {
        return "sys_role";
    }

    // native admin router（per 041：menu 實 mount 在 /route）
    if path.starts_with("/user") {
        return "sys_user";
    }
    if path.starts_with("/role") {
        return "sys_role";
    }
    if path.starts_with("/route") {
        return "sys_menu";
    }
    if path.starts_with("/domain") {
        return "sys_domain";
    }
    if path.starts_with("/organization") {
        return "sys_organization";
    }
    if path.starts_with("/api-endpoint") {
        return "sys_endpoint";
    }
    if path.starts_with("/access-key") {
        return "sys_access_key";
    }
    if path.starts_with("/authorization") {
        return "sys_role";
    }

    // fallback / unknown / /auth/*
    "http_event"
}
