//! F11 extracted-stubs: `/mock/getLastTime` route mount。
//! per spec FR-004 + data-model E3、對齊 sys_sandbox_route.rs pattern（`const BASE_PATH` + `nest`）。

use axum::{http::Method, routing::get, Router};
use server_api::admin::SysMockApi;
use server_global::global::{add_route, RouteInfo};

pub struct SysMockRouter;

impl SysMockRouter {
    const BASE_PATH: &'static str = "/mock";

    pub async fn init_mock_router() -> Router {
        let service_name = "SysMockApi";

        let routes = vec![RouteInfo::new(
            &format!("{}/getLastTime", Self::BASE_PATH),
            Method::GET,
            service_name,
            "获取最后时间",
        )];

        for route in routes {
            add_route(route).await;
        }

        let router = Router::new().route("/getLastTime", get(SysMockApi::get_last_time));

        Router::new().nest(Self::BASE_PATH, router)
    }
}
