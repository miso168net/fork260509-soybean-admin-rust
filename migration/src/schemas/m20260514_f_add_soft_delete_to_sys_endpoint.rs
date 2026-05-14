use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // sys_endpoint 無 UNIQUE constraint(除 PK),僅加 deleted_at column
        manager
            .alter_table(
                Table::alter()
                    .table(SysEndpoint::Table)
                    .add_column(
                        ColumnDef::new(SysEndpoint::DeletedAt)
                            .timestamp()
                            .null()
                            .comment("软删除时间"),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SysEndpoint::Table)
                    .drop_column(SysEndpoint::DeletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SysEndpoint {
    Table,
    DeletedAt,
}
