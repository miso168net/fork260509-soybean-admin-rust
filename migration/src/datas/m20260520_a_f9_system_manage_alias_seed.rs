//! F9 systemManage-alias-router Casbin policy seed — 補 10 條 /systemManage/* alias endpoint 的
//! ROLE_SUPER + ROLE_ADMIN allow rules（共 20 rows）、GeneralUser default deny。
//! per F9 spec FR-008 + FR-009 + brainstorm Q1 + R-Q6 沿用 F11 R-Q5 v4='' baseline。
//! 沿用 F11 m20260519 既有 pattern：Casbin model `p = sub, dom, obj, act` 4 field、
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
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getRoleList',       'GET',    '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getAllRoles',       'GET',    '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getUserList',       'GET',    '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/addUser',           'POST',   '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/updateUser',        'POST',   '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/deleteUser',        'DELETE', '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/batchDeleteUser',   'DELETE', '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getMenuList/v2',    'GET',    '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getAllPages',       'GET',    '', ''),
            ('p', 'ROLE_SUPER', 'built-in', '/systemManage/getMenuTree',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getRoleList',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getAllRoles',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getUserList',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/addUser',           'POST',   '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/updateUser',        'POST',   '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/deleteUser',        'DELETE', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/batchDeleteUser',   'DELETE', '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getMenuList/v2',    'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getAllPages',       'GET',    '', ''),
            ('p', 'ROLE_ADMIN', 'built-in', '/systemManage/getMenuTree',       'GET',    '', '')
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
              AND v2 LIKE '/systemManage/%'
              AND v0 IN ('ROLE_SUPER', 'ROLE_ADMIN')
            "#
            .to_string(),
        );

        db.execute(delete_stmt).await?;

        Ok(())
    }
}
