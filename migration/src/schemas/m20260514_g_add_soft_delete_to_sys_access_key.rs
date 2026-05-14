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
                    .table(SysAccessKey::Table)
                    .add_column(
                        ColumnDef::new(SysAccessKey::DeletedAt)
                            .timestamp()
                            .null()
                            .comment("软删除时间"),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 2: DROP 既有 UNIQUE constraint × 2
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_access_key DROP CONSTRAINT IF EXISTS sys_access_key_access_key_id_key",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_access_key DROP CONSTRAINT IF EXISTS sys_access_key_access_key_secret_key",
        )
        .await?;

        // Step 3: CREATE partial UNIQUE INDEX × 2
        manager
            .create_index(
                Index::create()
                    .name("sys_access_key_access_key_id_active_uidx")
                    .table(SysAccessKey::Table)
                    .col(SysAccessKey::AccessKeyId)
                    .unique()
                    .and_where(Expr::col(SysAccessKey::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("sys_access_key_access_key_secret_active_uidx")
                    .table(SysAccessKey::Table)
                    .col(SysAccessKey::AccessKeySecret)
                    .unique()
                    .and_where(Expr::col(SysAccessKey::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("sys_access_key_access_key_secret_active_uidx")
                    .table(SysAccessKey::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("sys_access_key_access_key_id_active_uidx")
                    .table(SysAccessKey::Table)
                    .to_owned(),
            )
            .await?;

        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_access_key ADD CONSTRAINT sys_access_key_access_key_id_key UNIQUE (access_key_id)",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_access_key ADD CONSTRAINT sys_access_key_access_key_secret_key UNIQUE (access_key_secret)",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SysAccessKey::Table)
                    .drop_column(SysAccessKey::DeletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SysAccessKey {
    Table,
    AccessKeyId,
    AccessKeySecret,
    DeletedAt,
}
