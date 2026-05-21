//! F7 manage-crud-alignment: 5 個 systemManage alias wrapper handler。
//! per F7 spec FR-007 + FR-008 + data-model E6 — call F9 既有 service method、
//! 用 .into() map raw model → F7 Output DTO、wrap Res::new_data envelope。
//! W-FW1: add/update user transform handlers (base-web shape → backend domain shape).
//! W-FW2: add/update/delete/batchDelete menu transform handlers (base-web shape → backend domain shape).

use std::sync::Arc;

use axum::{extract::Query, Extension, Json};
use serde_json::{json, Value};
use server_core::web::{audit::Actor, auth::User, code, error::AppError, page::PaginatedData, res::Res};
use server_service::admin::{
    BatchDeleteMenuInput, CreateUserInput, DeleteMenuByBodyInput, Gender,
    MenuInput, MenuType, RolePageRequest, Status, SysMenuModel, SysMenuService, SysRoleService,
    SysUserService, SystemManageAddMenuInput, SystemManageAddUserInput, SystemManageAllRoleOutput,
    SystemManageMenuOutput, SystemManageMenuTreeNodeOutput, SystemManageRoleOutput,
    SystemManageUpdateMenuInput, SystemManageUpdateUserInput, SystemManageUserOutput, TMenuService,
    TRoleService, TUserService, UpdateMenuInput, UpdateUserInput, UserPageRequest,
    UserWithoutPassword,
};

pub struct SysSystemManageApi;

impl SysSystemManageApi {
    /// F7 alias: GET /systemManage/getRoleList
    pub async fn list_roles_for_systemmanage(
        Query(params): Query<RolePageRequest>,
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<PaginatedData<SystemManageRoleOutput>>, AppError> {
        let raw = service.find_paginated_roles(params).await?;
        Ok(Res::new_data(PaginatedData {
            current: raw.current,
            size: raw.size,
            total: raw.total,
            records: raw.records.into_iter().map(Into::into).collect(),
        }))
    }

    /// F7 alias: GET /systemManage/getAllRoles
    pub async fn list_all_roles_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<Vec<SystemManageAllRoleOutput>>, AppError> {
        let raw = service.find_all_enabled().await?;
        Ok(Res::new_data(raw.into_iter().map(Into::into).collect()))
    }

    /// F7 alias: GET /systemManage/getUserList
    pub async fn list_users_for_systemmanage(
        Query(params): Query<UserPageRequest>,
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<PaginatedData<SystemManageUserOutput>>, AppError> {
        let raw = service.find_paginated_users(params).await?;
        Ok(Res::new_data(PaginatedData {
            current: raw.current,
            size: raw.size,
            total: raw.total,
            records: raw.records.into_iter().map(Into::into).collect(),
        }))
    }

    /// F7 alias: GET /systemManage/getMenuList/v2
    pub async fn list_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<PaginatedData<SystemManageMenuOutput>>, AppError> {
        // F7 follow-up: base-web `Api.SystemManage.MenuList = PaginatingQueryRecord<Menu>`
        // 預期 paginated 包裝。F7 原 wrapper 回扁平 array、base view 顯示「无数据」。
        // 改為 paginated envelope:size=total=records.len()(rust 端不分頁、一次回全)。
        let raw = service.get_menu_list().await?;
        let records: Vec<SystemManageMenuOutput> = raw.into_iter().map(Into::into).collect();
        let total = records.len() as u64;
        Ok(Res::new_data(PaginatedData {
            current: 1,
            size: total,
            total,
            records,
        }))
    }

    /// F7 alias: GET /systemManage/getMenuTree
    pub async fn tree_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<SystemManageMenuTreeNodeOutput>>, AppError> {
        let raw = service.tree_menu().await?;
        Ok(Res::new_data(raw.into_iter().map(Into::into).collect()))
    }

    /// W-FW1 transform: POST /systemManage/addUser (base-web shape → backend domain shape)
    pub async fn add_user_for_systemmanage(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        let create_input = CreateUserInput {
            domain: "built-in".to_string(),
            username: input.user_name,
            // 預設密碼 per research R-Q5；完整密碼 UX 屬 W-FW1-N2 follow-up
            password: "123456".to_string(),
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        service.create_user(create_input, &actor).await.map(Res::new_data)
    }

    /// W-FW1 transform: POST /systemManage/updateUser (base-web shape → backend domain shape)
    pub async fn update_user_for_systemmanage(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        let update_input = UpdateUserInput {
            id: input.id,
            domain: "built-in".to_string(),
            username: input.user_name,
            password: None,
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        service.update_user(update_input, &actor).await.map(Res::new_data)
    }

    /// W-FW2 transform: POST /systemManage/addMenu (base-web shape → backend domain shape)
    pub async fn add_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        let create_input = MenuInput {
            menu_type: map_menu_type(&input.menu_type)?,
            menu_name: input.menu_name,
            icon_type: map_icon_type(input.icon_type.as_deref())?,
            icon: input.icon,
            route_name: input.route_name,
            route_path: input.route_path,
            component: input.component,
            path_param: None, // MenuInput 既有欄位，base-web 表單無對應，刻意填 None
            status: map_status(&input.status)?,
            active_menu: input.active_menu,
            hide_in_menu: input.hide_in_menu,
            pid: input.parent_id.to_string(),
            sequence: input.order,
            i18n_key: input.i18n_key,
            keep_alive: input.keep_alive,
            constant: input.constant,
            href: input.href,
            multi_tab: input.multi_tab,
        };
        service.create_menu(create_input, &actor).await.map(Res::new_data)
    }

    /// W-FW2 transform: POST /systemManage/updateMenu (base-web shape → backend domain shape)
    pub async fn update_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        let update_input = UpdateMenuInput {
            id: input.id,
            menu: MenuInput {
                menu_type: map_menu_type(&input.menu_type)?,
                menu_name: input.menu_name,
                icon_type: map_icon_type(input.icon_type.as_deref())?,
                icon: input.icon,
                route_name: input.route_name,
                route_path: input.route_path,
                component: input.component,
                path_param: None, // MenuInput 既有欄位，base-web 表單無對應，刻意填 None
                status: map_status(&input.status)?,
                active_menu: input.active_menu,
                hide_in_menu: input.hide_in_menu,
                pid: input.parent_id.to_string(),
                sequence: input.order,
                i18n_key: input.i18n_key,
                keep_alive: input.keep_alive,
                constant: input.constant,
                href: input.href,
                multi_tab: input.multi_tab,
            },
        };
        service.update_menu(update_input, &actor).await.map(Res::new_data)
    }

    /// W-FW2 transform: DELETE /systemManage/deleteMenu (body id)
    pub async fn delete_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteMenuByBodyInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        service.delete_menu(input.id, &actor).await.map(|_| Res::new_data(true))
    }

    /// W-FW2 transform: DELETE /systemManage/batchDeleteMenu
    /// per-row Err 不快、continue loop（比照 W-FW1/F9 batch_delete_users 體例）
    pub async fn batch_delete_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteMenuInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for id in input.ids {
            match service.delete_menu(id, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }
}

fn map_status(s: &str) -> Result<Status, AppError> {
    match s {
        "1" => Ok(Status::Enabled),
        "2" => Ok(Status::Disabled),
        _ => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid status value: {}", s),
        }),
    }
}

fn map_gender(s: Option<&str>) -> Result<Option<Gender>, AppError> {
    match s {
        None => Ok(None),
        Some("1") => Ok(Some(Gender::Male)),
        Some("2") => Ok(Some(Gender::Female)),
        Some(other) => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid gender value: {}", other),
        }),
    }
}

fn map_menu_type(s: &str) -> Result<MenuType, AppError> {
    match s {
        "1" => Ok(MenuType::Directory),
        "2" => Ok(MenuType::Menu),
        _ => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid menu_type value: {}", s),
        }),
    }
}

fn map_icon_type(s: Option<&str>) -> Result<Option<i32>, AppError> {
    match s {
        None => Ok(None),
        Some("1") => Ok(Some(1)),
        Some("2") => Ok(Some(2)),
        Some(other) => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid icon_type value: {}", other),
        }),
    }
}
