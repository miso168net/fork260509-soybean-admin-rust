//! api_key pub-sub publisher（045 F3-N2、體例 mirror W-F11 casbin_notify）。
//!
//! 當 sys_access_key DELETE（軟刪）後,呼叫 [`notify_api_key_changed`] 對 redis
//! channel 廣播一則最小 invalidate 訊號;其他 rust-api 副本的 subscriber 收到後
//! 對 in-memory `SimpleApiKeyValidator` / `ComplexApiKeyValidator` 做 clear +
//! 從 DB 重新 SELECT 重填(full reload),讓多副本 api_key 驗證狀態保持跨 instance
//! coherence。
//!
//! fire-and-forget 設計:redis 不可用 / 非 Single 模式皆 log warning 並 no-op,
//! 不回傳 error、不 panic、不阻斷呼叫端(per FR-004)。

use crate::global::{RedisConnection, GLOBAL_PRIMARY_REDIS};

/// api_key invalidate 訊號的 redis pub-sub channel 名稱。
pub const API_KEY_INVALIDATE_CHANNEL: &str = "api_key:invalidate";

/// invalidate 訊號 payload。最小固定字串 — subscriber 收到後一律做 full reload,
/// 不從 payload 解讀異動 semantics。
const API_KEY_INVALIDATE_PAYLOAD: &str = "1";

/// publish 一則 api_key invalidate 訊號到 redis channel。
///
/// fire-and-forget:redis 不可用 / 非 Single 模式 → log warning、不回傳 error、
/// 不阻斷呼叫端。rev1 stack 為單一 redis;`Cluster` 模式 pub-sub 不在範圍內。
pub async fn notify_api_key_changed() {
    let client = {
        let guard = GLOBAL_PRIMARY_REDIS.read().await;
        match guard.as_ref() {
            Some(RedisConnection::Single(client)) => client.clone(),
            Some(RedisConnection::Cluster(_)) => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "notify_api_key_changed: GLOBAL_PRIMARY_REDIS 為 Cluster 模式,pub-sub 不支援,略過"
                );
                return;
            }
            None => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "notify_api_key_changed: GLOBAL_PRIMARY_REDIS 未初始化,略過"
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
                "notify_api_key_changed: redis 連線失敗,略過 publish: {}",
                err
            );
            return;
        }
    };

    let result: redis::RedisResult<()> = redis::cmd("PUBLISH")
        .arg(API_KEY_INVALIDATE_CHANNEL)
        .arg(API_KEY_INVALIDATE_PAYLOAD)
        .query_async(&mut conn)
        .await;

    match result {
        Ok(_) => {
            metrics::counter!("api_key_invalidate_total").increment(1);
        }
        Err(err) => {
            tracing::warn!(
                target: "[soybean-admin-rust]",
                "notify_api_key_changed: PUBLISH 到 channel '{}' 失敗: {}",
                API_KEY_INVALIDATE_CHANNEL,
                err
            );
        }
    }
}
