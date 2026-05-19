//! F11 extracted-stubs Casbin policy seed — 補 4 條抽離項 stub endpoint 的
//! ROLE_SUPER + ROLE_ADMIN allow rules（共 8 rows）、GeneralUser default deny。
//! per F11 spec FR-005 + FR-006 + brainstorm Q1。
//! 沿用 F5.1 / F6 既有 pattern：Casbin model `p = sub, dom, obj, act` 4 field、
//! v4 留空字串為 implicit allow（v4='allow' 顯式會與 model 衝突致 enforce error）。

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
            ('p', 'ROLE_SUPER', 'built-in', '/auth/sendCaptcha',   'POST', '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/auth/verifyCaptcha', 'POST', '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/auth/error',         'GET',  '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/mock/getLastTime',   'GET',  '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/auth/sendCaptcha',   'POST', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/auth/verifyCaptcha', 'POST', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/auth/error',         'GET',  '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/mock/getLastTime',   'GET',  '', '')
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
              AND v2 IN ('/auth/sendCaptcha', '/auth/verifyCaptcha', '/auth/error', '/mock/getLastTime')
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN')
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
