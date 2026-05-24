//! sys_endpoint facade — 範式 A（非樹狀 entity）
//!
//! 故意不 re-export Entity，封死 service code 走 Entity::find() / Entity::delete_*() 路徑。
//! 4 個 bare function 是 service 入口；UPDATE / INSERT 仍走 ActiveModel。

use chrono::Local;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel, QueryFilter,
    Select, Set, TransactionTrait,
};
use server_core::db::soft_delete::SoftDeletable;
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    code,
    error::AppError,
};

use crate::admin::audit_log;
use crate::admin::audit_serialize::audit_snapshot;
use crate::admin::entities::sys_endpoint as _entity;

pub use _entity::{ActiveModel, Column, Model, Relation};
// pub use _entity::Entity;  ← 故意不 re-export

pub fn find_active() -> Select<_entity::Entity> {
    <_entity::Entity as SoftDeletable>::find_active()
}
pub fn find_with_deleted() -> Select<_entity::Entity> {
    <_entity::Entity as SoftDeletable>::find_with_deleted()
}

pub async fn soft_delete_by_id<C>(db: &C, id: String, actor: &Actor) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("begin txn failed: {}", e),
    })?;

    // F2.1: fetch before snapshot（active row state）
    let before = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch before failed: {}", e),
        })?;

    let res = _entity::Entity::update_many()
        .col_expr(_entity::Column::DeletedAt, Expr::current_timestamp().into())
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .exec(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("soft delete failed: {}", e),
        })?;

    if res.rows_affected == 0 {
        return Err(AppError {
            code: code::CODE_BUSINESS_ENTITY_NOT_FOUND,
            message: format!("entity not found or already deleted: id={}", id),
        });
    }

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation: AuditOperation::SoftDelete,
            entity_type: "sys_endpoint",
            entity_id: id.clone(),
            payload_before: before.as_ref().map(audit_snapshot),
            payload_after: None,
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await?;

    txn.commit().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("commit failed: {}", e),
    })?;
    Ok(())
}

// =============================================================================
// 045 F3-N1: upsert_with_audit — INSERT/UPDATE row-level write + audit 一體化
// =============================================================================

/// 045 F3-N1：sys_endpoint INSERT / UPDATE 寫入 + audit row 同 txn commit。
///
/// 行為（per data-model.md §E1.2）：
/// 1. begin txn → fetch active before snapshot by id
/// 2. before 為 None → INSERT + audit `Insert`
/// 3. before 為 Some 但「業務欄位」與 caller 提供的 endpoint 等同 → noop（不 audit，僅 commit）
/// 4. before 為 Some 且有 diff → UPDATE（保留原 created_at、updated_at = now）+ audit `Update`
/// 5. commit txn
///
/// **與 data-model.md 字面 spec 的偏差（DONE_WITH_CONCERNS）**：
/// 字面 spec 寫 `before_row == endpoint`（用整 Model PartialEq）；實際 caller
/// `process_collected_routes()` 每次啟動會 regenerate `display_id`（snowflake）+
/// `created_at`（Local::now），導致整 Model 一定 != → 永遠走 UPDATE → 每次重啟 audit 噴 N 筆。
/// 改用「業務欄位 semantic 比較」（path / method / action / resource / controller / summary），
/// 並 UPDATE 時保留 before 的 created_at、display_id，僅刷新 updated_at。
pub async fn upsert_with_audit<C>(
    db: &C,
    endpoint: _entity::Model,
    actor: &Actor,
) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("begin txn failed: {}", e),
    })?;

    let before = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&endpoint.id))
        .filter(_entity::Column::DeletedAt.is_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch before failed: {}", e),
        })?;

    let (operation, payload_before, payload_after) = match before {
        None => {
            // INSERT 路徑
            let active: _entity::ActiveModel = endpoint.clone().into_active_model();
            let new_row = active.insert(&txn).await.map_err(|e| AppError {
                code: code::CODE_SERVER_DB_ERROR,
                message: format!("insert failed: {}", e),
            })?;
            (
                AuditOperation::Insert,
                None,
                Some(audit_snapshot(&new_row)),
            )
        },
        Some(before_row) => {
            // 業務欄位 semantic 比較（避免 created_at / display_id 假 diff 觸發 audit 噴洗）
            let same_business = before_row.path == endpoint.path
                && before_row.method == endpoint.method
                && before_row.action == endpoint.action
                && before_row.resource == endpoint.resource
                && before_row.controller == endpoint.controller
                && before_row.summary == endpoint.summary;

            if same_business {
                txn.commit().await.map_err(|e| AppError {
                    code: code::CODE_SERVER_DB_ERROR,
                    message: format!("commit failed: {}", e),
                })?;
                return Ok(());
            }

            // UPDATE 路徑：保留 before 的 created_at + display_id，僅刷新業務欄位 + updated_at
            let now = Local::now().naive_local();
            let mut am: _entity::ActiveModel = before_row.clone().into_active_model();
            am.path = Set(endpoint.path.clone());
            am.method = Set(endpoint.method.clone());
            am.action = Set(endpoint.action.clone());
            am.resource = Set(endpoint.resource.clone());
            am.controller = Set(endpoint.controller.clone());
            am.summary = Set(endpoint.summary.clone());
            am.updated_at = Set(Some(now));

            let updated = am.update(&txn).await.map_err(|e| AppError {
                code: code::CODE_SERVER_DB_ERROR,
                message: format!("update failed: {}", e),
            })?;

            (
                AuditOperation::Update,
                Some(audit_snapshot(&before_row)),
                Some(audit_snapshot(&updated)),
            )
        },
    };

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation,
            entity_type: "sys_endpoint",
            entity_id: endpoint.id.clone(),
            payload_before,
            payload_after,
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await?;

    txn.commit().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("commit failed: {}", e),
    })?;
    Ok(())
}

// =============================================================================
// 045 F3-N3: batch_soft_delete_with_audit — batch surface + policy
// =============================================================================

/// 045 F3-N3 batch soft-delete 策略。
#[derive(Debug, Clone, Copy)]
pub enum BatchDeletePolicy {
    /// 任一 row 失敗 → 整個 batch return Err。
    FailFast,
    /// 任一 row 失敗 → warn log（target 指定 tracing target）+ 收進 failed Vec、不阻斷後續。
    LogAndContinue { target: &'static str },
}

/// 045 F3-N3 batch soft-delete 結果：成功 id list + 失敗 (id, error) list。
#[derive(Debug, Default)]
pub struct BatchDeleteResult {
    pub ok: Vec<String>,
    pub failed: Vec<(String, AppError)>,
}

/// 045 F3-N3：對 ids 逐個呼叫 `soft_delete_by_id`，依 policy 決定失敗策略。
///
/// 注意：每筆 `soft_delete_by_id` 內部各自開 txn（row-level atomicity），
/// 故 batch 不是 all-or-nothing single-txn；FailFast 也只保證 fail 點以前已 commit，
/// 與既有 service-layer per-id loop 行為一致（per data-model.md §E1.2）。
pub async fn batch_soft_delete_with_audit<C>(
    db: &C,
    ids: Vec<String>,
    actor: &Actor,
    policy: BatchDeletePolicy,
) -> Result<BatchDeleteResult, AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut result = BatchDeleteResult::default();
    for id in ids {
        match soft_delete_by_id(db, id.clone(), actor).await {
            Ok(()) => result.ok.push(id),
            Err(e) => match policy {
                BatchDeletePolicy::FailFast => return Err(e),
                BatchDeletePolicy::LogAndContinue { target } => {
                    tracing::warn!(
                        target = target,
                        id = %id,
                        error = ?e,
                        "batch_soft_delete: per-row soft_delete failed"
                    );
                    result.failed.push((id, e));
                },
            },
        }
    }
    Ok(result)
}

pub async fn restore_by_id<C>(db: &C, id: String, actor: &Actor) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("begin txn failed: {}", e),
    })?;

    // F2.1: fetch before（軟刪態 snapshot）
    let before = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_not_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch before failed: {}", e),
        })?;

    let res = _entity::Entity::update_many()
        .col_expr(
            _entity::Column::DeletedAt,
            Expr::value(None::<chrono::NaiveDateTime>),
        )
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_not_null())
        .exec(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("restore failed: {}", e),
        })?;

    if res.rows_affected == 0 {
        return Err(AppError {
            code: code::CODE_BUSINESS_ENTITY_NOT_FOUND,
            message: format!("entity not found or already active: id={}", id),
        });
    }

    // F2.1: fetch after（active 態 snapshot）
    let after = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch after failed: {}", e),
        })?;

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation: AuditOperation::Restore,
            entity_type: "sys_endpoint",
            entity_id: id.clone(),
            payload_before: before.as_ref().map(audit_snapshot),
            payload_after: after.as_ref().map(audit_snapshot),
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await?;

    txn.commit().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("commit failed: {}", e),
    })?;
    Ok(())
}
