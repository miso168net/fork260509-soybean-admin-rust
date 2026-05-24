//! 044 W-F12 / US5 (042-N6): pure fn `url_to_entity_type` 從 `server-model::admin::audit_log`
//! 搬移到 `server-core::web`。
//!
//! 動機：HTTP middleware (`server-core::web::operation_log`) 也需要在 build
//! OperationLogContext 時填 module_name（entity_type）；server-core 不能依賴
//! server-model（會與 server-model → server-core 形成 cycle），故將純函式向下
//! 搬到 server-core。`server-model::admin::audit_log` 仍以 `pub use` 再導出、
//! 030+ 既有 callsite 0 改動。

/// Hybrid rule URL → entity_type (per spec 003 Clarifications Q2 + 042 FR-004)。
///
/// 順序：systemManage alias 先（避免 prefix 衝突）、native admin router 後、
/// fallback `http_event` 最後（per data-model.md §E2、unit-tested in
/// `server/model/tests/url_entity_type.rs`）。
///
/// 注意：rust-api 收到的 URL 已被 front-nginx `proxy_pass http://rust_api/`
/// 剝離 `/api/` 前綴。本表 match 對應 `/role`、`/auth/login` 等
/// （非 `/api/role`、`/api/auth/login`）。
pub fn url_to_entity_type(url: &str) -> &'static str {
    // remove query string
    let path = url.split('?').next().unwrap_or(url);

    // systemManage alias — sys_user
    if path.starts_with("/systemManage/addUser") || path.starts_with("/systemManage/updateUser") {
        return "sys_user";
    }
    // systemManage alias — sys_menu
    if path.starts_with("/systemManage/addMenu")
        || path.starts_with("/systemManage/updateMenu")
        || path.starts_with("/systemManage/deleteMenu")
        || path.starts_with("/systemManage/batchDeleteMenu")
    {
        return "sys_menu";
    }
    // systemManage alias — sys_role
    if path.starts_with("/systemManage/addRole")
        || path.starts_with("/systemManage/updateRole")
        || path.starts_with("/systemManage/deleteRole")
        || path.starts_with("/systemManage/batchDeleteRole")
        || path.starts_with("/systemManage/assignRoleMenus")
        || path.starts_with("/systemManage/updateRoleHome")
        || path.starts_with("/systemManage/assignRoleEndpoints")
    {
        return "sys_role";
    }

    // native admin router（per 041：menu 實 mount 在 /route）
    if path.starts_with("/user") {
        return "sys_user";
    }
    if path.starts_with("/role") {
        return "sys_role";
    }
    if path.starts_with("/route") {
        return "sys_menu";
    }
    if path.starts_with("/domain") {
        return "sys_domain";
    }
    if path.starts_with("/organization") {
        return "sys_organization";
    }
    if path.starts_with("/api-endpoint") {
        return "sys_endpoint";
    }
    if path.starts_with("/access-key") {
        return "sys_access_key";
    }
    if path.starts_with("/authorization") {
        return "sys_role";
    }

    // fallback / unknown / /auth/*
    "http_event"
}
