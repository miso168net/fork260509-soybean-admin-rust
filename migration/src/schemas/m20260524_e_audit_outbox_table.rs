//! 042 audit-outbox-and-http-mount: 新增 sys_audit_outbox 表為 audit event 耐久暫存區
//! per spec 042 FR-002、SC-009、data-model.md §E1。
//!
//! Schema (6 columns):
//! - id BIGSERIAL PK
//! - audit_event_json JSONB NOT NULL
//! - published_at TIMESTAMPTZ NULL  (set after drainer publishes to Redis Stream + sys_operation_log)
//! - retry_count INTEGER NOT NULL DEFAULT 0
//! - last_error TEXT NULL
//! - created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
//!
//! Partial index `idx_sys_audit_outbox_pending` 加速 drainer 撈未處理 row。

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE sys_audit_outbox (
                    id BIGSERIAL PRIMARY KEY,
                    audit_event_json JSONB NOT NULL,
                    published_at TIMESTAMPTZ NULL,
                    retry_count INTEGER NOT NULL DEFAULT 0,
                    last_error TEXT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );
                CREATE INDEX idx_sys_audit_outbox_pending
                    ON sys_audit_outbox (id)
                    WHERE published_at IS NULL;
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sys_audit_outbox;")
            .await?;
        Ok(())
    }
}
