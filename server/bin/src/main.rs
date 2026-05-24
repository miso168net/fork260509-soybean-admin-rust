use std::net::SocketAddr;

use axum::{extract::Request, ServiceExt};
use tokio::net::TcpListener;
use tower::Layer;
use tower_http::normalize_path::NormalizePathLayer;

#[tokio::main]
async fn main() {
    let config_path = if cfg!(debug_assertions) {
        "server/resources/application-test.yaml"
    } else {
        "server/resources/application.yaml"
    };

    server_initialize::initialize_log_tracing().await;

    // 使用多实例环境变量优先的配置加载方式
    // 支持单个配置项和多实例配置的环境变量覆盖
    server_initialize::initialize_config_with_multi_instance_env(config_path, None).await;
    let _ = server_initialize::init_xdb().await;
    server_initialize::init_primary_connection().await;
    server_initialize::init_db_pools().await;
    server_initialize::initialize_keys_and_validation().await;
    server_initialize::initialize_event_channel().await;

    server_initialize::init_primary_redis().await;
    server_initialize::init_redis_pools().await;
    server_initialize::init_primary_mongo().await;
    server_initialize::init_mongo_pools().await;

    // build our application with a route
    let app = server_initialize::initialize_admin_router().await;
    // 全 router compose 最外層 wrap NormalizePathLayer：trim request path 的 trailing slash，
    // 使 nested `/`-rooted endpoint（如 /api/user、/api/role）對帶 trailing slash request 一致回 200，
    // 而非 axum 0.8 nested router 預設 404 行為。layer 在 routing 之前 apply、casbin/audit 看到 normalized
    // path、與 sys_endpoint 表記錄字面一致（per spec 041 FR-008/009、Clarifications 2026-05-24 Q1）。
    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    //需要初始化验证器init_validators之后才能初始化访问密钥
    server_initialize::initialize_access_key().await;

    let addr = match server_initialize::get_server_address().await {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Failed to get server address: {}", e);
            return;
        },
    };

    // run it
    let listener = TcpListener::bind(&addr).await.unwrap();
    // tracing::debug!("listening on {}", listener.local_addr().unwrap());
    // 用 axum::ServiceExt 的 into_make_service_with_connect_info（而非 Router 內建版）：
    // NormalizePathLayer wrap 後型別由 Router 變 NormalizePath<Router>，Router 專屬 method
    // 不再可用；ServiceExt trait 為任何 Service<Request> 提供同名 helper、保留 ConnectInfo<SocketAddr>
    // extractor 對 auth handler / audit log 的支援。
    axum::serve(
        listener,
        ServiceExt::<Request>::into_make_service_with_connect_info::<SocketAddr>(app),
    )
    .await
    .unwrap();
}
