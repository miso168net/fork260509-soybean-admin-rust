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
                    .table(SysUser::Table)
                    .add_column(
                        ColumnDef::new(SysUser::DeletedAt)
                            .timestamp()
                            .null()
                            .comment("软删除时间"),
                    )
                    .to_owned(),
            )
            .await?;

        // Step 2: DROP 既有 UNIQUE constraint (postgres 預設命名 <table>_<column>_key)
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_user DROP CONSTRAINT IF EXISTS sys_user_username_key",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_user DROP CONSTRAINT IF EXISTS sys_user_email_key",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_user DROP CONSTRAINT IF EXISTS sys_user_phone_number_key",
        )
        .await?;

        // 既有 m20240815_082854_create_sys_user 在 create_table 之後另建了
        // unique index `idx_sys_user_username`,一併 DROP 避免與 partial index 重複
        conn.execute_unprepared("DROP INDEX IF EXISTS idx_sys_user_username")
            .await?;

        // Step 3: CREATE partial UNIQUE INDEX × 3
        manager
            .create_index(
                Index::create()
                    .name("sys_user_username_active_uidx")
                    .table(SysUser::Table)
                    .col(SysUser::Username)
                    .unique()
                    .and_where(Expr::col(SysUser::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("sys_user_email_active_uidx")
                    .table(SysUser::Table)
                    .col(SysUser::Email)
                    .unique()
                    .and_where(Expr::col(SysUser::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("sys_user_phone_number_active_uidx")
                    .table(SysUser::Table)
                    .col(SysUser::PhoneNumber)
                    .unique()
                    .and_where(Expr::col(SysUser::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 反向 Step 3: DROP partial unique index
        manager
            .drop_index(
                Index::drop()
                    .name("sys_user_phone_number_active_uidx")
                    .table(SysUser::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("sys_user_email_active_uidx")
                    .table(SysUser::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("sys_user_username_active_uidx")
                    .table(SysUser::Table)
                    .to_owned(),
            )
            .await?;

        // 反向 Step 2: 還原 UNIQUE constraint (含原本的 idx_sys_user_username index)
        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_user ADD CONSTRAINT sys_user_username_key UNIQUE (username)",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_user ADD CONSTRAINT sys_user_email_key UNIQUE (email)",
        )
        .await?;
        conn.execute_unprepared(
            "ALTER TABLE sys_user ADD CONSTRAINT sys_user_phone_number_key UNIQUE (phone_number)",
        )
        .await?;
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX idx_sys_user_username ON sys_user (username)",
        )
        .await?;

        // 反向 Step 1: DROP deleted_at column
        manager
            .alter_table(
                Table::alter()
                    .table(SysUser::Table)
                    .drop_column(SysUser::DeletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SysUser {
    Table,
    Username,
    Email,
    PhoneNumber,
    DeletedAt,
}
