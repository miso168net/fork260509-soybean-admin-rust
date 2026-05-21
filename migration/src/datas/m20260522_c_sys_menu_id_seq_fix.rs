//! W-FW2 menu-crud-wiring: 修正 `sys_menu_id_seq` sequence desync。
//!
//! 成因：seed migration `m20241024_034744_insert_sys_menu` 以**明確 id** 插入 15 筆菜單
//! （id 最大 72），但未推進 sequence，導致 `sys_menu_id_seq` 的 `last_value` 停在 2。
//! 下一次 INSERT 取得的 id 與既有菜單碰撞、報
//! `duplicate key value violates unique constraint "sys_menu_pkey"`，
//! native `create_menu` 完全無法使用。
//!
//! 此 migration 是 W-FW2 為讓 menu 建立可運作所需的前提修復：
//! 以 `setval` 把 sequence 推進到 `sys_menu` 當前最大 id，
//! 確保後續 `nextval` 取得不碰撞的 id。

use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        let setval_stmt = Statement::from_string(
            manager.get_database_backend(),
            "SELECT setval('sys_menu_id_seq', (SELECT COALESCE(MAX(id), 1) FROM sys_menu))"
                .to_string(),
        );

        db.execute(setval_stmt).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // down 為 no-op：sequence desync 本身是 bug，rollback 沒有有意義的「先前值」可還原。
        // 還原此 migration 不會重新造成碰撞問題，直接回傳 Ok 即可。
        Ok(())
    }
}
