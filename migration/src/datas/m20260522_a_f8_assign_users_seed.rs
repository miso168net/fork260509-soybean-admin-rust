//! F8 assign-users Casbin policy seed — 補 ROLE_SUPER 對 /authorization/assign-users
//! POST 的 allow rule(1 row)。per F8 spec FR-006 + FR-007 + brainstorm Q3。
//! 對齊既有 sibling /authorization/assign-permission + assign-routes(都只 ROLE_SUPER)。
//! 沿用 F7 m20260521 既有 raw SQL pattern;v4='' per F11 R-Q5 baseline。

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
            ('p', 'ROLE_SUPER', 'built-in', '/authorization/assign-users', 'POST', '', '')
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
              AND v0 = 'ROLE_SUPER'
              AND v1 = 'built-in'
              AND v2 = '/authorization/assign-users'
              AND v3 = 'POST'
            "#
            .to_string(),
        );
        db.execute(delete_stmt).await?;
        Ok(())
    }
}
