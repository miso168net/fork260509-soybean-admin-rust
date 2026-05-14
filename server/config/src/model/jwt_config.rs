use serde::Deserialize;

/// JWT 配置
///
/// 環境變數（rev1 prod 推薦 _FILE pattern、dev 可用 bare envvar）：
/// - `APP_JWT_JWT_SECRET` — 直接設 secret 值（dev 用、prod 避免）
/// - `APP_JWT_JWT_SECRET_FILE` — 指 file path、讀取檔案內容為 secret 值（prod 用、per Constitution §架構約束）
///   precedence: `_FILE` > bare envvar > yaml default（per F1.1 spec FR-007）
/// - `APP_JWT_ISSUER` — issuer 字串（非 secret、bare envvar OK）
/// - `APP_JWT_EXPIRE` — 整數秒（非 secret、bare envvar OK）
///
/// Strict validation 規則（per F1.1 spec FR-001~004、config_init 載入點 fail-fast）：
/// - `jwt_secret` MUST NOT 為空
/// - `jwt_secret` MUST NOT in `PLACEHOLDER_SECRETS` 黑名單
///   （`"soybean-admin-rust"` / `"change-me"` / `"change-me-jwt-secret"` /
///    `"your-secret-here"` / `"PLACEHOLDER"` / `"TODO"`）
/// - `jwt_secret.len()` MUST >= 32 bytes（HS256 minimum security baseline）
///
/// per spec FR-001~009 + data-model.md §E3。
#[derive(Deserialize, Debug, Clone)]
pub struct JwtConfig {
    /// JWT 密鑰（envvar `APP_JWT_JWT_SECRET` / `APP_JWT_JWT_SECRET_FILE`）
    pub jwt_secret: String,

    /// JWT 簽發者（envvar `APP_JWT_ISSUER`）
    pub issuer: String,

    /// JWT 過期時間（秒；envvar `APP_JWT_EXPIRE`）
    pub expire: i64,
}
