pub use sea_orm_migration::prelude::*;

mod datas;
mod schemas;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            // 架构迁移
            Box::new(schemas::m20240815_082808_create_enum_status::Migration),
            Box::new(schemas::m20240815_082854_create_sys_user::Migration),
            Box::new(schemas::m20241023_091143_create_sys_menu::Migration),
            Box::new(schemas::m20241023_091155_create_sys_organization::Migration),
            Box::new(schemas::m20241023_091109_create_sys_access_key::Migration),
            Box::new(schemas::m20241023_091115_create_sys_domain::Migration),
            Box::new(schemas::m20241023_091132_create_sys_endpoint::Migration),
            Box::new(schemas::m20241023_091138_create_sys_login_log::Migration),
            Box::new(schemas::m20241023_091149_create_sys_operation_log::Migration),
            Box::new(schemas::m20241023_090604_create_sys_role::Migration),
            Box::new(schemas::m20241023_091204_create_sys_tokens::Migration),
            Box::new(schemas::m20241023_091210_create_sys_user_role::Migration),
            Box::new(schemas::m20241023_091159_create_sys_role_menu::Migration),
            // F3 soft-delete-infrastructure: 7 entity 加 deleted_at + partial unique index
            Box::new(schemas::m20260514_a_add_soft_delete_to_sys_user::Migration),
            Box::new(schemas::m20260514_b_add_soft_delete_to_sys_role::Migration),
            Box::new(schemas::m20260514_c_add_soft_delete_to_sys_menu::Migration),
            Box::new(schemas::m20260514_d_add_soft_delete_to_sys_domain::Migration),
            Box::new(schemas::m20260514_e_add_soft_delete_to_sys_organization::Migration),
            Box::new(schemas::m20260514_f_add_soft_delete_to_sys_endpoint::Migration),
            Box::new(schemas::m20260514_g_add_soft_delete_to_sys_access_key::Migration),
            // F2.1 audit-log-infrastructure: sys_operation_log 加 4 結構化欄位
            Box::new(schemas::m20260514_h_extend_sys_operation_log_audit_fields::Migration),
            // 数据迁移
            Box::new(datas::m20241023_102950_insert_sys_domain::Migration),
            Box::new(datas::m20241024_033005_insert_sys_user::Migration),
            Box::new(datas::m20241024_034526_insert_sys_role::Migration),
            Box::new(datas::m20241024_034744_insert_sys_menu::Migration),
            Box::new(datas::m20241024_033933_insert_sys_user_role::Migration),
            Box::new(datas::m20241024_034305_insert_sys_role_menu::Migration),
            Box::new(datas::m20241024_082926_insert_casbin_rule::Migration),
            // F5.1 auth-login-and-dynamic-menu: minimum Casbin policy seed
            Box::new(datas::m20260515_a_f51_minimum_seed::Migration),
            // F6 route-guard: isRouteExist Casbin policy seed
            Box::new(datas::m20260518_a_f6_is_route_exist_seed::Migration),
            // F11 extracted-stubs: 4 條 stub endpoint Casbin policy seed
            Box::new(datas::m20260519_a_f11_extracted_stubs_seed::Migration),
            // F9 systemManage-alias-router: 10 條 alias endpoint Casbin policy seed(20 row)
            Box::new(datas::m20260520_a_f9_system_manage_alias_seed::Migration),
            // F7 manage-crud-alignment: ROLE_ADMIN 對既有 /user/* /role/* /route/* path 15 row Casbin policy seed
            Box::new(datas::m20260521_a_f7_admin_role_existing_paths_seed::Migration),
            // F8 assign-users: ROLE_SUPER 對 /authorization/assign-users POST 1 row Casbin policy seed
            Box::new(datas::m20260522_a_f8_assign_users_seed::Migration),
            // F030 US2 gender-display-filter: sys_user.gender 欄位 + PG enum + seed
            Box::new(datas::m20260523_a_030_user_gender::Migration),
            // W-FW2 menu-crud-wiring: 4 條 menu 寫入 alias Casbin policy seed(8 row)
            Box::new(datas::m20260522_b_wfw2_menu_alias_seed::Migration),
            // W-FW2 menu-crud-wiring: 修正 sys_menu_id_seq sequence desync(setval 至 MAX(id))
            Box::new(datas::m20260522_c_sys_menu_id_seq_fix::Migration),
            // W-FW3 role-crud-wiring: 4 條 role 寫入 alias Casbin policy seed(8 row)
            Box::new(datas::m20260522_d_wfw3_role_alias_seed::Migration),
            // W-FW4 role-authorization-wiring: 2 條角色菜單授權 alias Casbin policy seed(4 row)
            Box::new(datas::m20260522_e_wfw4_role_auth_alias_seed::Migration),
            // W-FW5 user-role-and-password-wiring: /auth/changePassword 3-role Casbin policy seed(3 row)
            Box::new(datas::m20260523_b_wfw5_change_password_seed::Migration),
            // W-FW7 menu-field-persistence: sys_menu 加 query/buttons (JSONB) + fixed_index_in_tab (INTEGER) 3 個 nullable 欄位
            Box::new(schemas::m20260523_c_wfw7_add_menu_fields_to_sys_menu::Migration),
        ]
    }
}
