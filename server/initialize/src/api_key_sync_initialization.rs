//! api_key pub-sub subscriber 背景 task（045 F3-N2、體例 mirror W-F11
//! casbin_sync_initialization）。
//!
//! [`spawn_api_key_sync_subscriber`] 起一個長駐 tokio task,訂閱 redis channel
//! `api_key:invalidate`(由 `server_global::notify_api_key_changed` publish);
//! 每收到一則 invalidate 訊號就對共用的 in-memory `SimpleApiKeyValidator` /
//! `ComplexApiKeyValidator` 做 clear + 從 DB SELECT active row 重新填入(full
//! reload),讓多副本 rust-api 的 api_key 驗證狀態保持跨 instance coherence。
//!
//! reconnect loop 設計(per FR-005):redis 全域未就緒、pub-sub 連線失敗、或
//! 訂閱中斷線 → log + backoff + 重連;斷線期間該副本 in-memory validator 可能
//! stale,重連後下一則訊號即恢復。publisher 自己也會收到自己的訊號 → reload 一次、
//! idempotent,不做 self-skip。

use std::time::Duration;

use futures::StreamExt;
use sea_orm::EntityTrait;
use server_core::sign::{add_key, clear_all_keys, ValidatorType};
use server_global::{
    global::{RedisConnection, GLOBAL_PRIMARY_REDIS},
    API_KEY_INVALIDATE_CHANNEL,
};
use server_model::admin::facade::sys_access_key;
use server_service::helper::db_helper;
use tracing::Instrument;

use crate::{project_error, project_info};

/// reconnect / retry 之間的 backoff 間隔。
const RECONNECT_BACKOFF: Duration = Duration::from_secs(5);

/// spawn 背景 task:訂閱 `api_key:invalidate`、收到訊息對 in-memory validator
/// 做 clear + 從 DB SELECT 重新填入(full reload)。
pub fn spawn_api_key_sync_subscriber() {
    tokio::spawn(
        async move {
            project_info!("api_key sync subscriber task spawned");
            loop {
                run_subscription().await;
                // run_subscription 只在 redis 未就緒 / 連線失敗 / 訂閱中斷時返回。
                tokio::time::sleep(RECONNECT_BACKOFF).await;
            }
        }
        .instrument(tracing::Span::current()),
    );
}

/// 跑一輪訂閱:取 redis client → 開專用 pub-sub 連線 → subscribe → 消費訊息。
/// 任一步失敗或訊息流結束即返回,交由外層 reconnect loop backoff 後重試。
async fn run_subscription() {
    let client = {
        let guard = GLOBAL_PRIMARY_REDIS.read().await;
        match guard.as_ref() {
            Some(RedisConnection::Single(client)) => client.clone(),
            Some(RedisConnection::Cluster(_)) => {
                project_error!(
                    "api_key sync subscriber: GLOBAL_PRIMARY_REDIS 為 Cluster 模式,pub-sub 不支援,等待重試"
                );
                return;
            }
            None => {
                project_info!(
                    "api_key sync subscriber: GLOBAL_PRIMARY_REDIS 尚未初始化,等待重試"
                );
                return;
            }
        }
    };

    let mut pubsub = match client.get_async_pubsub().await {
        Ok(pubsub) => pubsub,
        Err(err) => {
            project_error!(
                "api_key sync subscriber: 開啟 pub-sub 連線失敗,等待重連: {}",
                err
            );
            return;
        }
    };

    if let Err(err) = pubsub.subscribe(API_KEY_INVALIDATE_CHANNEL).await {
        project_error!(
            "api_key sync subscriber: 訂閱 channel '{}' 失敗,等待重連: {}",
            API_KEY_INVALIDATE_CHANNEL,
            err
        );
        return;
    }

    project_info!(
        "api_key sync subscriber: 已訂閱 channel '{}'",
        API_KEY_INVALIDATE_CHANNEL
    );

    let mut stream = pubsub.on_message();
    // stream 結束(None)= 連線中斷;返回後外層 backoff 重連。
    while stream.next().await.is_some() {
        reload_api_keys().await;
    }

    project_error!("api_key sync subscriber: pub-sub 訊息流中斷,等待重連");
}

/// 收到 invalidate 訊號後對 in-memory validator 做 full reload。
///
/// 步驟:
/// 1. `server_core::sign::clear_all_keys()` 清掉 Simple + Complex 兩個 validator
///    當下保存的所有 key / secret。
/// 2. 從 DB SELECT 所有 active(未軟刪)的 sys_access_key row。
/// 3. 對每 row 同時 register 進 Simple validator(key only)與 Complex validator
///    (key + secret),路徑與既有 `SysAccessKeyService::initialize_access_key`
///    啟動初始化一致。
/// 4. metrics::counter!("api_key_reload_total").increment(1)。
///
/// 任一步 DB 錯誤皆 log 後返回,不嘗試 partial state — 沿用既有 in-memory state
/// 等下一則訊號重試;若 clear 已執行但 SELECT 失敗,validator 會短暫為空、後續
/// 該副本 api_key 認證請求會 fail,等下次 invalidate 重 try 或 process 重啟 +
/// initialize_access_key 完整重填。
async fn reload_api_keys() {
    let db = match db_helper::get_db_connection().await {
        Ok(db) => db,
        Err(err) => {
            project_error!(
                "api_key sync subscriber: 取 DB connection 失敗,沿用舊 in-memory state: {}",
                err
            );
            return;
        }
    };

    // step 1: clear in-memory validator
    clear_all_keys().await;

    // step 2: SELECT 所有 active 的 access key
    let access_keys = match sys_access_key::find_active()
        .all(db.as_ref())
        .await
    {
        Ok(rows) => rows,
        Err(err) => {
            project_error!(
                "api_key sync subscriber: SELECT sys_access_key 失敗,in-memory validator 已清空、等待下一則訊號重 try: {}",
                err
            );
            return;
        }
    };

    // step 3: 重新填入 Simple + Complex validator
    for access_key in &access_keys {
        add_key(ValidatorType::Simple, &access_key.access_key_id, None).await;
        add_key(
            ValidatorType::Complex,
            &access_key.access_key_id,
            Some(&access_key.access_key_secret),
        )
        .await;
    }

    // step 4: metric
    metrics::counter!("api_key_reload_total").increment(1);

    project_info!(
        "api_key sync subscriber: 收到 invalidate 訊號,validator 已 reload({} keys)",
        access_keys.len()
    );
}
