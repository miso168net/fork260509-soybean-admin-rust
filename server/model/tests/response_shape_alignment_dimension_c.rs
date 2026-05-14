//! F4 Dimension C — auth & userInfo field shape.
//! Covers spec.md Acceptance Scenarios 9-11 + FR-015 / FR-016 / FR-017 + SC-003.

use serde_json::Value;
use server_core::web::res::Res;
use server_model::admin::output::{AuthOutput, UserInfoOutput};

#[test]
fn scenario_c9_auth_login_data_has_token_and_refresh_token() {
    let auth = AuthOutput {
        token: "tok".to_string(),
        refresh_token: "rtok".to_string(),
    };
    let res = Res::new_data(auth);
    let v = serde_json::to_value(&res).unwrap();
    let data: &Value = &v["data"];
    assert!(data.get("token").is_some());
    assert!(data.get("refreshToken").is_some());
    assert_eq!(data["token"], "tok");
    assert_eq!(data["refreshToken"], "rtok");
}

#[test]
fn scenario_c10_get_user_info_has_exactly_4_camel_case_keys() {
    let info = UserInfoOutput {
        user_id: "u-001".to_string(),
        user_name: "Alice".to_string(),
        roles: vec!["R_SUPER".to_string()],
        buttons: vec![],
    };
    let v = serde_json::to_value(&info).unwrap();
    let obj = v.as_object().expect("UserInfoOutput must serialize as JSON object");

    let expected: std::collections::BTreeSet<&str> =
        ["userId", "userName", "roles", "buttons"].iter().copied().collect();
    let actual: std::collections::BTreeSet<&str> = obj.keys().map(|s| s.as_str()).collect();
    assert_eq!(
        actual, expected,
        "UserInfoOutput JSON must have exactly {{userId, userName, roles, buttons}} keys"
    );

    // roles & buttons must be JSON arrays
    assert!(v["roles"].is_array());
    assert!(v["buttons"].is_array());
}

#[test]
fn scenario_c11_buttons_defaults_to_empty_array() {
    let info = UserInfoOutput {
        user_id: "u".to_string(),
        user_name: "n".to_string(),
        roles: vec![],
        buttons: vec![], // F4 階段預設 vec![]
    };
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["buttons"], serde_json::Value::Array(vec![]));
}
