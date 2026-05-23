//! 039 rust-entity-id-numeric-migration (A4 backfill)：對 5 entity 既有 row UPDATE display_id 為 Snowflake i64。
//!
//! 對 sys_user / sys_role / sys_endpoint / sys_organization / sys_access_key 各：
//! 1. SELECT id FROM <table>，逐 row UPDATE SET display_id = next_display_id() WHERE id = $2
//! 2. 完成後 ALTER COLUMN display_id DROP DEFAULT + ADD CONSTRAINT uq_<table>_display_id UNIQUE
//!
//! down：DROP CONSTRAINT IF EXISTS uq_<table>_display_id（DROP DEFAULT 不逆轉，design 已接受此不對稱）。

use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, Statement},
};
use server_global::snowflake::next_display_id;

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
        let backend = manager.get_database_backend();

        for table in TABLES {
            // 1. SELECT 既有 row id（VARCHAR ULID PK）
            let select_sql = format!("SELECT id FROM {}", table);
            let rows = db
                .query_all(Statement::from_string(backend, select_sql))
                .await?;

            // 2. 逐 row UPDATE display_id = Snowflake i64
            let update_sql = format!("UPDATE {} SET display_id = $1 WHERE id = $2", table);
            for row in rows {
                let id: String = row
                    .try_get("", "id")
                    .map_err(|e| DbErr::Custom(e.to_string()))?;
                let display_id = next_display_id();
                db.execute(Statement::from_sql_and_values(
                    backend,
                    &update_sql,
                    vec![display_id.into(), id.into()],
                ))
                .await?;
            }

            // 3. DROP DEFAULT + ADD UNIQUE CONSTRAINT
            // 拆 2 條 execute_unprepared：sea-orm Statement::from_string 走 prepared 協定、
            // PG 不允許多 statement 同 prepare（"cannot insert multiple commands into a prepared statement"）。
            db.execute_unprepared(&format!(
                "ALTER TABLE {table} ALTER COLUMN display_id DROP DEFAULT"
            ))
            .await?;
            db.execute_unprepared(&format!(
                "ALTER TABLE {table} ADD CONSTRAINT uq_{table}_display_id UNIQUE (display_id)"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        for table in TABLES {
            let drop_sql = format!(
                "ALTER TABLE {table} DROP CONSTRAINT IF EXISTS uq_{table}_display_id;",
                table = table
            );
            db.execute(Statement::from_string(backend, drop_sql)).await?;
        }
        Ok(())
    }
}
