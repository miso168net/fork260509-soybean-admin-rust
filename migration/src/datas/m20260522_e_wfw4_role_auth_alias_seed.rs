//! W-FW4 role-authorization-wiring：2 條角色菜單授權 alias endpoint 的 Casbin policy seed。
//! ROLE_SUPER + ROLE_ADMIN allow rules（共 4 rows）、GeneralUser default deny。
//! 涵蓋 getRoleMenuIds（GET）+ assignRoleMenus（POST）。

use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let insert_stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"
            INSERT INTO casbin_rule (ptype, v0, v1, v2, v3, v4, v5)
            VALUES
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getRoleMenuIds/:roleId', 'GET',  '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/assignRoleMenus',        'POST', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getRoleMenuIds/:roleId', 'GET',  '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/assignRoleMenus',        'POST', '', '')
            "#
            .to_string(),
        );

        db.execute(insert_stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let delete_stmt = Statement::from_string(
            manager.get_database_backend(),
            r#"
            DELETE FROM casbin_rule
            WHERE ptype = 'p'
              AND v1 = 'built-in'
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN')
              AND v2 IN (
                '/systemManage/getRoleMenuIds/:roleId',
                '/systemManage/assignRoleMenus'
              )
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
