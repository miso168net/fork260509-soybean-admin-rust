//! F5.1 Casbin envelope adapter — 將 axum-casbin reject 的 plain text 403 response
//! 轉成 F4 envelope `{code: CODE_PERMISSION_CASBIN_DENY, msg, data}`。
//!
//! axum-casbin/middleware.rs:153-197 reject 直接返 plain text body、未 hook F4 envelope；
//! F5.1 範圍內補此 thin adapter middleware、確保 base-web error handler 統一處理。
//!
//! per F5.1 spec FR-017 + Q2 拍板 + data-model.md §E2。

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use server_core::web::{code::CODE_PERMISSION_CASBIN_DENY, res::Res};

/// 攔 axum-casbin reject 的 plain text 403、轉 F4 envelope。
///
/// 處理範圍：
/// - HTTP 403 + body 含 "do not have the necessary permissions" 字樣 → F4 envelope CODE_PERMISSION_CASBIN_DENY
/// - HTTP 401 / 502 / 其他 status → 不動（jwt_auth_middleware 或 inner handler 處理）
pub async fn casbin_envelope_adapter(req: Request, next: Next) -> Response {
    let response = next.run(req).await;

    // 只攔 403 Forbidden 且 body 為 Casbin plain text
    if response.status() != StatusCode::FORBIDDEN {
        return response;
    }

    let (parts, body) = response.into_parts();
    let bytes = match to_bytes(body, 1024).await {
        Ok(b) => b,
        Err(_) => {
            // body 讀失敗、保 original response（不破壞）
            return Response::from_parts(parts, Body::empty());
        }
    };

    let body_str = std::str::from_utf8(&bytes).unwrap_or("");

    // 判斷是否為 Casbin reject 訊息（axum-casbin/middleware.rs:160-163 / 194-197 plain text）
    let is_casbin_reject = body_str.contains("do not have the necessary permissions")
        || body_str.contains("Please contact support if you believe this is an error");

    if !is_casbin_reject {
        // 非 Casbin reject（如其他 inner handler 返 403）、保 original
        return Response::from_parts(parts, Body::from(bytes));
    }

    // Casbin reject → 轉 F4 envelope
    Res::<()>::new_error(
        CODE_PERMISSION_CASBIN_DENY,
        "您没有访问该资源的权限，请联系管理员",
    )
    .into_response()
}
