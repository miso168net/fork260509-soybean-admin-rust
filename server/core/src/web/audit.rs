//! F3 audit context types.
//!
//! `Actor` 描述「誰」觸發 audit 寫入（一般 user 或 system actor）；
//! `AuditLogCtx` 描述「寫什麼」（entity_type / description / request_id）。
//!
//! `server_model::admin::audit_log::write_in_txn` 使用這兩個型別、
//! 在 caller 提供的 transaction 內寫一筆 `sys_operation_log` row。
//!
//! 兩個構造途徑（per data-model.md §E2）：
//! - `Actor::system(name)` — system actor（domain 固定 `"_system"`）
//! - `Actor::from(&user)` — 從 axum `Extension<User>` 構造

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

/// audit 寫入時的 payload — 由 facade module 在呼叫 `audit_log::write_in_txn` 時填。
#[derive(Clone, Debug)]
pub struct AuditLogCtx<'a> {
    /// 觸發 audit 的 actor 引用
    pub actor: &'a Actor,
    /// 對應 `sys_operation_log.module_name`、固定字串（如 `"sys_user"`）
    pub entity_type: &'static str,
    /// 對應 `sys_operation_log.description`（如 `"SOFT_DELETE id=u-001"`）
    pub description: String,
    /// 可選的 request_id 透傳（HTTP 場景由 axum `Extension<RequestId>` 帶入；
    /// service-level audit 可 None）
    pub request_id: Option<String>,
}
