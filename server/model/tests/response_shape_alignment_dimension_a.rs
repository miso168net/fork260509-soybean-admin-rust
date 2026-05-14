//! F4 Dimension A — response envelope code routing.
//! Covers spec.md Acceptance Scenarios 1-5 + FR-001 / FR-002 / FR-003 / FR-004 / FR-005.

use serde_json::json;
use server_core::web::{code, res::Res};

#[test]
fn scenario_a1_success_envelope_has_code_0_and_success_true() {
    let r = Res::new_data(json!({"sample": "data"}));
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 0, "success path must have code=0 (per FR-003 + CODE_SUCCESS)");
    assert_eq!(v["success"], true);
    assert_eq!(v["msg"], "success");
    assert!(v.get("data").is_some());
}

#[test]
fn scenario_a2_logout_code_8889_credential_rotated() {
    // 對應 base VITE_SERVICE_LOGOUT_CODES=8888,8889 觸發 immediate logout flow
    let r = Res::<()>::new_error(code::CODE_LOGOUT_CREDENTIAL_ROTATED, "credential rotated");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 8889);
    assert_eq!(v["success"], false);
    assert!(v.get("data").is_some()); // data 為 null
}

#[test]
fn scenario_a3_modal_logout_code_7778_account_suspended() {
    // 對應 base VITE_SERVICE_MODAL_LOGOUT_CODES=7777,7778 觸發 modal logout
    let r = Res::<()>::new_error(code::CODE_MODAL_LOGOUT_ACCOUNT_SUSPENDED, "account suspended");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 7778);
    assert_eq!(v["success"], false);
}

#[test]
fn scenario_a4_expired_access_token_code_9999() {
    // 對應 base VITE_SERVICE_EXPIRED_TOKEN_CODES=9999,9998,3333 觸發 auto-refresh
    let r = Res::<()>::new_error(code::CODE_EXPIRED_ACCESS_TOKEN, "access token expired");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 9999);
    assert_eq!(v["success"], false);
}

#[test]
fn scenario_a5_validation_error_code_4001_goes_to_default_handler() {
    // 4xxx 不在 base 任何 list → default error handler 顯示 msg
    let r = Res::<()>::new_error(code::CODE_VALIDATION_REQUIRED_FIELD, "field 'email' is required");
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["code"], 4001);
    assert_eq!(v["success"], false);
    assert_eq!(v["msg"], "field 'email' is required");
}

#[test]
fn middleware_error_5xxx_lands_in_envelope_per_fr_021() {
    // FR-021: middleware error MUST 經 IntoResponse 轉 envelope；驗證 5xxx 群 code 能正常成 envelope
    for &c in &[
        code::CODE_PERMISSION_CASBIN_DENY,
        code::CODE_PERMISSION_ROLE_INSUFFICIENT,
        code::CODE_PERMISSION_API_KEY_MISSING,
        code::CODE_PERMISSION_API_KEY_SIGNATURE_INVALID,
    ] {
        let r = Res::<()>::new_error(c, "permission denied");
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["code"], c, "code constant {} should serialize unchanged", c);
        assert_eq!(v["success"], false);
    }
}
