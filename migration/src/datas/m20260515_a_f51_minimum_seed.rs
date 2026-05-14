//! F5.1 minimum seed — 補 `/auth/getUserInfo` + `/route/getUserRoutes` 各 3 roles 的 Casbin
//! p-rules（共 6 rows）。base 既有 m20241024_082926_insert_casbin_rule.rs 只覆蓋 ROLE_SUPER × 31 CRUD
//! paths、未含這 2 個 F5.1 endpoint、F5.1 acceptance test 跑前必須補。
//!
//! per F5.1 spec FR-018 + research.md R5 + tasks.md T008（I1 finding fix）。

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
            ('p', 'ROLE_SUPER', 'built-in', '/auth/getUserInfo',    'GET', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/auth/getUserInfo',    'GET', '', ''),
            ('p', 'ROLE_USER',  'built-in', '/auth/getUserInfo',    'GET', '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/route/getUserRoutes', 'GET', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route/getUserRoutes', 'GET', '', ''),
            ('p', 'ROLE_USER',  'built-in', '/route/getUserRoutes', 'GET', '', '')
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
              AND v2 IN ('/auth/getUserInfo', '/route/getUserRoutes')
              AND v3 = 'GET'
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN', 'ROLE_USER')
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
