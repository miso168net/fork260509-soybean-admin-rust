//! F4 Dimension B — camelCase serialization.
//! Covers spec.md Acceptance Scenarios 6-8 + FR-010 / FR-011 / FR-014 + SC-004 (snake_case grep).

use serde_json::Value;
use server_model::admin::output::{AuthOutput, MenuRoute, RouteMeta, UserInfoOutput, UserRoute};

#[test]
fn scenario_b6_refresh_token_serializes_as_camel_case() {
    let auth = AuthOutput {
        token: "tok".to_string(),
        refresh_token: "rtok".to_string(),
    };
    let v = serde_json::to_value(&auth).unwrap();
    assert!(v.get("refreshToken").is_some(), "must have refreshToken (camelCase)");
    assert!(v.get("refresh_token").is_none(), "must NOT have refresh_token (snake_case)");
    assert_eq!(v["refreshToken"], "rtok");
}

#[test]
fn scenario_b7_nested_struct_camel_case_independent() {
    // UserRoute { routes: Vec<MenuRoute>, home } — outer + nested 各自需要 rename_all（FR-014）
    let route_meta = RouteMeta {
        title: "Home".to_string(),
        i18n_key: Some("route.home".to_string()),
        keep_alive: Some(true),
        constant: false,
        icon: Some("home".to_string()),
        order: 1,
        href: None,
        hide_in_menu: None,
        active_menu: None,
        multi_tab: None,
    };
    let menu_route = MenuRoute {
        name: "home".to_string(),
        path: "/home".to_string(),
        component: "layout.base$view.home".to_string(),
        meta: route_meta,
        children: None,
        id: 1,
        pid: "0".to_string(),
    };
    let user_route = UserRoute {
        routes: vec![menu_route],
        home: "home".to_string(),
    };
    let v = serde_json::to_value(&user_route).unwrap();

    // outer
    assert!(v.get("routes").is_some());
    assert!(v.get("home").is_some());

    // nested MenuRoute fields all single-word lowercase
    let inner: &Value = &v["routes"][0];
    assert!(inner.get("name").is_some());
    assert!(inner.get("meta").is_some());

    // nested RouteMeta camelCase
    let meta: &Value = &inner["meta"];
    assert!(meta.get("i18nKey").is_some(), "RouteMeta::i18n_key → i18nKey");
    assert!(meta.get("keepAlive").is_some(), "RouteMeta::keep_alive → keepAlive");
    assert!(meta.get("i18n_key").is_none(), "must NOT have snake_case i18n_key");
}

#[test]
fn scenario_b8_user_info_output_equivalence_after_per_field_rename_drop() {
    // 替換 per-field rename "userId"/"userName" 為 struct-level rename_all 後輸出 JSON 等價
    let info = UserInfoOutput {
        user_id: "u-001".to_string(),
        user_name: "Alice".to_string(),
        roles: vec!["admin".to_string()],
        buttons: vec![],
    };
    let v = serde_json::to_value(&info).unwrap();

    // 等價性: per-field rename 時是 "userId"/"userName"；struct-level rename_all 也是 "userId"/"userName"
    assert_eq!(v["userId"], "u-001");
    assert_eq!(v["userName"], "Alice");
    assert!(v.get("user_id").is_none());
    assert!(v.get("user_name").is_none());
}

#[test]
fn sc004_no_snake_case_keys_in_sample_admin_endpoint_outputs() {
    // SC-004: snake_case pattern [a-z]_[a-z] 命中數 = 0
    // 抽典型 admin output 結構 serialize 後 inspect 全 key（含巢狀）
    let auth_output = AuthOutput {
        token: "t".to_string(),
        refresh_token: "r".to_string(),
    };
    let user_info = UserInfoOutput {
        user_id: "u".to_string(),
        user_name: "n".to_string(),
        roles: vec![],
        buttons: vec![],
    };
    let route_meta = RouteMeta {
        title: "Home".to_string(),
        i18n_key: Some("route.home".to_string()),
        keep_alive: Some(true),
        constant: false,
        icon: None,
        order: 0,
        href: None,
        hide_in_menu: None,
        active_menu: None,
        multi_tab: None,
    };
    let menu_route = MenuRoute {
        name: "home".to_string(),
        path: "/home".to_string(),
        component: "layout.base$view.home".to_string(),
        meta: route_meta,
        children: None,
        id: 1,
        pid: "0".to_string(),
    };
    let user_route = UserRoute {
        routes: vec![menu_route],
        home: "home".to_string(),
    };

    for v in &[
        serde_json::to_value(&auth_output).unwrap(),
        serde_json::to_value(&user_info).unwrap(),
        serde_json::to_value(&user_route).unwrap(),
    ] {
        let snake_keys = collect_snake_case_keys(v);
        assert!(
            snake_keys.is_empty(),
            "snake_case keys found in JSON {}: {:?}",
            v,
            snake_keys
        );
    }
}

/// Recursively collect any object key that contains an ASCII lowercase
/// snake_case pattern (e.g. `i18n_key`, `refresh_token`).
fn collect_snake_case_keys(v: &Value) -> Vec<String> {
    let mut hits = Vec::new();
    walk(v, &mut hits);
    hits
}

fn walk(v: &Value, hits: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if is_snake_case_key(k) {
                    hits.push(k.clone());
                }
                walk(val, hits);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, hits);
            }
        }
        _ => {}
    }
}

fn is_snake_case_key(k: &str) -> bool {
    // Matches "<lower>+_<lower>...": at least one '_' surrounded by lowercase letters.
    let bytes = k.as_bytes();
    for i in 1..bytes.len().saturating_sub(1) {
        if bytes[i] == b'_'
            && bytes[i - 1].is_ascii_lowercase()
            && bytes[i + 1].is_ascii_lowercase()
        {
            return true;
        }
    }
    false
}
