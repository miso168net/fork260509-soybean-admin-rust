//! F7 manage-crud-alignment Casbin policy seed — 補 ROLE_ADMIN 對既有 /user/* /role/* /route/*
//! path 的 allow rules(共 15 rows)、解 spec A-006 已知差異(F9 baseline)。
//! per F7 spec FR-011 + FR-012 + FR-013 + brainstorm Q3 + F11 R-Q5 v4='' baseline。
//! 沿用 F9 m20260520 既有 pattern。
//!
//! F7 implement-time spec drift refinement: spec FR-012 + data-model E8 寫 base path
//! `/user/` `/role/` `/route/` (trailing slash)、實際 axum router mount + m20241024
//! ROLE_SUPER baseline 用 `/user` `/role` `/route` (無 trailing slash)、修齊。

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
            ('p', 'ROLE_ADMIN', 'built-in', '/user',           'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/user/users',     'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/user',           'POST',   '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/user',           'PUT',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/user/:id',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/user/:id',       'DELETE', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/role',           'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/role',           'POST',   '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/role',           'PUT',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/role/:id',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/role/:id',       'DELETE', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route',          'POST',   '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route',          'PUT',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route/:id',      'DELETE', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/route/:id',      'GET',    '', '')
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
              AND v0 = 'ROLE_ADMIN'
              AND v1 = 'built-in'
              AND (v2 LIKE '/user%' OR v2 LIKE '/role%' OR v2 LIKE '/route%')
              AND v2 NOT LIKE '/systemManage/%'
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
