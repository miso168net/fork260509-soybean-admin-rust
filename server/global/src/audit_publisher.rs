//! 042 audit-outbox-and-http-mount: Redis Streams `audit:events` publisher。
//!
//! per spec FR-005 + quickstart §4.1 + research R-5（沿 `casbin_notify.rs` 體例：
//! fire-and-forget + Cluster 模式 graceful skip + Redis 未初始化 / 連線失敗 graceful skip +
//! 失敗只 warn 不回 error）。
//!
//! Drainer 處理 outbox row 時呼叫此 fn → XADD audit:events MAXLEN ~ N * event {json}。
//! 不在 caller transaction 中、單純 Redis 端 side-effect、失敗不阻 drainer outbox row 更新。

use crate::global::{RedisConnection, GLOBAL_PRIMARY_REDIS};

/// Redis Streams key — subscriber XREAD / XREVRANGE 此 key 取最新 audit events。
pub const AUDIT_STREAM_KEY: &str = "audit:events";

/// 預設 stream MAXLEN（approximate trim 上限、實際以 AuditOutboxConfig.redis_stream_maxlen_approx 為準）。
pub const AUDIT_STREAM_MAXLEN_APPROX: usize = 10000;

/// publish 一則 audit event 到 redis stream `audit:events`。
///
/// fire-and-forget：redis 不可用 / 非 Single 模式 / XADD 失敗一律 log warning，
/// 不回傳 error、不 panic、不阻斷呼叫端（drainer 邏輯）。
///
/// `maxlen` 為 XADD `MAXLEN ~ N` approximate trim 上限（從 AuditOutboxConfig 傳入）。
pub async fn publish_audit_event(audit_event_json: &serde_json::Value, maxlen: usize) {
    let client = {
        let guard = GLOBAL_PRIMARY_REDIS.read().await;
        match guard.as_ref() {
            Some(RedisConnection::Single(client)) => client.clone(),
            Some(RedisConnection::Cluster(_)) => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "publish_audit_event: GLOBAL_PRIMARY_REDIS 為 Cluster 模式、stream publish 略過"
                );
                return;
            }
            None => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "publish_audit_event: GLOBAL_PRIMARY_REDIS 未初始化、略過"
                );
                return;
            }
        }
    };

    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!(
                target: "[soybean-admin-rust]",
                "publish_audit_event: redis 連線失敗、略過 XADD: {}",
                err
            );
            return;
        }
    };

    let payload = audit_event_json.to_string();
    let result: redis::RedisResult<String> = redis::cmd("XADD")
        .arg(AUDIT_STREAM_KEY)
        .arg("MAXLEN")
        .arg("~")
        .arg(maxlen)
        .arg("*") // auto-generate stream id
        .arg("event")
        .arg(payload)
        .query_async(&mut conn)
        .await;

    if let Err(err) = result {
        tracing::warn!(
            target: "[soybean-admin-rust]",
            "publish_audit_event: XADD 到 stream '{}' 失敗: {}",
            AUDIT_STREAM_KEY,
            err
        );
    }
}
