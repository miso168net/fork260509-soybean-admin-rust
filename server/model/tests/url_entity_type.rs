//! T007 (042 audit-outbox-and-http-mount) — `url_to_entity_type` pure-fn coverage.
//!
//! per data-model.md §E2 Unit-test 涵蓋（lines 215-240）+ spec FR-004 Hybrid rule。
//! 順序：systemManage alias 先、native admin 後、fallback `http_event` 最後。
//!
//! 注意：rust-api 收到的 URL 已被 front-nginx `proxy_pass http://rust_api/`
//! 剝離 `/api/` 前綴。本檔測 path 對應 `/role`、`/auth/login` 等（非 `/api/role`）。
//!
//! TDD：本檔在 `audit_log::url_to_entity_type` 落地前先寫、應 red；fn 落地後 green。

use server_model::admin::audit_log::url_to_entity_type;

#[test]
fn url_entity_type_complete() {
    // systemManage alias — sys_user
    assert_eq!(url_to_entity_type("/systemManage/addUser"), "sys_user");
    assert_eq!(
        url_to_entity_type("/systemManage/updateUser?id=1"),
        "sys_user"
    );

    // systemManage alias — sys_menu
    assert_eq!(url_to_entity_type("/systemManage/addMenu"), "sys_menu");
    assert_eq!(url_to_entity_type("/systemManage/updateMenu"), "sys_menu");
    assert_eq!(
        url_to_entity_type("/systemManage/deleteMenu/123"),
        "sys_menu"
    );
    assert_eq!(
        url_to_entity_type("/systemManage/batchDeleteMenu"),
        "sys_menu"
    );

    // systemManage alias — sys_role
    assert_eq!(url_to_entity_type("/systemManage/addRole"), "sys_role");
    assert_eq!(url_to_entity_type("/systemManage/updateRole"), "sys_role");
    assert_eq!(
        url_to_entity_type("/systemManage/deleteRole/9"),
        "sys_role"
    );
    assert_eq!(
        url_to_entity_type("/systemManage/batchDeleteRole"),
        "sys_role"
    );
    assert_eq!(
        url_to_entity_type("/systemManage/assignRoleMenus"),
        "sys_role"
    );
    assert_eq!(
        url_to_entity_type("/systemManage/updateRoleHome"),
        "sys_role"
    );
    assert_eq!(
        url_to_entity_type("/systemManage/assignRoleEndpoints"),
        "sys_role"
    );

    // native admin router
    assert_eq!(url_to_entity_type("/user"), "sys_user");
    assert_eq!(url_to_entity_type("/user/123"), "sys_user");
    assert_eq!(url_to_entity_type("/role?page=1"), "sys_role");
    assert_eq!(url_to_entity_type("/route/tree"), "sys_menu"); // per 041
    assert_eq!(url_to_entity_type("/domain"), "sys_domain");
    assert_eq!(url_to_entity_type("/organization"), "sys_organization");
    assert_eq!(url_to_entity_type("/api-endpoint"), "sys_endpoint");
    assert_eq!(url_to_entity_type("/access-key"), "sys_access_key");
    assert_eq!(
        url_to_entity_type("/authorization/assign-users"),
        "sys_role"
    );

    // fallback / http_event
    assert_eq!(url_to_entity_type("/auth/login"), "http_event");
    assert_eq!(url_to_entity_type("/auth/changePassword"), "http_event");
    assert_eq!(url_to_entity_type("/auth/getUserInfo"), "http_event");
    assert_eq!(url_to_entity_type("/random/unknown"), "http_event");
}
