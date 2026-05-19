//! F1.1 jwt-secrets — strict secret validation + _FILE pattern loader。
//!
//! 集中放：
//! - `PLACEHOLDER_SECRETS` 黑名單
//! - `load_secret_from_file_if_set(base_envvar)` 讀 `<envvar>_FILE` content
//! - `validate_jwt_secret(&secret)` 對 3 條 rule 跑 panic-on-fail validation
//! - `apply_jwt_secret_hardening(&mut JwtConfig)` — config_init 兩個 callsite 共用 helper
//!
//! per F1.1 spec FR-001~009 + data-model.md §E1。

use std::env;
use std::fs;

/// 已知 placeholder secret values — 任一命中就 panic
pub const PLACEHOLDER_SECRETS: &[&str] = &[
    "soybean-admin-rust",   // 既有 yaml hardcoded fake、F1.1 改後仍可能在舊 deploy
    "change-me",
    "change-me-jwt-secret", // F1.1 後 application.yaml default
    "your-secret-here",
    "PLACEHOLDER",
    "TODO",
];

/// 從 `<base_envvar>_FILE` 環境變數指向的 file 讀 secret content。
///
/// - `_FILE` env var 未 set → 返 None（caller 走 bare envvar / yaml path）
/// - `_FILE` set + file 讀成功 → trim 後返 Some(content)
/// - `_FILE` set 但 read 失敗（不存在 / permission denied / dir） → panic
///
/// per spec FR-005 + FR-008。
pub fn load_secret_from_file_if_set(base_envvar: &str) -> Option<String> {
    let file_envvar = format!("{}_FILE", base_envvar);
    match env::var(&file_envvar) {
        Ok(path) => match fs::read_to_string(&path) {
            Ok(content) => Some(content.trim().to_string()),
            Err(e) => panic!(
                "F1.1: {} = '{}' read failed: {}\n\
                 Fix: check Docker secrets mount (mode 0444, owner uid=<appuser uid>) or file path correctness",
                file_envvar, path, e
            ),
        },
        Err(_) => None,
    }
}

/// 對 jwt_secret 跑 strict validation：empty / placeholder / length < 32。
///
/// 失敗時 panic with friendly error message（含 source label + 修正建議）。
///
/// per spec FR-001 ~ FR-004。
pub fn validate_jwt_secret(secret: &str) {
    if secret.is_empty() {
        panic!(
            "F1.1: jwt_secret resolved to empty value.\n\
             Sources checked (in precedence order):\n\
               1. APP_JWT_JWT_SECRET_FILE (file path)\n\
               2. APP_JWT_JWT_SECRET (bare envvar)\n\
               3. application.yaml jwt.jwt_secret (yaml default)\n\
             Fix: generate via `openssl rand -hex 32` + set APP_JWT_JWT_SECRET or APP_JWT_JWT_SECRET_FILE"
        );
    }
    if PLACEHOLDER_SECRETS.contains(&secret) {
        panic!(
            "F1.1: jwt_secret = '{}' is a known placeholder (not a real secret).\n\
             Placeholders blacklist: {:?}\n\
             Fix: generate via `openssl rand -hex 32` + set APP_JWT_JWT_SECRET or APP_JWT_JWT_SECRET_FILE",
            secret, PLACEHOLDER_SECRETS
        );
    }
    if secret.len() < 32 {
        panic!(
            "F1.1: jwt_secret length = {} bytes < 32 (HS256 minimum security baseline).\n\
             Fix: generate via `openssl rand -hex 32` (produces 64-char hex = 32 bytes) + set APP_JWT_JWT_SECRET or APP_JWT_JWT_SECRET_FILE",
            secret.len()
        );
    }
}

/// 應用 F1.1 jwt secret hardening：_FILE precedence override + strict validation。
///
/// Precedence（per spec FR-007）：
///   1. `APP_JWT_JWT_SECRET_FILE` (highest) — reads file content
///   2. `APP_JWT_JWT_SECRET` (bare envvar) — direct env var value
///   3. `application.yaml` jwt.jwt_secret (lowest) — yaml default
///
/// Note: bare `APP_JWT_JWT_SECRET` cannot be relied on via the `config` crate's
/// Environment source due to `_`-separator ambiguity with the `jwt_secret` field name
/// (env key `APP_JWT_JWT_SECRET` splits as `jwt.jwt.secret` not `jwt.jwt_secret`).
/// We therefore read it explicitly here to guarantee correct precedence.
///
/// per data-model.md §E2，用於 config_init 內所有 JwtConfig init callsite
/// （`init_from_file` + `init_global_config`）。
pub fn apply_jwt_secret_hardening(jwt: &mut crate::JwtConfig) {
    // Step 1: _FILE takes highest precedence
    if let Some(secret_from_file) = load_secret_from_file_if_set("APP_JWT_JWT_SECRET") {
        jwt.jwt_secret = secret_from_file;
    } else if let Ok(bare) = env::var("APP_JWT_JWT_SECRET") {
        // Step 2: bare envvar (explicit read; config crate env-override is unreliable
        // for field names containing underscores due to separator ambiguity)
        jwt.jwt_secret = bare;
    }
    // Step 3: yaml default — already in jwt.jwt_secret, nothing to do
    validate_jwt_secret(&jwt.jwt_secret);

    // F10.1: refresh_secret hardening
    // Precedence: APP_JWT_REFRESH_SECRET_FILE > APP_JWT_REFRESH_SECRET > yaml default
    // Empty-file → fallback to jwt_secret (mirrors nestjs entrypoint `${RTS:-$JWT_SECRET}`)
    if let Some(content_from_file) = load_secret_from_file_if_set("APP_JWT_REFRESH_SECRET") {
        if content_from_file.is_empty() {
            // Empty file → fallback to jwt_secret
            jwt.refresh_secret = jwt.jwt_secret.clone();
        } else {
            jwt.refresh_secret = content_from_file;
        }
    } else if let Ok(bare) = env::var("APP_JWT_REFRESH_SECRET") {
        jwt.refresh_secret = bare;
    }
    // else: yaml default already in jwt.refresh_secret
    validate_jwt_secret(&jwt.refresh_secret);
}

/// W-F4: apply DATABASE_URL hardening — `_FILE` precedence override.
///
/// Precedence:
///   1. `APP_DATABASE_URL_FILE` (highest) — reads file via `load_secret_from_file_if_set`
///   2. `APP_DATABASE_URL` (bare envvar) — populated by config-rs Environment source
///      (no `_` separator ambiguity, yaml field 是 `database.url`,單 underscore)
///   3. `application.yaml` database.url (lowest)
///
/// 不做 URL format validation;由 sea-orm connection pool init 階段 fail-fast (per R-011)。
///
/// per W-F4 spec FR-009, contracts C-S8, research R-001 + R-003。
pub fn apply_database_url_hardening(database: &mut crate::DatabaseConfig) {
    if let Some(url_from_file) = load_secret_from_file_if_set("APP_DATABASE_URL") {
        database.url = url_from_file;
    }
    // bare APP_DATABASE_URL envvar 不需要顯式讀;config-rs Environment source 已處理
    // (yaml `database.url` 與 env `APP_DATABASE_URL` 無 `_`-separator ambiguity)。
    // yaml default 由原 config 物件 fallback。
}

/// W-F4: apply REDIS_URL hardening — `_FILE` precedence override (Option<String> 版)。
///
/// Precedence 同 `apply_database_url_hardening`,差別在 `RedisConfig.url` 是 `Option<String>`:
///   - `_FILE` 命中 → `redis.url = Some(content)`
///   - 否則保留原值 (config-rs envvar 或 yaml fallback)
///
/// 不做 URL format validation;由 redis client connect 階段 fail-fast (per R-011)。
///
/// per W-F4 spec FR-009, contracts C-S9, research R-001 + R-003。
pub fn apply_redis_url_hardening(redis: &mut crate::RedisConfig) {
    if let Some(url_from_file) = load_secret_from_file_if_set("APP_REDIS_URL") {
        redis.url = Some(url_from_file);
    }
}

// ============================================================
// W-F4 unit tests (TDD)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseConfig, JwtConfig, RedisConfig, RedisMode};
    use std::fs;
    use std::path::PathBuf;

    // env::set_var/remove_var 是 process-global mutable state;
    // 在 cargo test 同 binary 多 thread 平行下不安全,用 Mutex 序列化所有觸碰 env 的測試。
    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn make_database_config(url: &str) -> DatabaseConfig {
        DatabaseConfig {
            url: url.to_string(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: 30,
            idle_timeout: 600,
        }
    }

    fn make_redis_config(url: Option<&str>) -> RedisConfig {
        RedisConfig {
            mode: RedisMode::Single,
            url: url.map(|s| s.to_string()),
            urls: None,
        }
    }

    /// 在 std::env::temp_dir() 下建立唯一檔名的 tempfile 並寫入 content,返 path。
    /// caller 須 cleanup (fs::remove_file)。
    fn write_tempfile(unique_suffix: &str, content: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "wf4_secret_loader_test_{}_{}.txt",
            unique_suffix,
            std::process::id()
        ));
        fs::write(&path, content).expect("write tempfile");
        path
    }

    #[test]
    fn test_apply_database_url_hardening_from_file() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Arrange: tempfile 含 "postgres://from-file"; APP_DATABASE_URL_FILE 指向它
        let tmp = write_tempfile("db_from_file", "postgres://from-file");
        env::set_var("APP_DATABASE_URL_FILE", &tmp);
        env::remove_var("APP_DATABASE_URL");

        let mut database = make_database_config("yaml-default");

        // Act
        apply_database_url_hardening(&mut database);

        // Assert: file value 蓋過 yaml default
        assert_eq!(database.url, "postgres://from-file");

        // Cleanup
        env::remove_var("APP_DATABASE_URL_FILE");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_apply_database_url_hardening_no_file_keeps_url() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Arrange: 不設 APP_DATABASE_URL_FILE,helper 應該維持原 yaml default
        env::remove_var("APP_DATABASE_URL_FILE");
        env::remove_var("APP_DATABASE_URL");

        let mut database = make_database_config("yaml-default");

        // Act
        apply_database_url_hardening(&mut database);

        // Assert: 原值不動 (bare envvar fallback 是 config-rs 的責任,不是 helper 的)
        assert_eq!(database.url, "yaml-default");
    }

    fn make_jwt_config(jwt_secret: &str, refresh_secret: &str) -> JwtConfig {
        JwtConfig {
            jwt_secret: jwt_secret.to_string(),
            issuer: "https://test.example.com".to_string(),
            expire: 7200,
            refresh_secret: refresh_secret.to_string(),
            refresh_expire: 7200,
        }
    }

    // ----------------------------------------------------------------
    // F10.1 tests (T023)
    // ----------------------------------------------------------------

    #[test]
    fn test_apply_jwt_refresh_secret_empty_file_fallback_to_jwt_secret() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Arrange: empty tempfile (simulates dev default empty Docker secret)
        let tmp = write_tempfile("refresh_empty_fallback", "");
        env::set_var("APP_JWT_REFRESH_SECRET_FILE", &tmp);
        env::remove_var("APP_JWT_REFRESH_SECRET");
        // Unset the jwt_secret _FILE so it does not interfere
        env::remove_var("APP_JWT_JWT_SECRET_FILE");
        env::remove_var("APP_JWT_JWT_SECRET");

        let jwt_secret = "valid-jwt-secret-padded-to-32chars!";
        let mut jwt = make_jwt_config(jwt_secret, "placeholder-refresh-secret-xyz!");

        // Act
        apply_jwt_secret_hardening(&mut jwt);

        // Assert: empty file → fallback to jwt_secret
        assert_eq!(
            jwt.refresh_secret, jwt_secret,
            "empty refresh secret file must fallback to jwt_secret"
        );

        // Cleanup
        env::remove_var("APP_JWT_REFRESH_SECRET_FILE");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_apply_jwt_refresh_secret_non_empty_file_uses_content() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Arrange: non-empty tempfile with a valid 32+ char secret
        let refresh_content = "real-refresh-secret-padded-32chr!";
        let tmp = write_tempfile("refresh_non_empty", refresh_content);
        env::set_var("APP_JWT_REFRESH_SECRET_FILE", &tmp);
        env::remove_var("APP_JWT_REFRESH_SECRET");
        env::remove_var("APP_JWT_JWT_SECRET_FILE");
        env::remove_var("APP_JWT_JWT_SECRET");

        let jwt_secret = "valid-jwt-secret-padded-to-32chars!";
        let mut jwt = make_jwt_config(jwt_secret, "placeholder-refresh-secret-xyz!");

        // Act
        apply_jwt_secret_hardening(&mut jwt);

        // Assert: non-empty file content is used directly
        assert_eq!(
            jwt.refresh_secret, refresh_content,
            "non-empty refresh secret file must be used as refresh_secret"
        );

        // Cleanup
        env::remove_var("APP_JWT_REFRESH_SECRET_FILE");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_apply_redis_url_hardening_from_file() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Arrange: tempfile 含 "redis://from-file:6379/0"; APP_REDIS_URL_FILE 指向它
        let tmp = write_tempfile("redis_from_file", "redis://from-file:6379/0");
        env::set_var("APP_REDIS_URL_FILE", &tmp);
        env::remove_var("APP_REDIS_URL");

        let mut redis = make_redis_config(Some("redis://yaml-default:6379/0"));

        // Act
        apply_redis_url_hardening(&mut redis);

        // Assert: file value 蓋過 yaml default,且包成 Some()
        assert_eq!(redis.url, Some("redis://from-file:6379/0".to_string()));

        // Cleanup
        env::remove_var("APP_REDIS_URL_FILE");
        let _ = fs::remove_file(&tmp);
    }
}
