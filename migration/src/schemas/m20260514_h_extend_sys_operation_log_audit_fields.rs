//! F2.1 audit-log-infrastructure schema extension.
//!
//! 對 `sys_operation_log` 加 4 個結構化欄位（per data-model.md §E1）：
//! - `operation` VARCHAR(20) NOT NULL DEFAULT `'LEGACY'`（既有 row 自動 backfill）
//! - `entity_id` TEXT NULL
//! - `payload_before` JSONB NULL
//! - `payload_after` JSONB NULL
//!
//! 既有 18 欄全保留；新欄由 F2.1 `audit_log::write_in_txn` 填充。

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE sys_operation_log ADD COLUMN operation VARCHAR(20) NOT NULL DEFAULT 'LEGACY'",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_operation_log ADD COLUMN entity_id TEXT NULL")
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE sys_operation_log ADD COLUMN payload_before JSONB NULL",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_operation_log ADD COLUMN payload_after JSONB NULL")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE sys_operation_log DROP COLUMN IF EXISTS payload_after",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE sys_operation_log DROP COLUMN IF EXISTS payload_before",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_operation_log DROP COLUMN IF EXISTS entity_id")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE sys_operation_log DROP COLUMN IF EXISTS operation")
            .await?;
        Ok(())
    }
}
