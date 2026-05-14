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
}
