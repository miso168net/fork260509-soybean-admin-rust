//! 040 wire-id-consistency D1: raw endpoint output wire DTO for sys_role.
//! 隔離 Sea-ORM Model（internal SoT、`id: ULID` + `display_id: i64`）與 wire 表示（含 numeric display_id as id）。
//! Sea-ORM Model 不動、本 DTO 為 wire-shape 包裝；wire 上 `id: i64`、無 `displayId` 重複欄。

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::admin::entities::{sea_orm_active_enums::Status, sys_role};

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDetail {
    pub id: i64,
    pub pid: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub status: Status,
    pub home_route_name: Option<String>,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl From<sys_role::Model> for RoleDetail {
    fn from(m: sys_role::Model) -> Self {
        Self {
            id: m.display_id,
            pid: m.pid,
            code: m.code,
            name: m.name,
            description: m.description,
            status: m.status,
            home_route_name: m.home_route_name,
            created_at: m.created_at,
            created_by: m.created_by,
            updated_at: m.updated_at,
            updated_by: m.updated_by,
        }
    }
}
