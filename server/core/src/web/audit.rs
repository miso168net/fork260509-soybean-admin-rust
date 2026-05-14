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

use serde_json::Value as JsonValue;

use crate::web::auth::User;

/// 寫 audit 時的 actor 來源 — 一般 user / system actor 兩種建構途徑
#[derive(Clone, Debug)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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

// =============================================================================
// F3 過渡 — AuditLogCtx + From<&AuditEvent> shim
// =============================================================================
//
// 注：F2.1 G1 階段 facade 7 個檔已順手 shim 改為 AuditEvent + Internal source + payload_before/after=None
// （make compile pass、保 F3 行為）；fetch_before / fetch_after snapshot enrichment 留 G2 完整 refactor。
// 此處**暫不**加 `#[deprecated]` attribute — AuditLogCtx 留作 From<&AuditEvent> shim 過渡；
// G2 階段 audit_log::write_in_txn 介面與 facade snapshot 全到位後、再 cleanup pass 評估 deprecate / remove。

/// audit 寫入時的 payload — F3 既有結構、F2.1 為過渡層保留。
#[derive(Clone, Debug)]
pub struct AuditLogCtx<'a> {
    /// 觸發 audit 的 actor 引用
    pub actor: &'a Actor,
    /// 對應 `sys_operation_log.module_name`、固定字串（如 `"sys_user"`）
    pub entity_type: &'static str,
    /// 對應 `sys_operation_log.description`（如 `"SOFT_DELETE id=u-001"`）
    pub description: String,
    /// 可選的 request_id 透傳
    pub request_id: Option<String>,
}

impl<'a, 'b: 'a> From<&'a AuditEvent<'b>> for AuditLogCtx<'a> {
    fn from(event: &'a AuditEvent<'b>) -> Self {
        Self {
            actor: event.actor,
            entity_type: event.entity_type,
            description: event
                .description
                .clone()
                .unwrap_or_else(|| format!("{} id={}", event.operation, event.entity_id)),
            request_id: event.request_id.clone(),
        }
    }
}
