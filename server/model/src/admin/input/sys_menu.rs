use serde::{Deserialize, Deserializer, Serialize};
use server_core::web::page::PageRequest;
use validator::Validate;

use crate::admin::entities::sea_orm_active_enums::{MenuType, Status};

/// W-FW2 fix: parentId 可能是 number（建立路徑）或 string（編輯路徑，從 getMenuList 回傳值預填）。
/// 兩種形式都接受，統一反序列化成 i32。
fn deserialize_i32_or_string<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => n
            .as_i64()
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| serde::de::Error::custom(format!("parentId number out of i32 range: {}", n))),
        serde_json::Value::String(s) => s
            .parse::<i32>()
            .map_err(|_| serde::de::Error::custom(format!("parentId cannot parse string as i32: {:?}", s))),
        other => Err(serde::de::Error::custom(format!(
            "parentId expected number or string, got: {}",
            other
        ))),
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuPageRequest {
    #[serde(flatten)]
    pub page_details: PageRequest,
    pub keywords: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct MenuInput {
    pub menu_type: MenuType,
    #[validate(length(
        min = 1,
        max = 100,
        message = "Menu name must be between 1 and 100 characters"
    ))]
    pub menu_name: String,
    pub icon_type: Option<i32>,
    #[validate(length(max = 100, message = "Icon must not exceed 100 characters"))]
    pub icon: Option<String>,
    #[validate(length(
        min = 1,
        max = 100,
        message = "Route name must be between 1 and 100 characters"
    ))]
    pub route_name: String,
    #[validate(length(
        min = 1,
        max = 200,
        message = "Route path must be between 1 and 200 characters"
    ))]
    pub route_path: String,
    #[validate(length(
        min = 1,
        max = 200,
        message = "Component must be between 1 and 200 characters"
    ))]
    pub component: String,
    #[validate(length(max = 100, message = "Path param must not exceed 100 characters"))]
    pub path_param: Option<String>,
    pub status: Status,
    #[validate(length(max = 100, message = "Active menu must not exceed 100 characters"))]
    pub active_menu: Option<String>,
    pub hide_in_menu: Option<bool>,
    #[validate(length(min = 1, max = 50, message = "PID must be between 1 and 50 characters"))]
    pub pid: String,
    pub sequence: i32,
    #[validate(length(max = 100, message = "i18n key must not exceed 100 characters"))]
    pub i18n_key: Option<String>,
    pub keep_alive: Option<bool>,
    pub constant: bool,
    #[validate(length(max = 200, message = "Href must not exceed 200 characters"))]
    pub href: Option<String>,
    pub multi_tab: Option<bool>,
    #[serde(default)]
    pub query: Option<serde_json::Value>,
    #[serde(default)]
    pub buttons: Option<serde_json::Value>,
    #[serde(default)]
    pub fixed_index_in_tab: Option<i32>,
}

pub type CreateMenuInput = MenuInput;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMenuInput {
    pub id: i32,
    #[serde(flatten)]
    pub menu: MenuInput,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct IsRouteExistInput {
    #[validate(length(min = 1))]
    pub route_name: String,
}

// W-FW2 systemManage transform layer: base-web-shaped add/update/delete menu DTOs。
// 僅 Deserialize（無 Validate）— 必填欄位空值由 base-web 表單驗證把關，
// 後端轉換層只負責形狀對映。Add / Update 分為兩型：Update 多 id，刻意不共用。
// W-FW7: query / buttons / fixedIndexInTab 三欄已接通持久化、透傳至 native MenuInput。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAddMenuInput {
    pub menu_type: String,
    pub menu_name: String,
    pub route_name: String,
    pub route_path: String,
    pub component: String,
    pub order: i32,
    pub i18n_key: Option<String>,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub status: String,
    #[serde(deserialize_with = "deserialize_i32_or_string")]
    pub parent_id: i32,
    pub keep_alive: Option<bool>,
    pub constant: bool,
    pub href: Option<String>,
    pub hide_in_menu: Option<bool>,
    pub active_menu: Option<String>,
    pub multi_tab: Option<bool>,
    #[serde(default)]
    pub query: Option<serde_json::Value>,
    #[serde(default)]
    pub buttons: Option<serde_json::Value>,
    #[serde(default)]
    pub fixed_index_in_tab: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageUpdateMenuInput {
    pub id: i32,
    pub menu_type: String,
    pub menu_name: String,
    pub route_name: String,
    pub route_path: String,
    pub component: String,
    pub order: i32,
    pub i18n_key: Option<String>,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub status: String,
    #[serde(deserialize_with = "deserialize_i32_or_string")]
    pub parent_id: i32,
    pub keep_alive: Option<bool>,
    pub constant: bool,
    pub href: Option<String>,
    pub hide_in_menu: Option<bool>,
    pub active_menu: Option<String>,
    pub multi_tab: Option<bool>,
    #[serde(default)]
    pub query: Option<serde_json::Value>,
    #[serde(default)]
    pub buttons: Option<serde_json::Value>,
    #[serde(default)]
    pub fixed_index_in_tab: Option<i32>,
}

// W-FW2 systemManage: DELETE /systemManage/deleteMenu body-id payload
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteMenuByBodyInput {
    pub id: i32,
}

// W-FW2 systemManage: DELETE /systemManage/batchDeleteMenu payload
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteMenuInput {
    pub ids: Vec<i32>,
}
