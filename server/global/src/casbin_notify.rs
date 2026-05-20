//! Casbin pub-sub publisher。
//!
//! 當 Casbin policy 異動(role / route 指派變更)後,呼叫 [`notify_casbin_changed`]
//! 對 redis channel 廣播一則最小 invalidate 訊號;其他 rust-api 副本的
//! subscriber 收到後做 full Casbin reload。
//!
//! fire-and-forget 設計:redis 不可用 / 非 Single 模式皆 log warning 並 no-op,
//! 不回傳 error、不 panic、不阻斷呼叫端(per FR-006)。

use crate::global::{RedisConnection, GLOBAL_PRIMARY_REDIS};

/// Casbin policy invalidate 訊號的 redis pub-sub channel 名稱。
pub const CASBIN_INVALIDATE_CHANNEL: &str = "casbin:policy:invalidate";

/// invalidate 訊號 payload。最小固定字串 — subscriber 收到後一律做 full reload,
/// 不從 payload 解讀異動 semantics。
const CASBIN_INVALIDATE_PAYLOAD: &str = "1";

/// publish 一則 Casbin policy invalidate 訊號到 redis channel。
///
/// fire-and-forget:redis 不可用 / 非 Single 模式 → log warning、不回傳 error、
/// 不阻斷呼叫端。rev1 stack 為單一 redis;`Cluster` 模式 pub-sub 不在範圍內。
pub async fn notify_casbin_changed() {
    let client = {
        let guard = GLOBAL_PRIMARY_REDIS.read().await;
        match guard.as_ref() {
            Some(RedisConnection::Single(client)) => client.clone(),
            Some(RedisConnection::Cluster(_)) => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "notify_casbin_changed: GLOBAL_PRIMARY_REDIS 為 Cluster 模式,pub-sub 不支援,略過"
                );
                return;
            }
            None => {
                tracing::warn!(
                    target: "[soybean-admin-rust]",
                    "notify_casbin_changed: GLOBAL_PRIMARY_REDIS 未初始化,略過"
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
                "notify_casbin_changed: redis 連線失敗,略過 publish: {}",
                err
            );
            return;
        }
    };

    let result: redis::RedisResult<()> = redis::cmd("PUBLISH")
        .arg(CASBIN_INVALIDATE_CHANNEL)
        .arg(CASBIN_INVALIDATE_PAYLOAD)
        .query_async(&mut conn)
        .await;

    if let Err(err) = result {
        tracing::warn!(
            target: "[soybean-admin-rust]",
            "notify_casbin_changed: PUBLISH 到 channel '{}' 失敗: {}",
            CASBIN_INVALIDATE_CHANNEL,
            err
        );
    }
}
