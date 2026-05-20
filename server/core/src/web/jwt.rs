use std::{error::Error, fmt};

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use server_config::JwtConfig;
use server_global::global;
use ulid::Ulid;

use crate::web::auth::Claims;

/// F10.1: Refresh token claims struct (極簡 HS256 JWT、per data-model §E1)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshClaims {
    sub: String,
    exp: Option<usize>,
    iat: Option<usize>,
    nbf: Option<usize>,
    jti: Option<String>,
    iss: Option<String>,
}

impl RefreshClaims {
    pub fn new(sub: String) -> Self {
        Self {
            sub,
            exp: None,
            iat: None,
            nbf: None,
            jti: None,
            iss: None,
        }
    }

    pub fn set_exp(&mut self, exp: usize) {
        self.exp = Some(exp);
    }

    pub fn set_iss(&mut self, iss: String) {
        self.iss = Some(iss);
    }

    pub fn set_iat(&mut self, iat: usize) {
        self.iat = Some(iat);
    }

    pub fn set_nbf(&mut self, nbf: usize) {
        self.nbf = Some(nbf);
    }

    pub fn set_jti(&mut self, jti: String) {
        self.jti = Some(jti);
    }

    /// Accessor for the subject (user_id) claim.
    pub fn sub(&self) -> &str {
        &self.sub
    }
}

// pub static KEYS: Lazy<Arc<Mutex<Keys>>> = Lazy::new(|| {
//     let config = global::get_config::<JwtConfig>()
//         .expect("[soybean-admin-rust] >>>>>> [server-core] Failed to load JWT
// config");     Arc::new(Mutex::new(Keys::new(config.jwt_secret.as_bytes())))
// });
//
// pub static VALIDATION: Lazy<Arc<Mutex<Validation>>> = Lazy::new(|| {
//     let config = global::get_config::<JwtConfig>()
//         .expect("[soybean-admin-rust] >>>>>> [server-core] Failed to load JWT
// config");     let mut validation = Validation::default();
//     validation.leeway = 60;
//     validation.set_issuer(&[config.issuer.clone()]);
//     Arc::new(Mutex::new(validation))
// });

#[derive(Debug)]
pub enum JwtError {
    KeysNotInitialized,
    ValidationNotInitialized,
    TokenCreationError(String),
    TokenValidationError(String),
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwtError::KeysNotInitialized => write!(f, "Keys not initialized"),
            JwtError::ValidationNotInitialized => write!(f, "Validation not initialized"),
            JwtError::TokenCreationError(err) => write!(f, "Token creation error: {}", err),
            JwtError::TokenValidationError(err) => write!(f, "Token validation error: {}", err),
        }
    }
}

impl Error for JwtError {}

pub struct JwtUtils;

impl JwtUtils {
    pub async fn generate_token(claims: &Claims) -> Result<String, JwtError> {
        let keys_arc = global::KEYS.get().ok_or(JwtError::KeysNotInitialized)?;

        let keys = keys_arc.lock().await;

        let mut claims_clone = claims.clone();

        let now = Utc::now();
        let timestamp = now.timestamp() as usize;
        let jwt_config = global::get_config::<JwtConfig>().await.unwrap();
        claims_clone.set_exp((now + Duration::seconds(jwt_config.expire)).timestamp() as usize);
        claims_clone.set_iss(jwt_config.issuer.to_string());
        claims_clone.set_iat(timestamp);
        claims_clone.set_nbf(timestamp);
        claims_clone.set_jti(Ulid::new().to_string());

        let token = encode(&Header::default(), &claims_clone, &keys.encoding)
            .map_err(|e| JwtError::TokenCreationError(e.to_string()));

        if let Ok(ref tok) = token {
            global::send_string_event(tok.clone());
        }

        token
    }

    /// F10.1: 生成 HS256 refresh token（signed JWT、per data-model §E1）
    pub async fn generate_refresh_token(user_id: String) -> Result<String, JwtError> {
        let keys_arc = global::REFRESH_KEYS.get().ok_or(JwtError::KeysNotInitialized)?;
        let keys = keys_arc.lock().await;

        let jwt_config = global::get_config::<JwtConfig>().await.unwrap();

        Self::sign_refresh_claims(user_id, jwt_config.refresh_expire, &jwt_config.issuer, &keys.encoding)
    }

    /// Pure inner helper: 接受 encoding key + expire + issuer，不依賴 global state。
    /// 讓單元測試可直接注入 key、避免 OnceCell 初始化競爭。
    fn sign_refresh_claims(
        user_id: String,
        refresh_expire: i64,
        issuer: &str,
        encoding_key: &jsonwebtoken::EncodingKey,
    ) -> Result<String, JwtError> {
        let now = Utc::now();
        let timestamp = now.timestamp() as usize;

        let mut claims = RefreshClaims::new(user_id);
        claims.set_exp((now + Duration::seconds(refresh_expire)).timestamp() as usize);
        claims.set_iss(issuer.to_string());
        claims.set_iat(timestamp);
        claims.set_nbf(timestamp);
        claims.set_jti(Ulid::new().to_string());

        encode(&Header::default(), &claims, encoding_key)
            .map_err(|e| JwtError::TokenCreationError(e.to_string()))
    }

    /// Validate a refresh token using the refresh-specific key (REFRESH_KEYS).
    /// RefreshClaims has no `aud` field, so validate_aud is disabled.
    pub async fn validate_refresh_token(token: &str) -> Result<TokenData<RefreshClaims>, JwtError> {
        let keys_arc = global::REFRESH_KEYS.get().ok_or(JwtError::KeysNotInitialized)?;
        let keys = keys_arc.lock().await;

        let jwt_config = global::get_config::<JwtConfig>().await.ok_or(JwtError::KeysNotInitialized)?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 60;
        validation.set_issuer(&[jwt_config.issuer.as_str()]);
        validation.validate_nbf = true;
        // RefreshClaims has no `aud` field — disable audience validation
        validation.validate_aud = false;

        decode::<RefreshClaims>(token, &keys.decoding, &validation)
            .map_err(|e| JwtError::TokenValidationError(e.to_string()))
    }

    pub async fn validate_token(
        token: &str,
        audience: &str,
    ) -> Result<TokenData<Claims>, JwtError> {
        let keys_arc = global::KEYS.get().ok_or(JwtError::KeysNotInitialized)?;

        let keys = keys_arc.lock().await;
        let validation_arc = global::VALIDATION
            .get()
            .ok_or(JwtError::ValidationNotInitialized)?;
        let validation = validation_arc.lock().await;

        let mut validation_clone = validation.clone();
        validation_clone.set_audience(&[audience.to_string()]);
        decode::<Claims>(token, &keys.decoding, &validation_clone)
            .map_err(|e| JwtError::TokenValidationError(e.to_string()))
    }
}

// ============================================================
// F10.1 unit tests (TDD — T029)
// ============================================================

#[cfg(test)]
mod tests {
    use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Validation};

    use super::*;

    const TEST_SECRET: &str = "test-refresh-secret-padded-32char!";
    const TEST_ISSUER: &str = "https://test.example.com";
    const TEST_EXPIRE: i64 = 7200;

    #[test]
    fn test_generate_refresh_token_signs_valid_hs256_jwt_with_refresh_claims() {
        // Arrange: pure encoding key — no global state needed
        let encoding_key = EncodingKey::from_secret(TEST_SECRET.as_bytes());

        // Act: call pure helper directly
        let token =
            JwtUtils::sign_refresh_claims("test-user-id".to_string(), TEST_EXPIRE, TEST_ISSUER, &encoding_key)
                .expect("sign_refresh_claims should succeed");

        // Assert shape: 2 dots (header.payload.signature)
        let dot_count = token.chars().filter(|&c| c == '.').count();
        assert_eq!(dot_count, 2, "JWT must have exactly 2 dots");
        assert!(token.len() > 100, "JWT length should be > 100 chars");

        // Assert decoded claims
        let decoding_key = DecodingKey::from_secret(TEST_SECRET.as_bytes());
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[TEST_ISSUER]);
        validation.validate_nbf = true;
        // RefreshClaims has no aud field — disable audience validation
        validation.validate_aud = false;

        let token_data = jsonwebtoken::decode::<RefreshClaims>(&token, &decoding_key, &validation)
            .expect("JWT must decode successfully with the same secret");

        let claims = token_data.claims;
        assert_eq!(claims.sub, "test-user-id", "sub must match user_id");
        assert!(claims.exp.is_some(), "exp must be set");
        assert!(claims.iat.is_some(), "iat must be set");
        assert!(claims.nbf.is_some(), "nbf must be set");
        assert!(claims.jti.is_some(), "jti must be set");
    }
}
