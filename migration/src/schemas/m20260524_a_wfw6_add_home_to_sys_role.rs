//! W-FW6 role-authorization-completion (US1) schema extension.
//!
//! 對 `sys_role` 加 1 個 nullable 欄位(無預設值),用於持久化「角色首頁路由」,
//! 對齊 W-FW7 m20260523_c_wfw7_add_menu_fields_to_sys_menu.rs 體例:
//! - `home_route_name` VARCHAR NULL — 角色登入後預設導向之路由 `route_name`
//!   (對應 sys_menu.route_name;由 service 層 validate 必為 enabled + non-constant menu)

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_role ADD COLUMN home_route_name VARCHAR NULL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_role DROP COLUMN IF EXISTS home_route_name")
            .await?;
        Ok(())
    }
}
