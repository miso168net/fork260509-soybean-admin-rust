use serde::Deserialize;

/// JWT 配置
///
/// 支持的环境变量：
/// - APP_JWT_JWT_SECRET: JWT 密钥
/// - APP_JWT_ISSUER: JWT 签发者
/// - APP_JWT_EXPIRE: JWT 过期时间（秒）
/// - APP_JWT_REFRESH_TOKEN_EXPIRE: refresh token 过期时间（秒，默认 14 天）
#[derive(Deserialize, Debug, Clone)]
pub struct JwtConfig {
    /// JWT 密钥
    /// 环境变量: APP_JWT_JWT_SECRET
    pub jwt_secret: String,

    /// JWT 签发者
    /// 环境变量: APP_JWT_ISSUER
    pub issuer: String,

    /// JWT 过期时间（秒）
    /// 环境变量: APP_JWT_EXPIRE
    pub expire: i64,

    /// refresh token 过期时间（秒）
    /// 环境变量: APP_JWT_REFRESH_TOKEN_EXPIRE
    /// 默认: 1209600（14 天）
    #[serde(default = "default_refresh_token_expire")]
    pub refresh_token_expire: i64,
}

fn default_refresh_token_expire() -> i64 {
    1209600
}
