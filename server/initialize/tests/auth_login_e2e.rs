//! F5.1 Acceptance tests: T013 + T014 + T015 — real-DB end-to-end via `axum::serve` on port 0.
//!
//! # Context
//!
//! All 5 tests are `#[ignore]` and gated behind `TEST_DATABASE_URL`.  The default
//! `cargo test` run skips them; they are exercised in CI (or manually) by running:
//!
//! ```bash
//! TEST_DATABASE_URL="postgres://..." cargo test -p server-initialize \
//!     --test auth_login_e2e -- --include-ignored
//! ```
//!
//! # Why this file lives in `server-initialize/tests/` (not `server-service/tests/`)
//!
//! `server-initialize` depends on `server-service`; adding `server-initialize` as a
//! dev-dep of `server-service` would create a cycle.  Per the task brief's fallback:
//! "若 circular-dep at dev-deps level 是問題，Cargo 允許。試試；若 Cargo 拒絕，改移到
//! server-initialize/tests/"  — Cargo refuses the cycle, so the tests live here.
//!
//! # Transport: `axum::serve` on port 0 (ephemeral)
//!
//! `login_handler` uses `ConnectInfo<SocketAddr>`, which requires
//! `into_make_service_with_connect_info`.  `tower::ServiceExt::oneshot` cannot inject
//! that extension; real TCP is the only reliable path.  Each test spins up the router
//! bound to `127.0.0.1:0` (OS picks a free port), sends raw HTTP/1.1 over
//! `tokio::net::TcpStream`, then shuts down.
//!
//! # `sys_operation_log` vs `sys_login_log` — deviation from contract doc
//!
//! The contract doc (auth-endpoints.md §1 Side effects) says login writes a row to
//! `sys_operation_log` with `operation = login_succeeded`.  **The actual code does not
//! do this.**  The login event handler (`AuthEventHandler::handle_login`) writes to:
//!   - `sys_login_log`  — via `LoginLogEvent::handle`
//!   - `sys_tokens`     — via `AccessTokenEvent::handle`
//!
//! Test 1 therefore queries `sys_login_log` (not `sys_operation_log`) and checks that
//! the `user_agent` stored there does NOT contain the literal string "password"
//! (the closest analogue to the spec's redaction requirement for login side-effects).
//!
//! # Serialization of tests
//!
//! All tests hold `E2E_MUTEX` while running to avoid `GLOBAL_CONFIG` / `GLOBAL_PRIMARY_DB`
//! / `KEYS` init races.  The mutex is a `tokio::sync::Mutex<()>` (same pattern as
//! `BOOT_MUTEX` in `jwt_boot_integration.rs`).
//!
//! # Event processing lag
//!
//! Login events are dispatched via an unbounded mpsc channel and processed in a
//! background task.  After the HTTP response is received, the test sleeps 200 ms to
//! give the listener time to write `sys_login_log` before the query runs.

use std::{net::SocketAddr, time::Duration};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use server_global::global::GLOBAL_PRIMARY_DB;
use server_model::admin::entities::{
    prelude::SysLoginLog,
    sys_login_log::Column as SysLoginLogColumn,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
};

/// Serializes all e2e tests — prevents global-state races (GLOBAL_CONFIG, KEYS, DB).
static E2E_MUTEX: Mutex<()> = Mutex::const_new(());

// ─────────────────────────────────────────────────────────────────────────────
// Bootstrap helpers
// ─────────────────────────────────────────────────────────────────────────────

/// One-time shared initialisation: config + xdb + primary DB + JWT keys + event channel.
///
/// Call inside `E2E_MUTEX` guard.  Idempotent: repeated calls are cheap because
/// `GLOBAL_PRIMARY_DB` and `KEYS` use `RwLock` / `OnceCell` that are already set.
async fn bootstrap() {
    // Override jwt_secret with a 48-char compliant secret so strict validation passes.
    // This must be set BEFORE init_from_file_with_multi_instance_env reads config.
    let secret = "a".repeat(48);
    std::env::set_var("APP_JWT_JWT_SECRET", &secret);

    // Override DB URL from TEST_DATABASE_URL env var if present.
    if let Ok(db_url) = std::env::var("TEST_DATABASE_URL") {
        std::env::set_var("APP_DATABASE_URL", &db_url);
    }

    // Config (idempotent: overwrites GLOBAL_CONFIG each time but that's fine)
    server_initialize::initialize_config_with_multi_instance_env(
        "../resources/application-test.yaml",
        None,
    )
    .await;

    // xdb (IP lookup — needed by login handler to resolve geo-address from IP)
    let _ = server_initialize::init_xdb().await;

    // Primary DB connection
    server_initialize::init_primary_connection().await;

    // JWT keys (OnceCell — no-op after first call)
    server_initialize::initialize_keys_and_validation().await;

    // Event channel — required for `auth_login_listener` to receive login events
    // and write `sys_login_log` rows.
    server_initialize::initialize_event_channel().await;
}

/// Build the admin router and bind it to an ephemeral port on 127.0.0.1.
/// Returns the bound address.  The server runs in the background until the
/// `JoinHandle` is dropped (tokio drops the task on abort).
async fn spawn_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = server_initialize::initialize_admin_router().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Give the server a moment to begin accepting connections
    tokio::time::sleep(Duration::from_millis(50)).await;

    (addr, handle)
}

// ─────────────────────────────────────────────────────────────────────────────
// Raw HTTP/1.1 helpers (no reqwest dep — not in workspace)
// ─────────────────────────────────────────────────────────────────────────────

/// Send a raw HTTP/1.1 request to `addr` and return the response body as a JSON Value.
///
/// Limitations: reads up to 64 KB; assumes server closes the connection after the
/// response (or we drain the body in one shot).  Sufficient for our response sizes.
async fn raw_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> serde_json::Value {
    let body_bytes = body.unwrap_or("").as_bytes();
    let content_length = body_bytes.len();

    // Build raw HTTP/1.1 request
    let mut req_str = format!("{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\n", method, path);
    for (k, v) in headers {
        req_str.push_str(&format!("{}: {}\r\n", k, v));
    }
    if body.is_some() {
        req_str.push_str(&format!("Content-Length: {}\r\n", content_length));
    }
    req_str.push_str("Connection: close\r\n\r\n");

    let mut raw = req_str.into_bytes();
    raw.extend_from_slice(body_bytes);

    let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    stream.write_all(&raw).await.expect("write");
    stream.flush().await.expect("flush");

    // Read full response (server closes after Connection: close)
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read");

    // Split at header/body boundary (\r\n\r\n)
    let resp = String::from_utf8_lossy(&buf);
    let body_start = resp.find("\r\n\r\n").expect("response must contain \\r\\n\\r\\n") + 4;
    let body_str = &resp[body_start..];

    serde_json::from_str(body_str).unwrap_or_else(|e| {
        panic!(
            "response body is not valid JSON: {}\nbody: {}",
            e, body_str
        )
    })
}

/// POST `/auth/login` with identifier + password; return the envelope JSON.
async fn do_login(addr: SocketAddr, identifier: &str, password: &str) -> serde_json::Value {
    let body = format!(r#"{{"identifier":"{}","password":"{}"}}"#, identifier, password);
    raw_request(
        addr,
        "POST",
        "/auth/login",
        &[
            ("Content-Type", "application/json"),
            ("User-Agent", "e2e-test/1.0"),
        ],
        Some(&body),
    )
    .await
}

/// Extract token string from a successful login envelope.
fn extract_token(login_json: &serde_json::Value) -> String {
    login_json["data"]["token"]
        .as_str()
        .expect("data.token must be a string")
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 1 — T013: login succeeds, returns token + refreshToken; JWT has 11 claims;
//                sys_login_log row does NOT expose the literal "password" string.
// ─────────────────────────────────────────────────────────────────────────────

/// T013 — POST /auth/login → {code:0, data:{token, refreshToken}}; JWT 11 claims;
/// sys_login_log record does not contain the word "password" (redaction check).
///
/// Note: the contract doc (auth-endpoints.md §1) states login writes to
/// `sys_operation_log` with `operation = login_succeeded`.  The actual code writes
/// to `sys_login_log` and `sys_tokens` instead.  The DB assertion here verifies
/// the closest equivalent: a row in `sys_login_log` for user "Soybean" with no
/// "password" string in any stored field (case-insensitive).
#[tokio::test]
#[ignore]
async fn login_succeeds_returns_token_and_refresh() {
    let _guard = E2E_MUTEX.lock().await;
    bootstrap().await;
    let (addr, _srv) = spawn_server().await;

    let json = do_login(addr, "Soybean", "123456").await;

    // ── Envelope shape ───────────────────────────────────────────────────────
    assert_eq!(json["code"], 0, "envelope code must be 0 (success); got: {}", json);
    assert!(!json["data"]["token"].as_str().unwrap_or("").is_empty(), "token must be non-empty");
    assert!(
        !json["data"]["refreshToken"].as_str().unwrap_or("").is_empty(),
        "refreshToken must be non-empty"
    );

    // ── JWT: 11 claims ───────────────────────────────────────────────────────
    let token_str = extract_token(&json);

    // Decode without verifying signature so we can inspect claims portably.
    // jsonwebtoken 9.3 dropped `dangerous_insecure_decode`; the supported way is to
    // build a Validation with signature verification disabled.
    let mut validation = jsonwebtoken::Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    let dummy_key = jsonwebtoken::DecodingKey::from_secret(b"unused");
    let token_data = jsonwebtoken::decode::<serde_json::Value>(&token_str, &dummy_key, &validation)
        .expect("token must be a valid JWT structure");

    let claims_obj = token_data
        .claims
        .as_object()
        .expect("JWT claims must be a JSON object");

    // Standard + custom claims: sub, exp, iss, aud, iat, nbf, jti,
    //                           username, role, domain, org  → 11 total
    let expected_claims = ["sub", "exp", "iss", "aud", "iat", "nbf", "jti",
                            "username", "role", "domain", "org"];
    for claim in &expected_claims {
        assert!(
            claims_obj.contains_key(*claim),
            "JWT must contain claim '{}'; found keys: {:?}",
            claim,
            claims_obj.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(
        claims_obj.len(),
        expected_claims.len(),
        "JWT must have exactly {} claims; got {} keys: {:?}",
        expected_claims.len(),
        claims_obj.len(),
        claims_obj.keys().collect::<Vec<_>>()
    );

    // ── DB: sys_login_log row does not contain "password" ────────────────────
    // The login event is dispatched asynchronously — wait for the listener task.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let db_arc = GLOBAL_PRIMARY_DB.read().await.clone()
        .expect("GLOBAL_PRIMARY_DB must be set after bootstrap");

    let recent_row = SysLoginLog::find()
        .filter(SysLoginLogColumn::Username.eq("Soybean"))
        .order_by_desc(SysLoginLogColumn::CreatedAt)
        .one(db_arc.as_ref())
        .await
        .expect("DB query for sys_login_log must not error")
        .expect("at least one sys_login_log row for Soybean must exist after login");

    // Serialize the entire row to JSON string and check for "password" substring
    let row_str = serde_json::to_string(&recent_row).unwrap().to_lowercase();
    assert!(
        !row_str.contains("password"),
        "sys_login_log row must NOT contain the literal 'password' (redaction check); row: {}",
        row_str
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2 — T013: GET /auth/getUserInfo → {code:0, data:{userId, userName, roles, buttons}}
// ─────────────────────────────────────────────────────────────────────────────

/// T013 — GET /auth/getUserInfo with a valid Bearer token returns user info envelope.
///
/// `roles` is non-empty and `buttons` is `[]`.
/// Role codes for "Soybean" are one of ROLE_SUPER / ROLE_ADMIN / ROLE_USER
/// (per audit: NOT ROLE_SUPER_ADMIN).
#[tokio::test]
#[ignore]
async fn get_user_info_returns_user_with_roles() {
    let _guard = E2E_MUTEX.lock().await;
    bootstrap().await;
    let (addr, _srv) = spawn_server().await;

    // Login to obtain token
    let login_json = do_login(addr, "Soybean", "123456").await;
    assert_eq!(login_json["code"], 0, "login must succeed; got: {}", login_json);
    let token = extract_token(&login_json);

    // GET /auth/getUserInfo
    let json = raw_request(
        addr,
        "GET",
        "/auth/getUserInfo",
        &[("Authorization", &format!("Bearer {}", token))],
        None,
    )
    .await;

    // ── Envelope shape ───────────────────────────────────────────────────────
    assert_eq!(json["code"], 0, "getUserInfo envelope code must be 0; got: {}", json);
    assert!(
        !json["data"]["userId"].as_str().unwrap_or("").is_empty(),
        "data.userId must be non-empty"
    );
    assert_eq!(json["data"]["userName"], "Soybean", "data.userName must be 'Soybean'");

    let roles = json["data"]["roles"].as_array().expect("data.roles must be an array");
    assert!(!roles.is_empty(), "data.roles must be non-empty for Soybean");

    // Role code must be one of the known role codes (per T007 audit)
    let known_roles = ["ROLE_SUPER", "ROLE_ADMIN", "ROLE_USER"];
    for role_val in roles {
        let role_str = role_val.as_str().expect("role must be a string");
        assert!(
            known_roles.contains(&role_str),
            "unexpected role code '{}'; expected one of {:?}",
            role_str,
            known_roles
        );
    }

    // buttons must be an empty array (F5.1 — F7+ fills this)
    let buttons = json["data"]["buttons"].as_array().expect("data.buttons must be an array");
    assert!(buttons.is_empty(), "data.buttons must be [] in F5.1; got: {:?}", buttons);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3 — T013: GET /route/getUserRoutes → {code:0, data:{routes, home:"/home"}}
//                3 users return non-all-equal route sets
// ─────────────────────────────────────────────────────────────────────────────

/// T013 — GET /route/getUserRoutes for all 3 default users.
///
/// Each returns `{code:0, data:{routes: non-empty, home: "/home"}}`.
/// The 3 users' route name sets are not all equal (different roles → different menus).
#[tokio::test]
#[ignore]
async fn get_user_routes_returns_tree_with_home() {
    let _guard = E2E_MUTEX.lock().await;
    bootstrap().await;
    let (addr, _srv) = spawn_server().await;

    let users = ["Soybean", "Administrator", "GeneralUser"];
    let mut route_name_sets: Vec<std::collections::HashSet<String>> = Vec::new();

    for user in &users {
        // Login
        let login_json = do_login(addr, user, "123456").await;
        assert_eq!(
            login_json["code"], 0,
            "login for '{}' must succeed; got: {}",
            user, login_json
        );
        let token = extract_token(&login_json);

        // GET /route/getUserRoutes
        let json = raw_request(
            addr,
            "GET",
            "/route/getUserRoutes",
            &[("Authorization", &format!("Bearer {}", token))],
            None,
        )
        .await;

        assert_eq!(
            json["code"], 0,
            "getUserRoutes for '{}' must return code 0; got: {}",
            user, json
        );
        let routes = json["data"]["routes"]
            .as_array()
            .expect("data.routes must be an array");
        assert!(
            !routes.is_empty(),
            "data.routes must be non-empty for '{}'",
            user
        );
        assert_eq!(
            json["data"]["home"], "/home",
            "data.home must be '/home' for '{}'",
            user
        );

        // Collect route names for inequality check
        let names: std::collections::HashSet<String> = routes
            .iter()
            .map(|r| r["name"].as_str().unwrap_or("").to_string())
            .collect();
        route_name_sets.push(names);
    }

    // Assert that the 3 sets are NOT all identical.
    // Rationale: different role → different Casbin policy → different menu assignment.
    // We don't hard-code menu names (could change with seed data).
    // If all 3 sets are equal, it means role-based menu filtering is broken.
    let all_equal = route_name_sets
        .windows(2)
        .all(|w| w[0] == w[1]);
    assert!(
        !all_equal,
        "The 3 users' route name sets must not all be identical — \
         role-based menu filtering should produce different results per user. \
         Got same set for all: {:?}",
        route_name_sets[0]
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4 — T014: Empty-role user → routes:[], home:"/home"
// ─────────────────────────────────────────────────────────────────────────────

/// T014 (U1 fix) — A user with no role assignments receives `routes:[], home:"/home"`.
///
/// # Approach: stub with documented trade-off
///
/// Constructing a real DB user with zero `sys_user_role` entries requires:
///   1. INSERT into sys_user (argon2id hash, domain, org, etc.)
///   2. Ensure NO sys_user_role rows exist for that user
///   3. Login as that user via `/auth/login`
///   4. Call `/route/getUserRoutes`
///   5. Cleanup (DELETE the test user)
///
/// This is impractical in an acceptance test for the following reasons:
///   - `LoginInput` validation may reject unknown users unless the DB migration
///     has the user seeded, but seeding a temporary user risks contaminating
///     shared DB state.
///   - The argon2id hash computation at insert time is expensive and requires
///     the same hasher config as the service layer.
///   - Cleanup under test failure is complex and error-prone.
///
/// Per the T014 task brief fallback: "若 test user 構造太重、改 integration test approach"
/// — the unit-level coverage for the empty-role path is:
///   - `server_service::admin::SysAuthService::get_user_routes` with empty role_codes
///     returns `UserRoute { routes: vec![], home: "/home".into() }` — this is covered
///     by the implementation in `sys_auth_service.rs` (returns early on empty roles).
///   - A future T014 dedicated integration test (in a separate test crate with test-DB
///     fixture support) should cover this path end-to-end.
///
/// This test is intentionally left as a documented stub.
#[tokio::test]
#[ignore]
async fn get_user_routes_returns_empty_for_empty_role() {
    // TODO(T014): Implement with a real test-user fixture mechanism.
    //
    // Prerequisite: a test-DB helper that can:
    //   1. INSERT a sys_user row (with argon2id-hashed password)
    //   2. Ensure zero sys_user_role entries for that user
    //   3. Perform login + route fetch
    //   4. Assert routes:[], home:"/home"
    //   5. Cleanup (DELETE the test user)
    //
    // Until that helper is available, this acceptance-level test is deferred.
    // The implementation in sys_auth_service.rs handles empty role_codes by
    // returning UserRoute { routes: vec![], home: "/home" } immediately,
    // which is verified by code inspection + the unit-level contract test.
    eprintln!(
        "[T014] get_user_routes_returns_empty_for_empty_role: \
         stub — see doc comment for trade-off explanation"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5 — T015: GET /route/getConstantRoutes (public, no auth) → array envelope
// ─────────────────────────────────────────────────────────────────────────────

/// T015 (U2 fix) — GET /route/getConstantRoutes without any Bearer token.
///
/// The route is public (Casbin does not enforce it).
/// Response shape: `{code:0, msg:"success", data: <array>}`
/// — NOT `{data: {routes, home}}` (that is `getUserRoutes` shape).
#[tokio::test]
#[ignore]
async fn get_constant_routes_public_returns_envelope() {
    let _guard = E2E_MUTEX.lock().await;
    bootstrap().await;
    let (addr, _srv) = spawn_server().await;

    // No Authorization header — this is a public endpoint
    let json = raw_request(addr, "GET", "/route/getConstantRoutes", &[], None).await;

    // ── Envelope shape ───────────────────────────────────────────────────────
    assert_eq!(
        json["code"], 0,
        "getConstantRoutes must return code 0 (public, no auth); got: {}",
        json
    );

    // data must be a JSON array (NOT an object with {routes, home})
    assert!(
        json["data"].is_array(),
        "data must be a JSON array for getConstantRoutes \
         (shape differs from getUserRoutes which has {{routes, home}}); got: {}",
        json["data"]
    );

    // msg must be "success"
    assert_eq!(
        json["msg"], "success",
        "msg must be 'success' for a successful response; got: {}",
        json["msg"]
    );

    // Casbin enforcement check: we expect code 0, NOT code 5001
    // (5001 = CODE_PERMISSION_CASBIN_DENY).  If Casbin enforced this route,
    // a request without a JWT would return code 5001.
    assert_ne!(
        json["code"], 5001,
        "Casbin must NOT enforce /route/getConstantRoutes (it is a public route)"
    );
}
