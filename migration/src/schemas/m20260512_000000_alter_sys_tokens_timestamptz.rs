use sea_orm_migration::{prelude::*, sea_orm::Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // FR-002: `USING ... AT TIME ZONE 'Asia/Taipei'` 對齊 deploy/.env 的 TZ 設定
        // （per Phase 0 R1）。既有 naive 值是 admin-api 以 Asia/Taipei 本地時間寫入的，
        // 故 `AT TIME ZONE 'Asia/Taipei'` 正確把它們標記為台北時間、轉成 UTC instant
        // 後存進 timestamptz，真實 instant 不偏移。
        // sea-orm `modify_column` API 不支援 USING clause，因此走 raw SQL。
        let alter_stmts = [
            "ALTER TABLE sys_tokens ALTER COLUMN expires_at TYPE TIMESTAMPTZ USING expires_at AT TIME ZONE 'Asia/Taipei'",
            "ALTER TABLE sys_tokens ALTER COLUMN created_at TYPE TIMESTAMPTZ USING created_at AT TIME ZONE 'Asia/Taipei'",
            "ALTER TABLE sys_tokens ALTER COLUMN login_time TYPE TIMESTAMPTZ USING login_time AT TIME ZONE 'Asia/Taipei'",
        ];

        for sql in alter_stmts {
            db.execute(Statement::from_string(backend, sql.to_string()))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // NOTE (per spec Edge Cases + Phase 0 R8): down 不帶 USING clause，
        // Postgres 會用當下 session TZ 投影 timestamptz → naive timestamp。
        // 若 session TZ ≠ Asia/Taipei，instant 會偏移 → 等同資料毀損。
        //
        // 因此 down 僅供 dev 緊急回退使用（且只能在 session TZ = Asia/Taipei 時跑）；
        // prod 環境若需修正，應採 fix-forward 策略（再寫一個 corrective migration）、
        // 不應執行 down。
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        let alter_stmts = [
            "ALTER TABLE sys_tokens ALTER COLUMN expires_at TYPE TIMESTAMP",
            "ALTER TABLE sys_tokens ALTER COLUMN created_at TYPE TIMESTAMP",
            "ALTER TABLE sys_tokens ALTER COLUMN login_time TYPE TIMESTAMP",
        ];

        for sql in alter_stmts {
            db.execute(Statement::from_string(backend, sql.to_string()))
                .await?;
        }
        Ok(())
    }
}
