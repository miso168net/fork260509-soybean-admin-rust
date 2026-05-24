use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Response, StatusCode, Uri},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use once_cell::sync::Lazy;
use server_constant::definition::consts::SystemEvent;
use server_global::global;
use std::{collections::HashSet, sync::RwLock};

use crate::web::{code, res::Res};

use super::{ApiKeyEvent, ComplexApiKeyValidator, SimpleApiKeyValidator};

/// Global set of protected paths.
///
/// This set stores the paths that require API key validation.
static PROTECTED_PATHS: Lazy<RwLock<HashSet<String>>> = Lazy::new(|| RwLock::new(HashSet::new()));

/// Source location for API key.
///
/// This enum defines the possible locations where the API key can be found.
#[derive(Clone, Copy, PartialEq)]
pub enum ApiKeySource {
    /// From request header.
    Header,
    /// From query parameter.
    Query,
}

/// Configuration for simple API key validation.
///
/// This struct holds the configuration for simple API key validation.
#[derive(Clone)]
pub struct SimpleApiKeyConfig {
    /// Source location of API key.
    pub source: ApiKeySource,
    /// Name of API key parameter.
    pub key_name: String,
}

impl Default for SimpleApiKeyConfig {
    fn default() -> Self {
        Self {
            source: ApiKeySource::Header,
            key_name: "x-api-key".to_string(),
        }
    }
}

/// Configuration for complex API key validation with signature.
///
/// This struct holds the configuration for complex API key validation with signature.
#[derive(Clone)]
pub struct ComplexApiKeyConfig {
    /// Access key ID parameter name.
    pub key_name: String,
    /// Timestamp parameter name.
    pub timestamp_name: String,
    /// Nonce parameter name.
    pub nonce_name: String,
    /// Signature parameter name.
    pub signature_name: String,
}

impl Default for ComplexApiKeyConfig {
    fn default() -> Self {
        Self {
            key_name: "AccessKeyId".to_string(),
            timestamp_name: "timestamp".to_string(),
            nonce_name: "nonce".to_string(),
            signature_name: "signature".to_string(),
        }
    }
}

/// API key validation strategy.
///
/// This enum defines the possible API key validation strategies.
#[derive(Clone)]
pub enum ApiKeyValidation {
    /// Simple API key validation.
    Simple(SimpleApiKeyValidator, SimpleApiKeyConfig),
    /// Complex API key validation with signature.
    Complex(ComplexApiKeyValidator, ComplexApiKeyConfig),
}

/// Add a path to protected routes requiring API key validation.
///
/// This function adds a path to the set of protected paths.
#[allow(dead_code)]
pub fn protect_route(path: &str) {
    if let Ok(mut paths) = PROTECTED_PATHS.write() {
        paths.insert(path.to_string());
    }
}

/// Check if URI path requires API key validation.
///
/// This function checks if the given URI path is in the set of protected paths.
#[inline]
fn is_protected_path(uri: &Uri) -> bool {
    if let Ok(paths) = PROTECTED_PATHS.read() {
        let path = uri.path();
        paths.contains(path.strip_suffix('/').unwrap_or(path))
    } else {
        false
    }
}

/// 047 US1 helper：構造 HTTP 401 + WWW-Authenticate: ApiKey + body envelope。
///
/// 用於 api_key_middleware 兩 error 分支（missing 5003 / invalid 5004）。
/// body envelope（{code, data, msg, success}）完全保留、對既有 client 不破壞；
/// 同時設 HTTP 401 + RFC 7235 / 9110 強制的 WWW-Authenticate header 對齊
/// standard REST 語義。
fn unauthorized_response(code: u16, msg: &str) -> Response<Body> {
    let mut resp = (
        StatusCode::UNAUTHORIZED,
        Json(Res::<()>::new_error(code, msg)),
    )
        .into_response();
    resp.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("ApiKey"),
    );
    resp
}

/// API key validation middleware.
///
/// This middleware checks if the API key is valid for the given request.
#[inline]
pub async fn api_key_middleware(
    validator: ApiKeyValidation,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    if !is_protected_path(req.uri()) {
        return next.run(req).await.into_response();
    }

    match validate_request(&validator, &req) {
        Ok(true) => next.run(req).await.into_response(),
        Ok(false) => unauthorized_response(
            code::CODE_PERMISSION_API_KEY_SIGNATURE_INVALID,
            "Invalid API key or signature",
        )
        .into_response(),
        Err(e) => unauthorized_response(code::CODE_PERMISSION_API_KEY_MISSING, e).into_response(),
    }
}

/// Get value from request headers.
///
/// This function retrieves the value of a header from the request headers.
#[inline]
fn get_header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Get value from query parameters.
///
/// This function retrieves the value of a query parameter from the request query.
#[inline]
fn get_query_value<'a>(params: &'a [(String, String)], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// Validate API key in request.
///
/// This function validates the API key in the given request.
#[inline]
fn validate_request(
    validator: &ApiKeyValidation,
    req: &Request<Body>,
) -> Result<bool, &'static str> {
    let headers = req.headers();
    let query = req.uri().query().unwrap_or("");
    let params = if !query.is_empty() {
        parse_query(query)
    } else {
        Vec::new()
    };

    match validator {
        ApiKeyValidation::Simple(validator, config) => {
            let api_key = match config.source {
                ApiKeySource::Header => get_header_value(headers, &config.key_name),
                ApiKeySource::Query => get_query_value(&params, &config.key_name),
            }
            .ok_or("Missing API key")?;

            global::send_dyn_event(
                SystemEvent::AuthApiKeyValidatedEvent.as_ref(),
                Box::new(ApiKeyEvent {
                    api_key: api_key.to_owned(),
                }),
            );
            Ok(validator.validate_key(api_key))
        },
        ApiKeyValidation::Complex(validator, config) => {
            let api_key =
                get_query_value(&params, &config.key_name).ok_or("Missing AccessKeyId")?;

            let timestamp = get_query_value(&params, &config.timestamp_name)
                .ok_or("Missing timestamp")?
                .parse::<i64>()
                .map_err(|_| "Invalid timestamp")?;

            let nonce = get_query_value(&params, &config.nonce_name).ok_or("Missing nonce")?;

            let signature =
                get_query_value(&params, &config.signature_name).ok_or("Missing signature")?;

            let params_for_signing: Vec<(String, String)> = params
                .iter()
                .filter(|(k, _)| k != &config.signature_name)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            global::send_dyn_event(
                SystemEvent::AuthApiKeyValidatedEvent.as_ref(),
                Box::new(ApiKeyEvent {
                    api_key: api_key.to_owned(),
                }),
            );
            Ok(validator.validate_signature(
                api_key,
                &params_for_signing,
                signature,
                timestamp,
                nonce,
            ))
        },
    }
}

/// Parse query string into key-value pairs.
///
/// This function parses a query string into a vector of key-value pairs.
#[inline]
fn parse_query(query: &str) -> Vec<(String, String)> {
    let capacity = query.matches('&').count() + 1;
    let mut params = Vec::with_capacity(capacity);

    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if !k.is_empty() && !v.is_empty() {
                params.push((k.to_string(), v.to_string()));
            }
        }
    }

    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
        middleware::from_fn,
        routing::get,
        Router,
    };
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    async fn dummy_handler() -> &'static str {
        "ok"
    }

    fn build_test_router(path: &str, validation: ApiKeyValidation) -> Router {
        Router::new()
            .route(path, get(dummy_handler))
            .layer(from_fn(move |req, next| {
                api_key_middleware(validation.clone(), req, next)
            }))
    }

    async fn parse_body_envelope(response: axum::response::Response) -> serde_json::Value {
        let body_bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        serde_json::from_slice(&body_bytes).unwrap()
    }

    #[test]
    fn test_api_key_sign() {
        let validator = ComplexApiKeyValidator::new(None);
        validator.add_key_secret("test-access-key".to_string(), "test-secret-key".to_string());

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let nonce = format!("nonce_{}", timestamp);

        let mut params = vec![
            ("AccessKeyId".to_string(), "test-access-key".to_string()),
            ("param1".to_string(), "value1".to_string()),
            ("param2".to_string(), "value2".to_string()),
            ("t".to_string(), timestamp.to_string()),
            ("n".to_string(), nonce.clone()),
        ];

        params.sort_by(|a, b| a.0.cmp(&b.0));

        let signing_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = validator.calculate_signature(&signing_string, "test-secret-key");

        println!(
            "URL with signature: /api/url?{}&sign={}",
            signing_string, signature
        );
    }

    #[tokio::test]
    async fn simple_missing_api_key_returns_401_with_www_authenticate() {
        let path = "/test-simple-missing";
        protect_route(path);

        let validator = SimpleApiKeyValidator::new();
        validator.add_key("valid-key".to_string());
        let validation = ApiKeyValidation::Simple(validator, SimpleApiKeyConfig::default());

        let app = build_test_router(path, validation);
        let req = Request::builder().uri(path).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "ApiKey"
        );

        let body = parse_body_envelope(response).await;
        assert_eq!(body["code"], 5003);
        assert_eq!(body["success"], false);
    }

    #[tokio::test]
    async fn simple_invalid_api_key_returns_401_with_www_authenticate() {
        let path = "/test-simple-invalid";
        protect_route(path);

        let validator = SimpleApiKeyValidator::new();
        validator.add_key("valid-key".to_string());
        let validation = ApiKeyValidation::Simple(validator, SimpleApiKeyConfig::default());

        let app = build_test_router(path, validation);
        let req = Request::builder()
            .uri(path)
            .header("x-api-key", "bogus-XYZ")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "ApiKey"
        );

        let body = parse_body_envelope(response).await;
        assert_eq!(body["code"], 5004);
        assert_eq!(body["success"], false);
    }

    #[tokio::test]
    async fn simple_valid_api_key_passes_through() {
        let path = "/test-simple-valid";
        protect_route(path);

        let validator = SimpleApiKeyValidator::new();
        validator.add_key("valid-key".to_string());
        let validation = ApiKeyValidation::Simple(validator, SimpleApiKeyConfig::default());

        let app = build_test_router(path, validation);
        let req = Request::builder()
            .uri(path)
            .header("x-api-key", "valid-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body_bytes[..], b"ok");
    }

    #[tokio::test]
    async fn simple_non_protected_path_passes_through() {
        let path = "/test-simple-non-protected";
        // intentionally do NOT call protect_route(path) — middleware should short-circuit

        let validator = SimpleApiKeyValidator::new();
        validator.add_key("any-key".to_string());
        let validation = ApiKeyValidation::Simple(validator, SimpleApiKeyConfig::default());

        let app = build_test_router(path, validation);
        let req = Request::builder().uri(path).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        let body_bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body_bytes[..], b"ok");
    }

    #[tokio::test]
    async fn complex_missing_field_returns_401_with_www_authenticate() {
        let path = "/test-complex-missing-field";
        protect_route(path);

        let validator = ComplexApiKeyValidator::new(None);
        validator.add_key_secret(
            "test-access-key".to_string(),
            "test-secret-key".to_string(),
        );
        let validation = ApiKeyValidation::Complex(
            validator,
            ComplexApiKeyConfig {
                key_name: "AccessKeyId".to_string(),
                timestamp_name: "t".to_string(),
                nonce_name: "n".to_string(),
                signature_name: "sign".to_string(),
            },
        );

        let app = build_test_router(path, validation);
        // query missing AccessKeyId
        let uri = format!("{}?t=1234567890&n=nonce_x&sign=anything", path);
        let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "ApiKey"
        );

        let body = parse_body_envelope(response).await;
        assert_eq!(body["code"], 5003);
        assert_eq!(body["success"], false);
        let msg = body["msg"].as_str().unwrap_or("");
        assert!(
            msg.contains("Missing AccessKeyId"),
            "expected msg to contain 'Missing AccessKeyId', got: {}",
            msg
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn complex_invalid_signature_returns_401_with_www_authenticate() {
        let path = "/test-complex-invalid-sig";
        protect_route(path);

        let validator = ComplexApiKeyValidator::new(None);
        validator.add_key_secret(
            "test-access-key".to_string(),
            "test-secret-key".to_string(),
        );
        let validation = ApiKeyValidation::Complex(
            validator,
            ComplexApiKeyConfig {
                key_name: "AccessKeyId".to_string(),
                timestamp_name: "t".to_string(),
                nonce_name: "n".to_string(),
                signature_name: "sign".to_string(),
            },
        );

        let app = build_test_router(path, validation);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let uri = format!(
            "{}?AccessKeyId=test-access-key&t={}&n=nonce_x&sign=wronghex",
            path, timestamp
        );
        let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "ApiKey"
        );

        let body = parse_body_envelope(response).await;
        assert_eq!(body["code"], 5004);
        assert_eq!(body["success"], false);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn complex_valid_signed_request_passes_through() {
        let path = "/test-complex-valid";
        protect_route(path);

        let validator = ComplexApiKeyValidator::new(None);
        validator.add_key_secret(
            "test-access-key".to_string(),
            "test-secret-key".to_string(),
        );
        let validation = ApiKeyValidation::Complex(
            validator.clone(),
            ComplexApiKeyConfig {
                key_name: "AccessKeyId".to_string(),
                timestamp_name: "t".to_string(),
                nonce_name: "n".to_string(),
                signature_name: "sign".to_string(),
            },
        );

        let app = build_test_router(path, validation);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let nonce = format!("nonce_{}", timestamp);

        let mut params = vec![
            ("AccessKeyId".to_string(), "test-access-key".to_string()),
            ("t".to_string(), timestamp.to_string()),
            ("n".to_string(), nonce.clone()),
        ];
        params.sort_by(|a, b| a.0.cmp(&b.0));
        let signing_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let signature = validator.calculate_signature(&signing_string, "test-secret-key");

        let uri = format!("{}?{}&sign={}", path, signing_string, signature);
        let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(&body_bytes[..], b"ok");
    }

    #[tokio::test]
    async fn complex_invalid_timestamp_returns_401_with_www_authenticate() {
        let path = "/test-complex-invalid-ts";
        protect_route(path);

        let validator = ComplexApiKeyValidator::new(None);
        validator.add_key_secret(
            "test-access-key".to_string(),
            "test-secret-key".to_string(),
        );
        let validation = ApiKeyValidation::Complex(
            validator,
            ComplexApiKeyConfig {
                key_name: "AccessKeyId".to_string(),
                timestamp_name: "t".to_string(),
                nonce_name: "n".to_string(),
                signature_name: "sign".to_string(),
            },
        );

        let app = build_test_router(path, validation);
        let uri = format!(
            "{}?AccessKeyId=test-access-key&t=not-a-number&n=nonce_x&sign=anything",
            path
        );
        let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers().get(header::WWW_AUTHENTICATE).unwrap(),
            "ApiKey"
        );

        let body = parse_body_envelope(response).await;
        assert_eq!(body["code"], 5003);
        assert_eq!(body["success"], false);
        assert_eq!(body["msg"], "Invalid timestamp");
    }
}
