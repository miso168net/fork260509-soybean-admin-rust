use axum::{
    http::Method,
    routing::{get, post},
    Router,
};
use server_api::admin::SysAuthenticationApi;
use server_global::global::{add_route, RouteInfo};

pub struct SysAuthenticationRouter;

impl SysAuthenticationRouter {
    pub async fn init_authentication_router() -> Router {
        let router = Router::new()
            .route("/login", post(SysAuthenticationApi::login_handler))
            .route("/refreshToken", post(SysAuthenticationApi::refresh_token_handler)); // F13
        Router::new().nest("/auth", router)
    }

    pub async fn init_protected_router() -> Router {
        // F11 extracted-stubs: 加 3 個 stub mount + RouteInfo register（per spec FR-001/002/003、
        // 對齊既有 init_authorization_router pattern；sendCaptcha / verifyCaptcha / error）
        let base_path = "/auth";
        let service_name = "SysAuthenticationApi";

        let routes = vec![
            RouteInfo::new(
                &format!("{}/getUserInfo", base_path),
                Method::GET,
                service_name,
                "获取用户信息",
            ),
            RouteInfo::new(
                &format!("{}/sendCaptcha", base_path),
                Method::POST,
                service_name,
                "发送验证码",
            ),
            RouteInfo::new(
                &format!("{}/verifyCaptcha", base_path),
                Method::POST,
                service_name,
                "验证验证码",
            ),
            RouteInfo::new(
                &format!("{}/error", base_path),
                Method::GET,
                service_name,
                "演示错误回显",
            ),
        ];

        for route in routes {
            add_route(route).await;
        }

        let router = Router::new()
            .route("/getUserInfo", get(SysAuthenticationApi::get_user_info))
            .route("/sendCaptcha", post(SysAuthenticationApi::send_captcha))
            .route("/verifyCaptcha", post(SysAuthenticationApi::verify_captcha))
            .route("/error", get(SysAuthenticationApi::auth_error));

        Router::new().nest(base_path, router)
    }

    pub async fn init_authorization_router() -> Router {
        let base_path = "/authorization";
        let service_name = "SysAuthorizationApi";

        let routes = vec![
            RouteInfo::new(
                &format!("{}/assign-permission", base_path),
                Method::POST,
                service_name,
                "分配权限",
            ),
            RouteInfo::new(
                &format!("{}/assign-routes", base_path),
                Method::POST,
                service_name,
                "分配路由",
            ),
            RouteInfo::new(
                &format!("{}/assign-users", base_path),
                Method::POST,
                service_name,
                "分配用户",
            ),
        ];

        for route in routes {
            add_route(route).await;
        }

        let authorization_router = Router::new()
            .route("/getUserRoutes", get(SysAuthenticationApi::get_user_routes))
            .route(
                "/assign-permission",
                post(SysAuthenticationApi::assign_permission),
            )
            .route("/assign-routes", post(SysAuthenticationApi::assign_routes))
            .route("/assign-users", post(SysAuthenticationApi::assign_users));

        Router::new().nest(base_path, authorization_router)
    }
}
