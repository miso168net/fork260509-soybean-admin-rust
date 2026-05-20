//! F7 manage-crud-alignment: 5 個 systemManage alias Output DTO + From impl。
//! per F7 spec FR-001~FR-006 + data-model E1/E2/E3/E4/E5 + R-Q2 (menu_type "1"/"2" mapping)
//! + R-Q6 (`pId` 大寫 I camelCase override)。
//! NOTE: service.find_paginated_users 回 `UserWithoutPassword`(非 sys_user::Model);
//!       service.get_menu_list / tree_menu 回 `MenuTree`(非 sys_menu::Model)。

use chrono::NaiveDateTime;
use serde::Serialize;
use tracing::warn;

use crate::admin::entities::{
    sea_orm_active_enums::{MenuType, Status},
    sys_role,
};
use crate::admin::output::{sys_menu::MenuTree, sys_user::UserWithoutPassword};

// ===== E1: SystemManageRoleOutput =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageRoleOutput {
    pub id: String,
    pub role_name: String,
    pub role_code: String,
    pub role_desc: String,
    pub status: Status,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl From<sys_role::Model> for SystemManageRoleOutput {
    fn from(m: sys_role::Model) -> Self {
        Self {
            id: m.id,
            role_name: m.name,
            role_code: m.code,
            role_desc: m.description.unwrap_or_default(),
            status: m.status,
            created_at: m.created_at,
            created_by: m.created_by,
            updated_at: m.updated_at,
            updated_by: m.updated_by,
        }
    }
}

// ===== E2: SystemManageAllRoleOutput =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAllRoleOutput {
    pub id: String,
    pub role_name: String,
    pub role_code: String,
}

impl From<sys_role::Model> for SystemManageAllRoleOutput {
    fn from(m: sys_role::Model) -> Self {
        Self {
            id: m.id,
            role_name: m.name,
            role_code: m.code,
        }
    }
}

// ===== E3: SystemManageUserOutput =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageUserOutput {
    pub id: String,
    pub user_name: String,
    pub user_gender: Option<String>,
    pub nick_name: String,
    pub user_phone: Option<String>,
    pub user_email: Option<String>,
    pub user_roles: Vec<String>,
    pub status: Status,
    pub created_at: NaiveDateTime,
    pub created_by: String,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

impl From<UserWithoutPassword> for SystemManageUserOutput {
    fn from(m: UserWithoutPassword) -> Self {
        Self {
            id: m.id,
            user_name: m.username,
            user_gender: None,
            nick_name: m.nick_name,
            user_phone: m.phone_number,
            user_email: m.email,
            user_roles: vec![],
            status: m.status,
            created_at: m.created_at,
            created_by: m.created_by,
            updated_at: m.updated_at,
            updated_by: m.updated_by,
        }
    }
}

// ===== E4: SystemManageMenuOutput (getMenuList/v2 平坦列表用) =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageMenuOutput {
    pub id: i32,
    pub parent_id: String,
    pub menu_type: String,
    pub menu_name: String,
    pub route_name: String,
    pub route_path: String,
    pub component: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub buttons: Option<Vec<serde_json::Value>>,
    pub children: Option<Vec<SystemManageMenuOutput>>,
    pub status: Status,
    pub hide_in_menu: Option<bool>,
    pub order: i32,
    pub i18n_key: Option<String>,
    pub keep_alive: Option<bool>,
    pub constant: bool,
    pub href: Option<String>,
    pub active_menu: Option<String>,
    pub multi_tab: Option<bool>,
    pub fixed_index_in_tab: Option<i32>,
    pub query: Option<serde_json::Value>,
}

fn map_menu_type(rust_type: MenuType) -> String {
    match rust_type {
        MenuType::Directory => "1".to_string(),
        MenuType::Menu => "2".to_string(),
    }
}

fn map_icon_type(rust_type: Option<i32>) -> Option<String> {
    match rust_type {
        None => None,
        Some(1) => Some("1".to_string()),
        Some(2) => Some("2".to_string()),
        Some(other) => {
            warn!(
                "SystemManageMenuOutput: unexpected icon_type {} from sys_menu, defaulting to \"1\"",
                other
            );
            Some("1".to_string())
        }
    }
}

impl From<MenuTree> for SystemManageMenuOutput {
    fn from(m: MenuTree) -> Self {
        Self {
            id: m.id,
            parent_id: m.pid,
            menu_type: map_menu_type(m.menu_type),
            menu_name: m.menu_name,
            route_name: m.route_name,
            route_path: m.route_path,
            component: m.component,
            icon: m.icon,
            icon_type: map_icon_type(m.icon_type),
            buttons: None,
            children: None,
            status: m.status,
            hide_in_menu: m.hide_in_menu,
            order: m.sequence,
            i18n_key: m.i18n_key,
            keep_alive: m.keep_alive,
            constant: m.constant,
            href: m.href,
            active_menu: m.active_menu,
            multi_tab: m.multi_tab,
            fixed_index_in_tab: None,
            query: None,
        }
    }
}

// ===== E5: SystemManageMenuTreeNodeOutput (getMenuTree zTree-style 用) =====
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageMenuTreeNodeOutput {
    pub id: i32,
    pub label: String,
    #[serde(rename = "pId")]
    pub p_id: String,
    pub children: Option<Vec<SystemManageMenuTreeNodeOutput>>,
}

impl From<MenuTree> for SystemManageMenuTreeNodeOutput {
    fn from(m: MenuTree) -> Self {
        Self {
            id: m.id,
            label: m.menu_name,
            p_id: m.pid,
            children: m
                .children
                .map(|children| children.into_iter().map(Into::into).collect()),
        }
    }
}
