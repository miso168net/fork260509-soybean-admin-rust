//! W-FW8 button-auth-completion (US1): 3 條 endpoint alias 的 Casbin policy seed。
//! ROLE_SUPER + ROLE_ADMIN allow rules(共 6 rows)、GeneralUser default deny。
//! 涵蓋 getAllEndpoints(GET) + getRoleEndpointIds/:roleId(GET) + assignRoleEndpoints(POST)。

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
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getAllEndpoints',           'GET',  '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getRoleEndpointIds/:roleId', 'GET',  '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/assignRoleEndpoints',       'POST', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getAllEndpoints',           'GET',  '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getRoleEndpointIds/:roleId', 'GET',  '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/assignRoleEndpoints',       'POST', '', '')
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
                '/systemManage/getAllEndpoints',
                '/systemManage/getRoleEndpointIds/:roleId',
                '/systemManage/assignRoleEndpoints'
              )
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
