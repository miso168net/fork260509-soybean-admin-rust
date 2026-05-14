//! F2.1 `AuditSerialize` trait + `audit_snapshot` helper + 7 admin entity impls.
//!
//! 提供「serialize entity Model 為 JSON、並 shallow redact 敏感 top-level field」能力，
//! 供 `audit_log::write_in_txn` caller 構造 `AuditEvent::payload_before/after` 用。
//!
//! per spec FR-008/009 + data-model.md §E5。
//!
//! Note: rev1 7 個 admin entity Model 都已 `#[serde(rename_all = "camelCase")]`（F4 落地）。
//! `redacted_fields()` 列出的 field 名須為 **camelCase**：
//! - `sys_user::Model.password` → camelCase `"password"`（單字無變化）
//! - `sys_access_key::Model.access_key_secret` → camelCase `"accessKeySecret"`

use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::admin::entities::{
    sys_access_key, sys_domain, sys_endpoint, sys_menu, sys_organization, sys_role, sys_user,
};

/// audit payload serialization trait — `redacted_fields()` 列出的 top-level field 將
/// 由 `audit_snapshot` 替換為 `"<redacted>"`。預設返空 slice。
pub trait AuditSerialize: Serialize {
    fn redacted_fields() -> &'static [&'static str] {
        &[]
    }
}

/// F2.1 audit payload snapshot — serialize model + shallow redact top-level field。
///
/// serialize 失敗時容錯返 `JsonValue::Null`（不 panic、不破壞業務 transaction）。
pub fn audit_snapshot<M: AuditSerialize>(model: &M) -> JsonValue {
    let mut v = serde_json::to_value(model).unwrap_or(JsonValue::Null);
    if let Some(obj) = v.as_object_mut() {
        for field in M::redacted_fields() {
            if obj.contains_key(*field) {
                obj.insert(
                    field.to_string(),
                    JsonValue::String("<redacted>".to_string()),
                );
            }
        }
    }
    v
}

// =============================================================================
// 7 admin entity impls — sys_user / sys_access_key 含敏感欄位、其餘 5 個 default 空
// =============================================================================

impl AuditSerialize for sys_user::Model {
    fn redacted_fields() -> &'static [&'static str] {
        &["password"]
    }
}

impl AuditSerialize for sys_access_key::Model {
    fn redacted_fields() -> &'static [&'static str] {
        &["accessKeySecret"]
    }
}

impl AuditSerialize for sys_role::Model {}
impl AuditSerialize for sys_menu::Model {}
impl AuditSerialize for sys_domain::Model {}
impl AuditSerialize for sys_organization::Model {}
impl AuditSerialize for sys_endpoint::Model {}
