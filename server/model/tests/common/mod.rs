//! F3 acceptance test 共用 helper —— DB connection + actor + ulid 工具。
//!
//! 此 mod 被 `soft_delete_*.rs` 三個 integration test 檔 `mod common;` 引入。
//! 所有 test 透過 `#[ignore]` 標註、需 export `TEST_DATABASE_URL` 並先跑 migration up 才會跑。

use std::sync::Arc;

use sea_orm::{Database, DatabaseConnection};
use server_core::web::audit::Actor;
use ulid::Ulid;

/// 取 test DB 連線 URL — fallback default 為本機 postgres + new_admin_test DB。
pub fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:123456@127.0.0.1:5432/new_admin_test".to_string()
    })
}

/// 連線到 test DB；連不上即 panic（要求外部 export `TEST_DATABASE_URL` + migration up）。
pub async fn connect() -> Arc<DatabaseConnection> {
    Arc::new(
        Database::connect(&test_db_url())
            .await
            .expect("test DB connect failed — set TEST_DATABASE_URL or 起 postgres + migration up"),
    )
}

/// 生成測試專用 `Actor`（id / username 帶 ulid 後綴避免跨 test 衝突）。
pub fn test_actor(prefix: &str) -> Actor {
    let suffix = Ulid::new().to_string();
    Actor {
        id: format!("{}_{}", prefix, suffix),
        username: format!("{}_{}", prefix, suffix),
        domain: "default".to_string(),
    }
}
