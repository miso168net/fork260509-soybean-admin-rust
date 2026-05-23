//! 039 rust-entity-id-numeric-migration (A3 schema)：5 業務 entity 加 display_id BIGINT 副欄 + INDEX。
//!
//! 對 sys_user / sys_role / sys_endpoint / sys_organization / sys_access_key 各：
//! - ADD COLUMN `display_id BIGINT NOT NULL DEFAULT 0`（DEFAULT 0 為過渡，A4 backfill 後 DROP）
//! - CREATE INDEX `idx_<table>_display_id`（non-UNIQUE，UNIQUE 於 A4 backfill 完成後再加）
//!
//! 對稱 down：DROP INDEX + DROP COLUMN，順序逆轉。

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLES: &[&str] = &[
    "sys_user",
    "sys_role",
    "sys_endpoint",
    "sys_organization",
    "sys_access_key",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in TABLES {
            let add_col_sql = format!(
                "ALTER TABLE {} ADD COLUMN display_id BIGINT NOT NULL DEFAULT 0",
                table
            );
            db.execute_unprepared(&add_col_sql).await?;

            let create_idx_sql = format!(
                "CREATE INDEX idx_{}_display_id ON {} (display_id)",
                table, table
            );
            db.execute_unprepared(&create_idx_sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in TABLES {
            let drop_idx_sql = format!("DROP INDEX IF EXISTS idx_{}_display_id", table);
            db.execute_unprepared(&drop_idx_sql).await?;

            let drop_col_sql =
                format!("ALTER TABLE {} DROP COLUMN IF EXISTS display_id", table);
            db.execute_unprepared(&drop_col_sql).await?;
        }
        Ok(())
    }
}
