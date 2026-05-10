use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Step 1: add column as nullable
        manager
            .alter_table(
                Table::alter()
                    .table(SysTokens::Table)
                    .add_column(ColumnDef::new(SysTokens::ExpiresAt).timestamp().null())
                    .to_owned(),
            )
            .await?;

        // Step 2: backfill via raw SQL (DEFAULT can't reference other columns)
        let db = manager.get_connection();
        let backfill_stmt = Statement::from_string(
            manager.get_database_backend(),
            "UPDATE sys_tokens SET expires_at = created_at + INTERVAL '14 days' WHERE expires_at IS NULL".to_string(),
        );
        db.execute(backfill_stmt).await?;

        // Step 3: alter to NOT NULL
        manager
            .alter_table(
                Table::alter()
                    .table(SysTokens::Table)
                    .modify_column(ColumnDef::new(SysTokens::ExpiresAt).timestamp().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SysTokens::Table)
                    .drop_column(SysTokens::ExpiresAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SysTokens {
    Table,
    ExpiresAt,
}
