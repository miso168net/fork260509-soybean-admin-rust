//! F3 service-level audit log writer.
//!
//! 與既有 `sys_operation_log_service::handle_operation_log_event`（HTTP middleware
//! event 機制）解耦並存；F3 facade module 在 SOFT_DELETE / RESTORE 路徑下呼叫此 helper
//! 同 transaction 寫一筆 `sys_operation_log` row。
//!
//! `method="INTERNAL"` 標記 — grep `module_name=sys_<entity>` + `method=INTERNAL`
//! 即可挑出 F3 service-level audit row（vs middleware event audit row）。
//!
//! 放在 `server-model` 而非 `server-service` — 避免 facade（in server-model）dep
//! service 形成循環依賴（per research.md R6 + speckit-analyze C3 修正）。

use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use server_core::web::{audit::AuditLogCtx, code, error::AppError};
use ulid::Ulid;

use crate::admin::entities::sys_operation_log::ActiveModel as SysOperationLogActiveModel;

/// 在 caller 提供的 transaction 內 INSERT 一筆 `sys_operation_log` row。
///
/// Caller 負責 commit / rollback — 此 helper 只 insert、不 commit。
/// 寫入失敗時返回 `AppError { code: CODE_SERVER_DB_ERROR, ... }`，caller 應 rollback。
pub async fn write_in_txn(
    txn: &DatabaseTransaction,
    ctx: AuditLogCtx<'_>,
) -> Result<(), AppError> {
    let now = Utc::now().naive_utc();
    let row = SysOperationLogActiveModel {
        id: Set(Ulid::new().to_string()),
        user_id: Set(ctx.actor.id.clone()),
        username: Set(ctx.actor.username.clone()),
        domain: Set(ctx.actor.domain.clone()),
        module_name: Set(ctx.entity_type.to_string()),
        description: Set(ctx.description.clone()),
        request_id: Set(ctx.request_id.clone().unwrap_or_default()),
        method: Set("INTERNAL".to_string()),
        url: Set(String::new()),
        ip: Set(String::new()),
        user_agent: Set(None),
        params: Set(None),
        body: Set(None),
        response: Set(None),
        start_time: Set(now),
        end_time: Set(now),
        duration: Set(0),
        created_at: Set(now),
    };
    row.insert(txn).await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("audit log insert failed: {}", e),
    })?;
    Ok(())
}
