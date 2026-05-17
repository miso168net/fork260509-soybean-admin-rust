//! F6 minimum seed — 補 `/route/isRouteExist` 3 roles 的 Casbin p-rules（共 3 rows）。
//! per F6 spec FR-005 / FR-014 + research.md R-2（Casbin allowlist mode → 未顯式 allow
//! 等於 default deny；F6 endpoint 必須補 3 row 對 SUPER/ADMIN/USER 各 1 允許條目）。
//! Mirror F5.1 既有 m20260515_a_f51_minimum_seed.rs pattern。

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
            ('p', 'ROLE_SUPER', 'built-in', '/route/isRouteExist', 'GET', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route/isRouteExist', 'GET', '', ''),
            ('p', 'ROLE_USER',  'built-in', '/route/isRouteExist', 'GET', '', '')
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
              AND v2 = '/route/isRouteExist'
              AND v3 = 'GET'
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN', 'ROLE_USER')
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
