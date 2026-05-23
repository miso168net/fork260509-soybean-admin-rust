use serde::{Deserialize, Serialize};
use server_core::web::page::PageRequest;
use validator::Validate;

use crate::admin::entities::sea_orm_active_enums::Status;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePageRequest {
    #[serde(flatten)]
    pub page_details: PageRequest,
    pub keywords: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleInput {
    pub pid: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Code must be between 1 and 50 characters"
    ))]
    pub code: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Name must be between 1 and 50 characters"
    ))]
    pub name: String,
    pub status: Status,
    #[validate(length(max = 200, message = "Description must not exceed 200 characters"))]
    pub description: Option<String>,
}

pub type CreateRoleInput = RoleInput;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleInput {
    // 039 T030.5: base-web 對外傳 numeric display_id；handler 先 lookup ULID 再餵 service。
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub id: i64,
    #[serde(flatten)]
    pub role: RoleInput,
}

// W-FW3 systemManage transform layer: base-web-shaped add/update/delete role DTOs。
// 僅 Deserialize（無 Validate）— 必填欄位空值由 base-web 表單驗證把關，
// 後端轉換層只負責形狀對映。Add / Update 分為兩型：Update 多 id，刻意不共用。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAddRoleInput {
    pub role_name: String,
    pub role_code: String,
    pub role_desc: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageUpdateRoleInput {
    // 039 T030.5: base-web 對外傳 numeric display_id
    pub id: i64,
    pub role_name: String,
    pub role_code: String, // W-FW6 N4：直送 rust update_role（rust 端同步 Casbin policy 並 publish reload）
    pub role_desc: Option<String>,
    pub status: String,
}

// W-FW3 systemManage: DELETE /systemManage/deleteRole body-id payload
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRoleByBodyInput {
    // 039 T030.5: base-web 對外傳 numeric display_id
    pub id: i64,
}

// W-FW3 systemManage: DELETE /systemManage/batchDeleteRole payload
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteRoleInput {
    // 039 T030.5: base-web 對外傳 numeric display_id 清單
    pub ids: Vec<i64>,
}

// W-FW4 systemManage: POST /systemManage/assignRoleMenus payload
// 039: role_id 改 numeric display_id（i64）；menu_ids 維持 i32（sys_menu.id PK）。
// 讀 alias(getRoleMenuIds)走 path param，無 body DTO。
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignRoleMenusInput {
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,
    pub menu_ids: Vec<i32>,
}

// W-FW6 systemManage: POST /systemManage/updateRoleHome payload
// 用於設定 / 清除角色登入後預設首頁路由(對應 sys_menu.route_name)。
// home None / Some("") 皆視為「明示清除 home」、持久化為 NULL。
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleHomeInput {
    // 039: role_id 改 numeric display_id（i64）
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,
    /// None / Some("") 皆視為「明示清除 home」, 持久化為 NULL
    pub home: Option<String>,
}
