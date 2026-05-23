//! W-FW7 menu-field-persistence schema extension.
//!
//! 對 `sys_menu` 加 3 個 nullable 欄位(無預設值),用於把先前 base-web 端
//! 已維護但未持久化的 menu meta 落到資料庫:
//! - `query`              JSONB    NULL — 路由 query 參數(陣列 of {key,value})
//! - `buttons`            JSONB    NULL — 按鈕權限清單(陣列 of {code,desc})
//! - `fixed_index_in_tab` INTEGER  NULL — 固定 tab 順序索引

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu ADD COLUMN query JSONB NULL")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu ADD COLUMN buttons JSONB NULL")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu ADD COLUMN fixed_index_in_tab INTEGER NULL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu DROP COLUMN IF EXISTS fixed_index_in_tab")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu DROP COLUMN IF EXISTS buttons")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_menu DROP COLUMN IF EXISTS query")
            .await?;
        Ok(())
    }
}
