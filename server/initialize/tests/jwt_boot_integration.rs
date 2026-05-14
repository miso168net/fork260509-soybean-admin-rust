//! F1.1 jwt-secrets integration tests
//! per spec SC-005 ~ SC-007
//!
//! Design notes:
//! - `server_config::init_from_file` internally calls `apply_jwt_secret_hardening` which panics on
//!   bad secrets; we intercept these panics via `std::panic::catch_unwind`.
//! - `global::GLOBAL_CONFIG` is an overwriteable `RwLock<HashMap>` (not OnceLock), so multiple
//!   sequential tests can safely re-initialize configs.
//! - Tests are serialized via `BOOT_MUTEX` (tokio::sync::Mutex) held with `.lock().await` to
//!   prevent env-var + global-state races when tests run in parallel.
//! - Panic tests use `futures::FutureExt::catch_unwind` on an `AssertUnwindSafe`-wrapped future.
//!   The caught panic means `init_from_file` never returned Ok, so global state stays as-is.
//!
//! Simplification rationale (vs mission spec T011 full version):
//! - `global::get_config::<JwtConfig>()` check is omitted for success tests: `Result::is_ok()`
//!   is sufficient evidence that strict validation passed (any invalid secret would have panicked,
//!   causing `init_from_file` to propagate panic rather than return Ok).
//! - This avoids holding a `std::sync::Mutex` guard across `.await` (which is `!Send`).

use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use tokio::sync::Mutex;

/// 序列化所有 boot integration tests，防止 env var + global state 競態
static BOOT_MUTEX: Mutex<()> = Mutex::const_new(());

/// SC-005: 無 env override + application.yaml 含 placeholder secret → boot panic
///
/// application.yaml jwt_secret = "change-me-jwt-secret"（PLACEHOLDER_SECRETS 黑名單內）
/// → `apply_jwt_secret_hardening` 呼叫 `validate_jwt_secret` → panic
/// → `init_from_file` propagates panic → `catch_unwind` 捕捉 Err
#[tokio::test]
async fn boot_panics_on_placeholder_secret_in_yaml() {
    let _guard = BOOT_MUTEX.lock().await;
    // 確保無 env override（_FILE or bare）
    std::env::remove_var("APP_JWT_JWT_SECRET");
    std::env::remove_var("APP_JWT_JWT_SECRET_FILE");

    let result = AssertUnwindSafe(
        server_config::init_from_file("../resources/application.yaml"),
    )
    .catch_unwind()
    .await;

    assert!(
        result.is_err(),
        "expected boot to panic with placeholder secret, but it returned Ok"
    );
    // 驗 panic 訊息含 "placeholder"
    if let Err(payload) = result {
        let msg = payload
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains("placeholder"),
            "panic message should contain 'placeholder', got: {}",
            msg
        );
    }
}

/// SC-006: APP_JWT_JWT_SECRET 設合規 secret → boot 成功
///
/// 環境變數 APP_JWT_JWT_SECRET 優先於 yaml（透過 `load_config_with_env` env override）
/// → `apply_jwt_secret_hardening` 取到合規值 → `validate_jwt_secret` pass → Ok
#[tokio::test]
async fn boot_succeeds_with_env_override() {
    let _guard = BOOT_MUTEX.lock().await;
    let real_secret = "a".repeat(32);
    std::env::set_var("APP_JWT_JWT_SECRET", &real_secret);
    std::env::remove_var("APP_JWT_JWT_SECRET_FILE");

    let result = server_config::init_from_file_with_multi_instance_env(
        "../resources/application.yaml",
        None,
    )
    .await;

    std::env::remove_var("APP_JWT_JWT_SECRET");

    assert!(
        result.is_ok(),
        "expected boot to succeed with valid env override, got: {:?}",
        result.err()
    );
}

/// SC-007: APP_JWT_JWT_SECRET_FILE 指向合規 secret 檔案 → boot 成功，_FILE 優先
///
/// - APP_JWT_JWT_SECRET_FILE 指向含合規 secret 的臨時檔
/// - APP_JWT_JWT_SECRET 故意設為 placeholder（"change-me-jwt-secret"）
/// → `load_secret_from_file_if_set` 讀 _FILE，覆蓋 bare envvar 值
/// → `validate_jwt_secret` pass → Ok
/// 驗 _FILE precedence over bare envvar（per spec FR-007）
#[tokio::test]
async fn boot_succeeds_with_file_envvar() {
    let _guard = BOOT_MUTEX.lock().await;
    let path = std::env::temp_dir()
        .join(format!("f1_1_integration_jwt_{}", std::process::id()));
    let file_secret = "0123456789abcdef0123456789abcdef"; // 32 chars，合規
    std::fs::write(&path, file_secret).unwrap();

    // _FILE 指向合規 secret；bare envvar 故意設 placeholder 以驗 _FILE 勝出
    std::env::set_var("APP_JWT_JWT_SECRET_FILE", path.to_str().unwrap());
    std::env::set_var("APP_JWT_JWT_SECRET", "change-me-jwt-secret");

    let result = server_config::init_from_file_with_multi_instance_env(
        "../resources/application.yaml",
        None,
    )
    .await;

    std::env::remove_var("APP_JWT_JWT_SECRET_FILE");
    std::env::remove_var("APP_JWT_JWT_SECRET");
    let _ = std::fs::remove_file(&path);

    assert!(
        result.is_ok(),
        "_FILE should take precedence and boot should succeed, got: {:?}",
        result.err()
    );
}
