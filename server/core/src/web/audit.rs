//! F3 + F2.1 audit context types.
//!
//! `Actor` 描述「誰」觸發 audit 寫入（一般 user 或 system actor）。
//!
//! F3 既有 `AuditLogCtx` 為簡化版 audit payload（entity_type / description / request_id）；
//! F2.1 取代為 `AuditEvent<'a>` — 含 operation enum / entity_id / payload_before/after JSONB
//! / source enum 區分視角（Http / Internal / Cleanup）。
//!
//! `server_model::admin::audit_log::write_in_txn` 接 `AuditEvent`、
//! 在 caller 提供的 transaction 內寫一筆 `sys_operation_log` row。
//!
//! 兩個 Actor 構造途徑（per data-model.md §E2）：
//! - `Actor::system(name)` — system actor（domain 固定 `"_system"`）
//! - `Actor::from(&user)` — 從 axum `Extension<User>` 構造

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::web::auth::User;

/// 寫 audit 時的 actor 來源 — 一般 user / system actor 兩種建構途徑
///
/// 042: Serialize+Deserialize derived — outbox payload JSONB 需 serialize 寫入，
/// drainer 端 deserialize 後重建 sys_operation_log row（per data-model.md §E1）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    /// user_id（一般 caller）or system actor name（如 `"cleanup_job"` / `"migration"`）
    pub id: String,
    /// 顯示名（一般是 username；system actor 為 actor name）
    pub username: String,
    /// `user.domain`（一般 caller）or `"_system"`（system actor）
    pub domain: String,
}

impl Actor {
    /// 構造 system actor — 用於 cleanup job / migration 等非 user 觸發的 audit 寫入。
    /// `domain` 固定為 `"_system"`。
    pub fn system(name: &str) -> Self {
        Self {
            id: name.to_string(),
            username: name.to_string(),
            domain: "_system".to_string(),
        }
    }
}

impl From<&User> for Actor {
    fn from(u: &User) -> Self {
        Self {
            id: u.user_id(),
            username: u.username(),
            domain: u.domain(),
        }
    }
}

// =============================================================================
// F2.1 — AuditOperation / AuditSource / AuditEvent
// =============================================================================

/// write 操作分類（per spec FR-005 + data-model.md §E2）
///
/// 042: Serialize+Deserialize derived for outbox round-trip。
/// 序列化形式 = SCREAMING_SNAKE_CASE（對齊 `as_str()` + sys_operation_log.operation 欄值）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditOperation {
    Insert,
    Update,
    SoftDelete,
    Restore,
    /// F2.1 預留 enum 值、實作留 F12 cleanup-job
    HardDelete,
}

impl AuditOperation {
    /// 回 SCREAMING_SNAKE_CASE 字串、對應 sys_operation_log.operation 欄值
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditOperation::Insert => "INSERT",
            AuditOperation::Update => "UPDATE",
            AuditOperation::SoftDelete => "SOFT_DELETE",
            AuditOperation::Restore => "RESTORE",
            AuditOperation::HardDelete => "HARD_DELETE",
        }
    }
}

impl std::fmt::Display for AuditOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// audit 來源視角（per spec FR-005 + data-model.md §E3）
///
/// 042: Serialize+Deserialize derived；JSONB 內以 `{"type":"Http",...}` /
/// `{"type":"Internal"}` / `{"type":"Cleanup"}` 形式呈現（per data-model.md §E1
/// audit_event_json JSONB schema）。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuditSource {
    /// HTTP middleware audit — method 為 POST/PUT/PATCH/DELETE 等
    Http {
        method: String,
        url: String,
        ip: String,
        user_agent: Option<String>,
    },
    /// service-level audit（內部 method 呼叫、非 HTTP 觸發）
    Internal,
    /// F12 cleanup-job audit（F2.1 預留 variant）
    Cleanup,
}

/// audit 寫入單位（取代 F3 `AuditLogCtx`、per spec FR-005 + data-model.md §E4）
///
/// 042: 僅 Serialize derived（drainer 端用 [`AuditEventOwned`] 反序列化、
/// 因為 `&'a Actor` 與 `&'static str entity_type` 不能直接 Deserialize）。
#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent<'a> {
    pub actor: &'a Actor,
    pub operation: AuditOperation,
    /// 對應 sys_operation_log.module_name（如 `"sys_user"`）
    pub entity_type: &'static str,
    /// 對應 sys_operation_log.entity_id（sys_menu i32 用 to_string()）
    pub entity_id: String,
    /// 變動前 entity snapshot — INSERT=None / UPDATE/SoftDelete/Restore=Some
    pub payload_before: Option<JsonValue>,
    /// 變動後 entity snapshot — INSERT/UPDATE/Restore=Some / SoftDelete=None
    pub payload_after: Option<JsonValue>,
    /// 可選 human-readable summary（None 時 audit_log 自動填 `"{operation} id={entity_id}"`）
    pub description: Option<String>,
    pub source: AuditSource,
    pub request_id: Option<String>,
}

/// 042: owned 版本的 `AuditEvent`，drainer 端從 outbox JSONB deserialize 用。
///
/// 與 `AuditEvent<'a>` 對應、欄位名 / 序列化形式相同；差別僅在於 `actor` 改為擁有式
/// 且 `entity_type` 改為 `String`（無 `&'static`）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEventOwned {
    pub actor: Actor,
    pub operation: AuditOperation,
    pub entity_type: String,
    pub entity_id: String,
    pub payload_before: Option<JsonValue>,
    pub payload_after: Option<JsonValue>,
    pub description: Option<String>,
    pub source: AuditSource,
    pub request_id: Option<String>,
}

// F3 既有 AuditLogCtx + From<&AuditEvent> shim 在 G1 已被 7 facade 完全 refactor 走 AuditEvent —
// 0 active callsite (grep verified)、無 external crate 依賴、刪除無 downstream impact。
// per code-review 2026-05-14 Important #1。
