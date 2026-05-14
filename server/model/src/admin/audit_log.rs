//! F2.1 unified audit log writer.
//!
//! 所有 admin write 路徑（service-level + HTTP middleware + F12 cleanup-job）共用此入口、
//! 由 `AuditEvent::source` 區分視角（Http / Internal / Cleanup）；
//! 對 sys_operation_log 18 既有欄 + 4 F2.1 新欄做完整 mapping（per data-model.md §E6）。
//!
//! Caller 負責 commit / rollback — 此 helper 只 insert、不 commit。
//! 寫入失敗時返回 `AppError { code: CODE_SERVER_DB_ERROR, ... }`，caller 應 rollback。
//!
//! 放在 `server-model` 而非 `server-service` — 避免 facade（in server-model）dep
//! service 形成循環依賴（per research.md R6 + speckit-analyze C3 修正）。

use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use server_core::web::{
    audit::{AuditEvent, AuditSource},
    code,
    error::AppError,
};
use ulid::Ulid;

use crate::admin::entities::sys_operation_log::ActiveModel as SysOperationLogActiveModel;

/// 在 caller 提供的 transaction 內 INSERT 一筆 `sys_operation_log` row。
///
/// `AuditEvent::source` 解構 → HTTP-視角欄位 mapping：
/// - `Http { method, url, ip, user_agent }` → 對應填充
/// - `Internal` → method=`"INTERNAL"`、url/ip 空字串、user_agent=None
/// - `Cleanup` → method=`"CLEANUP"`、url/ip 空字串、user_agent=None
pub async fn write_in_txn(
    txn: &DatabaseTransaction,
    event: AuditEvent<'_>,
) -> Result<(), AppError> {
    let now = Utc::now().naive_utc();

    let (method, url, ip, user_agent) = match &event.source {
        AuditSource::Http {
            method,
            url,
            ip,
            user_agent,
        } => (method.clone(), url.clone(), ip.clone(), user_agent.clone()),
        AuditSource::Internal => (
            "INTERNAL".to_string(),
            String::new(),
            String::new(),
            None,
        ),
        AuditSource::Cleanup => (
            "CLEANUP".to_string(),
            String::new(),
            String::new(),
            None,
        ),
    };

    let description = event
        .description
        .clone()
        .unwrap_or_else(|| format!("{} id={}", event.operation, event.entity_id));

    let row = SysOperationLogActiveModel {
        id: Set(Ulid::new().to_string()),
        // actor
        user_id: Set(event.actor.id.clone()),
        username: Set(event.actor.username.clone()),
        domain: Set(event.actor.domain.clone()),
        // entity
        module_name: Set(event.entity_type.to_string()),
        entity_id: Set(Some(event.entity_id.clone())),
        operation: Set(event.operation.as_str().to_string()),
        // payload
        payload_before: Set(event.payload_before.clone()),
        payload_after: Set(event.payload_after.clone()),
        // human-readable summary
        description: Set(description),
        request_id: Set(event.request_id.clone().unwrap_or_default()),
        // HTTP-視角 metadata（source 解構填充）
        method: Set(method),
        url: Set(url),
        ip: Set(ip),
        user_agent: Set(user_agent),
        // F2.1 暫不使用 params / response / body 三欄（F2.2 HTTP middleware enrichment 範圍）
        params: Set(None),
        response: Set(None),
        body: Set(None),
        // 時間
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
