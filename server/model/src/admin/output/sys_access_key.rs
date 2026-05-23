//! 040 wire-id-consistency D3: raw endpoint output wire DTO for sys_access_key.
//! 隔離 Sea-ORM Model（internal SoT、`id: ULID` + `display_id: i64`）與 wire 表示（含 numeric display_id as id）。
//! wire 上 `id: i64`、無 `displayId` 重複欄；不含 `access_key_secret`（敏感）、不含 `deleted_at`（internal）。

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::admin::entities::{sea_orm_active_enums::Status, sys_access_key};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKeyDetail {
    pub id: i64,
    pub domain: String,
    pub access_key_id: String,
    pub status: Status,
    pub description: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: String,
}

impl From<sys_access_key::Model> for AccessKeyDetail {
    fn from(m: sys_access_key::Model) -> Self {
        Self {
            id: m.display_id,
            domain: m.domain,
            access_key_id: m.access_key_id,
            status: m.status,
            description: m.description,
            created_at: m.created_at,
            created_by: m.created_by,
        }
    }
}
