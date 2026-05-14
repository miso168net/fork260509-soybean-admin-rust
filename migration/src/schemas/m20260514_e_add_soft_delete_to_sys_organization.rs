use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Step 1: 加 deleted_at column
        manager
            .alter_table(
                Table::alter()
                    .table(SysOrganization::Table)
                    .add_column(
                        ColumnDef::new(SysOrganization::DeletedAt)
                            .timestamp()
                            .null()
                            .comment("软删除时间"),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 2: DROP 既有 UNIQUE constraint
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_organization DROP CONSTRAINT IF EXISTS sys_organization_code_key",
        )
        .await?;

        // Step 3: CREATE partial UNIQUE INDEX
        manager
            .create_index(
                Index::create()
                    .name("sys_organization_code_active_uidx")
                    .table(SysOrganization::Table)
                    .col(SysOrganization::Code)
                    .unique()
                    .and_where(Expr::col(SysOrganization::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("sys_organization_code_active_uidx")
                    .table(SysOrganization::Table)
                    .to_owned(),
            )
            .await?;

        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_organization ADD CONSTRAINT sys_organization_code_key UNIQUE (code)",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SysOrganization::Table)
                    .drop_column(SysOrganization::DeletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SysOrganization {
    Table,
    Code,
    DeletedAt,
}
