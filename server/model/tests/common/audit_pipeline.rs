//! 046 US4 helper：等 audit row 從 sys_audit_outbox 透過 drainer 進到
//! sys_operation_log。test 用 closure 描述 Sea-ORM 查詢、helper 輪詢直到
//! 拿到 Some(model) 或 timeout。
//!
//! 設計目的：042 outbox refactor 後 `audit_log::write_in_txn` 改寫
//! sys_audit_outbox（同 caller txn），drainer 背景異步消化、才到
//! sys_operation_log。改前直接 SELECT 取 row 變 None；改後輪詢直到 row 出現
//! 或 timeout（500ms 默認 budget、50ms interval）。

use sea_orm::DbErr;
use server_model::admin::entities::sys_operation_log;
use std::future::Future;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// 輪詢 find_fn 直到回 `Ok(Some(model))` 或 timeout。
///
/// `find_fn` 為 closure、每次輪詢呼叫一次、用 Sea-ORM Entity::find()
/// pattern 拿 `Option<sys_operation_log::Model>`。
///
/// 預設 budget：500ms（與 042 drainer_sleep_interval_ms=50ms × 10 safety
/// margin、042 SC-005 p95<200ms 預期落點對齊）。
#[allow(dead_code)]
pub async fn wait_for_audit_row<F, Fut>(
    find_fn: F,
    timeout_ms: u64,
) -> Result<sys_operation_log::Model, String>
where
    F: Fn() -> Fut + Send,
    Fut: Future<Output = Result<Option<sys_operation_log::Model>, DbErr>> + Send,
{
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match find_fn().await {
            Ok(Some(model)) => return Ok(model),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return Err(format!("timeout {timeout_ms}ms waiting for audit row"));
                }
                sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(format!("find_fn DB error: {}", e)),
        }
    }
}

/// 變體：輪詢 count_fn 直到回 `Ok(count >= min_count)` 或 timeout。
///
/// 適用於 audit_http_middleware 場景需驗多 row（HTTP request 一次寫 1 HTTP
/// audit + 1 INTERNAL audit、共 2 row）。
#[allow(dead_code)]
pub async fn wait_for_audit_count<F, Fut>(
    count_fn: F,
    min_count: u64,
    timeout_ms: u64,
) -> Result<u64, String>
where
    F: Fn() -> Fut + Send,
    Fut: Future<Output = Result<u64, DbErr>> + Send,
{
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        match count_fn().await {
            Ok(c) if c >= min_count => return Ok(c),
            Ok(_) => {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "timeout {timeout_ms}ms waiting for count>={min_count}"
                    ));
                }
                sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(format!("count_fn DB error: {}", e)),
        }
    }
}
