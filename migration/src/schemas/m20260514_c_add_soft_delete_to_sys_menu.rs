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
                    .table(SysMenu::Table)
                    .add_column(
                        ColumnDef::new(SysMenu::DeletedAt)
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
            "ALTER TABLE sys_menu DROP CONSTRAINT IF EXISTS sys_menu_route_name_key",
        )
        .await?;

        // Step 3: CREATE partial UNIQUE INDEX
        manager
            .create_index(
                Index::create()
                    .name("sys_menu_route_name_active_uidx")
                    .table(SysMenu::Table)
                    .col(SysMenu::RouteName)
                    .unique()
                    .and_where(Expr::col(SysMenu::DeletedAt).is_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("sys_menu_route_name_active_uidx")
                    .table(SysMenu::Table)
                    .to_owned(),
            )
            .await?;

        let conn = manager.get_connection();
        conn.execute_unprepared(
            "ALTER TABLE sys_menu ADD CONSTRAINT sys_menu_route_name_key UNIQUE (route_name)",
        )
        .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SysMenu::Table)
                    .drop_column(SysMenu::DeletedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SysMenu {
    Table,
    RouteName,
    DeletedAt,
}
