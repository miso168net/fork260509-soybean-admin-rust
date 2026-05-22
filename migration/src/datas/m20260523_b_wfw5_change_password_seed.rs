//! W-FW5 user-role-and-password-wiring：自助改密碼 endpoint 的 Casbin policy seed。
//! `/auth/changePassword`（POST）為自助操作，任何已登入 user 皆可呼叫 →
//! ROLE_SUPER + ROLE_ADMIN + ROLE_USER 三 role allow（共 3 rows）。
//! 比照 m20260515_a_f51_minimum_seed 的 `/auth/getUserInfo` 3-role seed 體例。

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
            ('p', 'ROLE_SUPER', 'built-in', '/auth/changePassword', 'POST', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/auth/changePassword', 'POST', '', ''),
            ('p', 'ROLE_USER',  'built-in', '/auth/changePassword', 'POST', '', '')
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
              AND v2 = '/auth/changePassword'
              AND v3 = 'POST'
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN', 'ROLE_USER')
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
