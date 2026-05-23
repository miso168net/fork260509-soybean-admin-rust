use chrono::NaiveDateTime;
use sea_orm::FromQueryResult;
use serde::Serialize;

use crate::admin::entities::{
    sea_orm_active_enums::{Gender, Status},
    sys_user::Model as SysUserModel,
};

#[derive(Debug, FromQueryResult)]
pub struct UserWithDomainAndOrgOutput {
    pub id: String,
    pub domain: String,
    pub username: String,
    pub password: String,
    pub nick_name: String,
    pub avatar: Option<String>,
    pub domain_code: String,
    pub domain_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserWithoutPassword {
    pub id: i64,
    pub domain: String,
    pub username: String,
    pub nick_name: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub status: Status,
    pub gender: Option<Gender>,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl From<SysUserModel> for UserWithoutPassword {
    fn from(model: SysUserModel) -> Self {
        Self {
            id: model.display_id,
            domain: model.domain,
            username: model.username,
            nick_name: model.nick_name,
            avatar: model.avatar,
            email: model.email,
            phone_number: model.phone_number,
            status: model.status,
            gender: model.gender,
            created_at: model.created_at,
            created_by: model.created_by,
            updated_at: model.updated_at,
            updated_by: model.updated_by,
        }
    }
}

/// 040 wire-id-consistency D2: raw endpoint output wire DTO for sys_user.
/// 與既有 `UserWithoutPassword` 並存（不重用，避免破壞 alias 層 ID 語意）。
/// wire 上 `id: i64`（從 `model.display_id`）、無 `displayId` 重複欄；不含 password（敏感）。
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDetail {
    pub id: i64,
    pub domain: String,
    pub username: String,
    pub nick_name: String,
    pub avatar: Option<String>,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub status: Status,
    pub gender: Option<Gender>,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl From<SysUserModel> for UserDetail {
    fn from(m: SysUserModel) -> Self {
        Self {
            id: m.display_id,
            domain: m.domain,
            username: m.username,
            nick_name: m.nick_name,
            avatar: m.avatar,
            email: m.email,
            phone_number: m.phone_number,
            status: m.status,
            gender: m.gender,
            created_at: m.created_at,
            created_by: m.created_by,
            updated_at: m.updated_at,
            updated_by: m.updated_by,
        }
    }
}

/// 040 T011: sys_user service 既有路徑回 `UserWithoutPassword`（非 raw `SysUserModel`）。
/// 為讓 raw endpoint handler 能 `.map(UserDetail::from)` wrap，補一條等價 From。
/// 兩 struct 欄位 1:1 對齊（同 13 欄、id 已是 i64）；純 field move、零轉換。
impl From<UserWithoutPassword> for UserDetail {
    fn from(u: UserWithoutPassword) -> Self {
        Self {
            id: u.id,
            domain: u.domain,
            username: u.username,
            nick_name: u.nick_name,
            avatar: u.avatar,
            email: u.email,
            phone_number: u.phone_number,
            status: u.status,
            gender: u.gender,
            created_at: u.created_at,
            created_by: u.created_by,
            updated_at: u.updated_at,
            updated_by: u.updated_by,
        }
    }
}
