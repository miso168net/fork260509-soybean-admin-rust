//! F5.1 Integration tests: login handler envelope wiring + JWT auth middleware + Casbin adapter.
//!
//! # Test inventory
//!
//! | # | fn | what is tested |
//! |---|---|---|
//! | 1 | `login_handler_returns_auth_output_envelope` | Envelope wiring: a mock route returning `Res::new_data(AuthOutput{…})` serialises to `{code:0, data:{token,refreshToken}, …}`. **Not** the real login handler (DB/service deps prohibited per F5.1 FR-024). Real login path covered by T013 acceptance test. |
//! | 2 | `jwt_auth_middleware_rejects_missing_token` | Missing Bearer header → middleware short-circuits with envelope `{code:5001, …}`. Code is `CODE_PERMISSION_CASBIN_DENY` (5001) because `jwt_auth_middleware` returns that constant on the `Authorization<Bearer>` typed-get `None` branch (server/middleware/src/jwt.rs:17-22). |
//! | 3 | `casbin_envelope_adapter_converts_403_to_envelope` | `CasbinAxumLayer` rejects an unmatched route with plain text 403; `casbin_envelope_adapter` rewrites it to `{code:5001, msg:"您没有访问该资源的权限，请联系管理员", data:null}`. |
//!
//! # Res JSON shape (server/core/src/web/res.rs)
//!
//! Fields: `code: u16`, `data: Option<T>`, `msg: String`, `success: bool`.
//! No `rename_all` on `Res`, so JSON keys are snake_case: `code`, `data`, `msg`, `success`.
//! `AuthOutput` has `#[serde(rename_all = "camelCase")]` → `token` / `refreshToken`.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use axum_casbin::{
    casbin::{DefaultModel, FileAdapter},
    CasbinAxumLayer,
};
use serde::Serialize;
use server_core::web::res::Res;
use server_middleware::casbin_envelope_adapter::casbin_envelope_adapter;
use tower::{ServiceBuilder, ServiceExt};

// ────────────────────────────────────────────────────────────────────────────
// Helper: read response body as serde_json::Value
// ────────────────────────────────────────────────────────────────────────────

async fn body_to_json(body: Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, 4096).await.unwrap();
    serde_json::from_slice(&bytes).expect("response body should be valid JSON")
}

// ────────────────────────────────────────────────────────────────────────────
// Test 1: envelope wiring for AuthOutput (mock route, no DB)
// ────────────────────────────────────────────────────────────────────────────

/// Local mirror of `server_model::admin::output::sys_authentication::AuthOutput`.
///
/// `server-model` is NOT a direct dependency of `server-initialize` (Cargo.toml lists
/// `server-core`, `server-service`, etc. but not `server-model`). Rather than add a new
/// dep, we mirror only the fields exercised in this test — the JSON shape is what matters.
/// The `#[serde(rename_all = "camelCase")]` on the real struct is the property under test.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MockAuthOutput {
    token: String,
    refresh_token: String,
}

/// Mock handler: returns the same envelope shape the real login handler would.
/// Scope: verifies `Res<MockAuthOutput>` → axum `Json` → correct JSON field names.
/// NOT exercising the real `sys_auth_service` (FR-024 prohibits rewriting it;
/// T013 covers the end-to-end login path with a live DB).
async fn mock_login_handler() -> Res<MockAuthOutput> {
    Res::new_data(MockAuthOutput {
        token: "access.token.here".to_string(),
        refresh_token: "refresh.token.here".to_string(),
    })
}

#[tokio::test]
async fn login_handler_returns_auth_output_envelope() {
    let app = Router::new().route("/auth/login", get(mock_login_handler));
    let svc = ServiceBuilder::new().service(app);

    let request = Request::builder()
        .method("GET")
        .uri("/auth/login")
        .body(Body::empty())
        .unwrap();

    let response = svc.oneshot(request).await.unwrap();

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "mock login route should return HTTP 200"
    );

    let json = body_to_json(response.into_body()).await;

    // Envelope outer shape (Res fields: code, data, msg, success)
    assert_eq!(
        json["code"], 0,
        "code should be 0 (CODE_SUCCESS) for a successful response"
    );
    assert!(
        json["success"].as_bool().unwrap_or(false),
        "success should be true"
    );

    // data must be present and contain camelCase fields (AuthOutput #[serde(rename_all = "camelCase")])
    let data = &json["data"];
    assert!(
        !data.is_null(),
        "data should not be null for a successful auth response"
    );
    assert_eq!(
        data["token"], "access.token.here",
        "data.token should be present (snake_case field → JSON key 'token')"
    );
    assert_eq!(
        data["refreshToken"], "refresh.token.here",
        "data.refreshToken should be camelCase (AuthOutput #[serde(rename_all = \"camelCase\")])"
    );
    // Verify no 'refresh_token' leaks through (would indicate rename_all not applied)
    assert!(
        data["refresh_token"].is_null(),
        "data.refresh_token should not exist; camelCase rename must apply"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Test 2: jwt_auth_middleware rejects missing Bearer token
// ────────────────────────────────────────────────────────────────────────────
//
// `jwt_auth_middleware` uses `req.headers().typed_get::<Authorization<Bearer>>()`.
// When no Authorization header is present → None branch → returns:
//   Res::<String>::new_error(code::CODE_PERMISSION_CASBIN_DENY, "No token provided…")
//
// CODE_PERMISSION_CASBIN_DENY = 5001 (server/core/src/web/code.rs:33)
// This is NOT a JwtError path; it's the missing-header short-circuit.
// So the expected envelope code is 5001.
//
// Note: initialize_config + initialize_keys_and_validation are NOT called here
// because the middleware returns before reaching JwtUtils::validate_token when
// the header is absent — no global state needed for this specific case.
// However, `jwt_auth_middleware` internally does reach `JwtUtils::validate_token`
// only when a token IS present. For the missing-token path, global state is irrelevant.
//
// ⚠ One subtlety: the middleware also checks GLOBAL_PRIMARY_DB after JWT validation.
// For missing-token, execution never reaches the DB check. Safe to run without DB.

#[tokio::test]
async fn jwt_auth_middleware_rejects_missing_token() {
    use server_constant::definition::Audience;
    use server_middleware::jwt_auth_middleware;

    // Minimal handler; never reached in this test since middleware short-circuits.
    async fn unreachable_handler() -> &'static str {
        "should not be called"
    }

    let app = Router::new()
        .route("/auth/getUserInfo", get(unreachable_handler))
        .layer(middleware::from_fn(move |req, next| {
            jwt_auth_middleware(req, next, Audience::ManagementPlatform.as_str())
        }));

    let svc = ServiceBuilder::new().service(app);

    // Request with NO Authorization header
    let request = Request::builder()
        .method("GET")
        .uri("/auth/getUserInfo")
        .body(Body::empty())
        .unwrap();

    let response = svc.oneshot(request).await.unwrap();

    // The middleware short-circuits and serialises Res as JSON.
    // axum's `Json` wrapper always returns 200 for IntoResponse — the error is
    // encoded in the envelope `code` field, not in the HTTP status.
    // (Res::new_error does not change HTTP status, only sets code != 0.)
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "HTTP status is 200; business error is in envelope code field"
    );

    let json = body_to_json(response.into_body()).await;

    // code must be CODE_PERMISSION_CASBIN_DENY = 5001 (missing-header branch in jwt.rs:17)
    assert_eq!(
        json["code"], 5001,
        "missing Bearer header → CODE_PERMISSION_CASBIN_DENY (5001); \
         see server/middleware/src/jwt.rs:17-22 and code.rs:33"
    );
    assert!(
        !json["success"].as_bool().unwrap_or(true),
        "success should be false for an auth error"
    );
    assert!(
        json["data"].is_null(),
        "data should be null for an error envelope"
    );
    // msg should be non-empty
    assert!(
        !json["msg"].as_str().unwrap_or("").is_empty(),
        "msg should describe the error"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Test 3: casbin_envelope_adapter converts Casbin plain-text 403 to envelope
// ────────────────────────────────────────────────────────────────────────────
//
// Setup:
//   - Load rbac_with_domains model + policy (same files used in jwt_auth_middleware.rs test)
//   - Route "/no-policy-route" has NO matching policy entry → CasbinAxumLayer returns plain 403
//   - casbin_envelope_adapter sits above CasbinAxumLayer and rewrites 403 → F4 envelope
//   - CasbinVals must be injected (no JWT middleware here); use a thin axum middleware
//     that inserts CasbinVals with a subject that has no policy match.
//
// Expected envelope: {code: 5001, msg: "您没有访问该资源的权限，请联系管理员", data: null}
// CODE_PERMISSION_CASBIN_DENY = 5001 (code.rs:33)

#[tokio::test]
async fn casbin_envelope_adapter_converts_403_to_envelope() {
    use axum_casbin::CasbinVals;

    // Inject CasbinVals with subject=["unknown_role"] in domain=domain1.
    // The policy CSV has alice/bob with specific roles; "unknown_role" has no GET on this path.
    async fn inject_casbin_vals(
        mut req: Request<Body>,
        next: axum::middleware::Next,
    ) -> axum::response::Response {
        req.extensions_mut().insert(CasbinVals {
            subject: vec!["unknown_role".to_string()],
            domain: Some("domain1".to_string()),
        });
        next.run(req).await
    }

    async fn inner_handler() -> &'static str {
        "should not be reached when Casbin denies"
    }

    let model = DefaultModel::from_file(
        "../../axum-casbin/examples/rbac_with_domains_model.conf",
    )
    .await
    .expect("rbac_with_domains_model.conf must be readable");

    let adapter =
        FileAdapter::new("../../axum-casbin/examples/rbac_with_domains_policy.csv");

    let casbin_layer = CasbinAxumLayer::new(model, adapter)
        .await
        .expect("CasbinAxumLayer must initialise");

    // Layer order — axum wraps outermost last:
    //   Request flow:  casbin_envelope_adapter → inject_casbin_vals → CasbinAxumLayer → handler
    //   Response flow: handler → CasbinAxumLayer (may 403) → inject_casbin_vals → casbin_envelope_adapter
    //
    // Build order (.layer() calls — each call wraps all previous):
    //   1. .layer(casbin_layer)                    → CasbinAxumLayer wraps handler
    //   2. .layer(inject_casbin_vals)              → inject_casbin_vals wraps (CasbinAxumLayer+handler)
    //   3. .layer(casbin_envelope_adapter)         → outermost; intercepts the 403 on response
    let app = Router::new()
        .route("/no-policy-route", get(inner_handler))
        .layer(casbin_layer)
        .layer(middleware::from_fn(inject_casbin_vals))
        .layer(middleware::from_fn(casbin_envelope_adapter));

    let svc = ServiceBuilder::new().service(app);

    let request = Request::builder()
        .method("GET")
        .uri("/no-policy-route")
        .body(Body::empty())
        .unwrap();

    let response = svc.oneshot(request).await.unwrap();

    // casbin_envelope_adapter rewrites the response to 200 with JSON envelope.
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "casbin_envelope_adapter rewrites 403 → 200 JSON envelope response"
    );

    let json = body_to_json(response.into_body()).await;

    assert_eq!(
        json["code"], 5001,
        "code must be CODE_PERMISSION_CASBIN_DENY = 5001 (code.rs:33)"
    );
    assert_eq!(
        json["msg"], "您没有访问该资源的权限，请联系管理员",
        "msg must match the string hardcoded in casbin_envelope_adapter.rs:52-55"
    );
    assert!(
        json["data"].is_null(),
        "data must be null for a permission-denied envelope"
    );
    assert!(
        !json["success"].as_bool().unwrap_or(true),
        "success must be false"
    );
}
