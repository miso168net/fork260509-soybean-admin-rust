//! F9 systemManage-alias-router: 10 條 /systemManage/* alias router mount。
//! per F9 spec FR-001~FR-007 + R-Q1 (updateUser 直接 mount 既有 handler with POST)、
//! 對齊 sys_authentication_route.rs init_protected_router pattern (`let base_path = "..."` + `add_route(route).await`)。

use axum::{
    http::Method,
    routing::{delete, get, post},
    Router,
};
use server_api::admin::{SysMenuApi, SysSystemManageApi, SysUserApi};
use server_global::global::{add_route, RouteInfo};

pub struct SysSystemManageRouter;

impl SysSystemManageRouter {
    const BASE_PATH: &'static str = "/systemManage";

    pub async fn init_router() -> Router {
        let base_path = Self::BASE_PATH;
        let user_service = "SysUserApi";
        let role_service = "SysRoleApi";
        let menu_service = "SysMenuApi";

        let routes = vec![
            RouteInfo::new(
                &format!("{}/getRoleList", base_path),
                Method::GET,
                role_service,
                "角色分页列表",
            ),
            RouteInfo::new(
                &format!("{}/getAllRoles", base_path),
                Method::GET,
                role_service,
                "所有启用角色",
            ),
            RouteInfo::new(
                &format!("{}/getUserList", base_path),
                Method::GET,
                user_service,
                "用户分页列表",
            ),
            RouteInfo::new(
                &format!("{}/addUser", base_path),
                Method::POST,
                user_service,
                "新增用户",
            ),
            RouteInfo::new(
                &format!("{}/updateUser", base_path),
                Method::POST,
                user_service,
                "更新用户(POST alias)",
            ),
            RouteInfo::new(
                &format!("{}/deleteUser", base_path),
                Method::DELETE,
                user_service,
                "删除用户(body id)",
            ),
            RouteInfo::new(
                &format!("{}/batchDeleteUser", base_path),
                Method::DELETE,
                user_service,
                "批量删除用户",
            ),
            RouteInfo::new(
                &format!("{}/getMenuList/v2", base_path),
                Method::GET,
                menu_service,
                "菜单列表 v2",
            ),
            RouteInfo::new(
                &format!("{}/getAllPages", base_path),
                Method::GET,
                menu_service,
                "所有页面 key",
            ),
            RouteInfo::new(
                &format!("{}/getMenuTree", base_path),
                Method::GET,
                menu_service,
                "菜单树",
            ),
        ];

        for route in routes {
            add_route(route).await;
        }

        let router = Router::new()
            .route(
                "/getRoleList",
                get(SysSystemManageApi::list_roles_for_systemmanage),
            )
            .route(
                "/getAllRoles",
                get(SysSystemManageApi::list_all_roles_for_systemmanage),
            )
            .route(
                "/getUserList",
                get(SysSystemManageApi::list_users_for_systemmanage),
            )
            .route("/addUser", post(SysUserApi::create_user))
            .route("/updateUser", post(SysUserApi::update_user))
            .route("/deleteUser", delete(SysUserApi::delete_user_by_body))
            .route("/batchDeleteUser", delete(SysUserApi::batch_delete_users))
            .route(
                "/getMenuList/v2",
                get(SysSystemManageApi::list_menu_for_systemmanage),
            )
            .route("/getAllPages", get(SysMenuApi::get_all_pages))
            .route(
                "/getMenuTree",
                get(SysSystemManageApi::tree_menu_for_systemmanage),
            );

        Router::new().nest(Self::BASE_PATH, router)
    }
}
