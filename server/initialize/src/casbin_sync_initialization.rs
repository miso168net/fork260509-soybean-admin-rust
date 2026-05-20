//! Casbin pub-sub subscriber 背景 task。
//!
//! [`spawn_casbin_sync_subscriber`] 起一個長駐 tokio task,訂閱 redis channel
//! `casbin:policy:invalidate`(由 `server_global::notify_casbin_changed` publish);
//! 每收到一則 invalidate 訊號就對共用的 `CachedEnforcer` 做 full `load_policy()`
//! reload,讓多副本 rust-api 的 Casbin policy 保持跨 instance coherence。
//!
//! reconnect loop 設計(per FR-005 / edge case E-2):redis 全域未就緒、pub-sub
//! 連線失敗、或訂閱中斷線 → log + backoff + 重連;斷線期間該副本 policy 可能 stale,
//! 重連後下一則訊號即恢復。publisher 自己也會收到自己的訊號 → reload 一次、idempotent,
//! 不做 self-skip。

use std::{sync::Arc, time::Duration};

use casbin::{CachedEnforcer, CoreApi, Event, EventData, EventEmitter};
use futures::StreamExt;
use server_global::{
    global::{RedisConnection, GLOBAL_PRIMARY_REDIS},
    CASBIN_INVALIDATE_CHANNEL,
};
use tokio::sync::RwLock;

use crate::{project_error, project_info};

/// reconnect / retry 之間的 backoff 間隔。
const RECONNECT_BACKOFF: Duration = Duration::from_secs(5);

/// spawn 背景 task:訂閱 `casbin:policy:invalidate`、收到訊息對 `enforcer` 做
/// full `load_policy()` reload(並清 `CachedEnforcer` 的 enforce 結果快取)。
///
/// `enforcer` 是與 axum casbin layer 共用的同一個 `Arc<RwLock<CachedEnforcer>>`
/// clone(per research R-Q1 — 不新增全域)。
pub fn spawn_casbin_sync_subscriber(enforcer: Arc<RwLock<CachedEnforcer>>) {
    tokio::spawn(async move {
        project_info!("Casbin sync subscriber task spawned");
        loop {
            run_subscription(&enforcer).await;
            // run_subscription 只在 redis 未就緒 / 連線失敗 / 訂閱中斷時返回。
            tokio::time::sleep(RECONNECT_BACKOFF).await;
        }
    });
}

/// 跑一輪訂閱:取 redis client → 開專用 pub-sub 連線 → subscribe → 消費訊息。
/// 任一步失敗或訊息流結束即返回,交由外層 reconnect loop backoff 後重試。
async fn run_subscription(enforcer: &Arc<RwLock<CachedEnforcer>>) {
    let client = {
        let guard = GLOBAL_PRIMARY_REDIS.read().await;
        match guard.as_ref() {
            Some(RedisConnection::Single(client)) => client.clone(),
            Some(RedisConnection::Cluster(_)) => {
                project_error!(
                    "Casbin sync subscriber: GLOBAL_PRIMARY_REDIS 為 Cluster 模式,pub-sub 不支援,等待重試"
                );
                return;
            }
            None => {
                project_info!(
                    "Casbin sync subscriber: GLOBAL_PRIMARY_REDIS 尚未初始化,等待重試"
                );
                return;
            }
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(pubsub) => pubsub,
        Err(err) => {
            project_error!(
                "Casbin sync subscriber: 開啟 pub-sub 連線失敗,等待重連: {}",
                err
            );
            return;
        }
    };

    if let Err(err) = pubsub.subscribe(CASBIN_INVALIDATE_CHANNEL).await {
        project_error!(
            "Casbin sync subscriber: 訂閱 channel '{}' 失敗,等待重連: {}",
            CASBIN_INVALIDATE_CHANNEL,
            err
        );
        return;
    }

    project_info!(
        "Casbin sync subscriber: 已訂閱 channel '{}'",
        CASBIN_INVALIDATE_CHANNEL
    );

    let mut stream = pubsub.on_message();
    // stream 結束(None)= 連線中斷;返回後外層 backoff 重連。
    while stream.next().await.is_some() {
        reload_enforcer(enforcer).await;
    }

    project_error!("Casbin sync subscriber: pub-sub 訊息流中斷,等待重連");
}

/// 收到 invalidate 訊號後對 enforcer 做 full reload。
///
/// `CachedEnforcer::load_policy()` 只 reload policy 規則,**不**清 enforce 結果快取
/// (casbin 2.10:`CachedEnforcer::load_policy` 僅委派給內層 `Enforcer`,未碰
/// `cache`)。故 reload 後必須額外 emit `Event::ClearCache` 觸發 casbin 內建的
/// `clear_cache` callback,否則舊的 enforce 判斷仍命中快取、跨 instance coherence
/// 會無聲失效。
async fn reload_enforcer(enforcer: &Arc<RwLock<CachedEnforcer>>) {
    let mut guard = enforcer.write().await;
    match guard.load_policy().await {
        Ok(()) => {
            guard.emit(Event::ClearCache, EventData::ClearCache);
            project_info!("Casbin sync subscriber: 收到 invalidate 訊號,policy 已 reload");
        }
        Err(err) => {
            project_error!(
                "Casbin sync subscriber: load_policy 失敗,沿用舊 policy: {}",
                err
            );
        }
    }
}
