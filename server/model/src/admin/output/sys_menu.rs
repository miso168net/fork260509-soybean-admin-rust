use chrono::NaiveDateTime;
use serde::Serialize;

use crate::admin::entities::sea_orm_active_enums::{MenuType, Status};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MenuRoute {
    pub name: String,
    pub path: String,
    pub component: String,
    pub meta: RouteMeta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<MenuRoute>>,
    pub id: i32,
    pub pid: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RouteMeta {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i18n_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<bool>,
    pub constant: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_in_menu: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_menu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_tab: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MenuTree {
    pub id: i32,
    pub pid: String,
    pub menu_type: MenuType,
    pub menu_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_type: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub route_name: String,
    pub route_path: String,
    pub component: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_param: Option<String>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_menu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_in_menu: Option<bool>,
    pub sequence: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i18n_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<bool>,
    pub constant: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_tab: Option<bool>,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<MenuTree>>,
}
