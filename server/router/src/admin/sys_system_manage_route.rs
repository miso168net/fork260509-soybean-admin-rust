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
            RouteInfo::new(
                &format!("{}/addMenu", base_path),
                Method::POST,
                menu_service,
                "新增菜单",
            ),
            RouteInfo::new(
                &format!("{}/updateMenu", base_path),
                Method::POST,
                menu_service,
                "更新菜单",
            ),
            RouteInfo::new(
                &format!("{}/deleteMenu", base_path),
                Method::DELETE,
                menu_service,
                "删除菜单(body id)",
            ),
            RouteInfo::new(
                &format!("{}/batchDeleteMenu", base_path),
                Method::DELETE,
                menu_service,
                "批量删除菜单",
            ),
            RouteInfo::new(
                &format!("{}/addRole", base_path),
                Method::POST,
                role_service,
                "新增角色",
            ),
            RouteInfo::new(
                &format!("{}/updateRole", base_path),
                Method::POST,
                role_service,
                "更新角色",
            ),
            RouteInfo::new(
                &format!("{}/deleteRole", base_path),
                Method::DELETE,
                role_service,
                "删除角色(body id)",
            ),
            RouteInfo::new(
                &format!("{}/batchDeleteRole", base_path),
                Method::DELETE,
                role_service,
                "批量删除角色",
            ),
            RouteInfo::new(
                &format!("{}/getRoleMenuIds/:roleId", base_path),
                Method::GET,
                menu_service,
                "取角色菜单 id 列表",
            ),
            RouteInfo::new(
                &format!("{}/assignRoleMenus", base_path),
                Method::POST,
                role_service,
                "分配角色菜单授权",
            ),
            RouteInfo::new(
                &format!("{}/getRoleHome/:roleId", base_path),
                Method::GET,
                role_service,
                "取角色首页路由",
            ),
            RouteInfo::new(
                &format!("{}/updateRoleHome", base_path),
                Method::POST,
                role_service,
                "更新角色首页路由",
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
            .route("/addUser", post(SysSystemManageApi::add_user_for_systemmanage))
            .route("/updateUser", post(SysSystemManageApi::update_user_for_systemmanage))
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
            )
            .route("/addMenu", post(SysSystemManageApi::add_menu_for_systemmanage))
            .route("/updateMenu", post(SysSystemManageApi::update_menu_for_systemmanage))
            .route("/deleteMenu", delete(SysSystemManageApi::delete_menu_for_systemmanage))
            .route(
                "/batchDeleteMenu",
                delete(SysSystemManageApi::batch_delete_menu_for_systemmanage),
            )
            .route("/addRole", post(SysSystemManageApi::add_role_for_systemmanage))
            .route("/updateRole", post(SysSystemManageApi::update_role_for_systemmanage))
            .route("/deleteRole", delete(SysSystemManageApi::delete_role_for_systemmanage))
            .route(
                "/batchDeleteRole",
                delete(SysSystemManageApi::batch_delete_role_for_systemmanage),
            )
            .route(
                "/getRoleMenuIds/{roleId}",
                get(SysSystemManageApi::get_role_menu_ids_for_systemmanage),
            )
            .route(
                "/assignRoleMenus",
                post(SysSystemManageApi::assign_role_menus_for_systemmanage),
            )
            .route(
                "/getRoleHome/{roleId}",
                get(SysSystemManageApi::get_role_home_for_systemmanage),
            )
            .route(
                "/updateRoleHome",
                post(SysSystemManageApi::update_role_home_for_systemmanage),
            );

        Router::new().nest(Self::BASE_PATH, router)
    }
}
