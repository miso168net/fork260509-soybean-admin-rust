//! F7 manage-crud-alignment: 5 個 systemManage alias wrapper handler。
//! per F7 spec FR-007 + FR-008 + data-model E6 — call F9 既有 service method、
//! 用 .into() map raw model → F7 Output DTO、wrap Res::new_data envelope。
//! W-FW1: add/update user transform handlers (base-web shape → backend domain shape).
//! W-FW2: add/update/delete/batchDelete menu transform handlers (base-web shape → backend domain shape).
//! W-FW4: getRoleMenuIds/assignRoleMenus 角色菜單授權 alias handlers (domain 由 JWT actor 注入).

use std::sync::Arc;

use axum::{extract::{Path, Query}, Extension, Json};
use serde_json::{json, Value};
use server_core::web::{audit::Actor, auth::User, code, error::AppError, page::PaginatedData, res::Res};
use server_service::admin::{
    AssignRoleMenusInput, BatchDeleteMenuInput, BatchDeleteRoleInput, CreateRoleInput,
    CreateUserInput, DeleteMenuByBodyInput, DeleteRoleByBodyInput, Gender, MenuInput, MenuType,
    RoleInput, RolePageRequest, Status, SysAuthorizationService, SysMenuModel, SysMenuService,
    SysRoleModel, SysRoleService, SysUserService, SystemManageAddMenuInput,
    SystemManageAddRoleInput, SystemManageAddUserInput, SystemManageAllRoleOutput,
    SystemManageMenuOutput, SystemManageMenuTreeNodeOutput, SystemManageRoleOutput,
    SystemManageUpdateMenuInput, SystemManageUpdateRoleInput, SystemManageUpdateUserInput,
    SystemManageUserOutput, TAuthorizationService, TMenuService, TRoleService, TUserService,
    UpdateMenuInput, UpdateRoleInput, UpdateUserInput, UserPageRequest, UserWithoutPassword,
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
    /// W-FW5 T002: user_roles 由批次查詢真實填充（取代 From impl 硬寫 vec![]）。
    pub async fn list_users_for_systemmanage(
        Query(params): Query<UserPageRequest>,
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<PaginatedData<SystemManageUserOutput>>, AppError> {
        let raw = service.find_paginated_users(params).await?;
        let mut records: Vec<SystemManageUserOutput> =
            raw.records.into_iter().map(Into::into).collect();

        // W-FW5 T002: 批次查本頁所有 user 的 role code 集合（一次 query、避免 N+1）
        let user_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
        let roles_map = service.get_role_codes_for_users(user_ids).await?;
        for record in &mut records {
            record.user_roles = roles_map.get(&record.id).cloned().unwrap_or_default();
        }

        Ok(Res::new_data(PaginatedData {
            current: raw.current,
            size: raw.size,
            total: raw.total,
            records,
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
    /// W-FW5: 收 userRoles（角色指派）+ 選填 password。
    pub async fn add_user_for_systemmanage(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        // W-FW5 T010: None / Some("") 皆視為未提供 → 沿用預設密碼 123456
        let password = normalize_password(input.password)
            .unwrap_or_else(|| "123456".to_string());
        let create_input = CreateUserInput {
            domain: "built-in".to_string(),
            username: input.user_name,
            password,
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        let created = service.create_user(create_input, &actor).await?;
        // W-FW5 T005: user→roles 指派（空清單為合法清空）
        service
            .assign_roles_to_user(created.id.clone(), input.user_roles, &actor)
            .await?;
        Ok(Res::new_data(created))
    }

    /// W-FW1 transform: POST /systemManage/updateUser (base-web shape → backend domain shape)
    /// W-FW5: 收 userRoles（角色指派）+ 選填 password。
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
            // W-FW5 T010: None / Some("") 皆視為未提供 → 不動 password
            password: normalize_password(input.password),
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        let updated = service.update_user(update_input, &actor).await?;
        // W-FW5 T005: user→roles 指派（空清單為合法清空）
        service
            .assign_roles_to_user(updated.id.clone(), input.user_roles, &actor)
            .await?;
        Ok(Res::new_data(updated))
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

    /// W-FW3 transform: POST /systemManage/addRole (base-web shape → backend domain shape)
    pub async fn add_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddRoleInput>,
    ) -> Result<Res<SysRoleModel>, AppError> {
        let actor = Actor::from(&user);
        let create_input = CreateRoleInput {
            // R-Q1 root convention：base-web 角色頁是扁平表格，新角色一律掛 root
            pid: "0".to_string(),
            code: input.role_code,
            name: input.role_name,
            status: map_status(&input.status)?,
            description: input.role_desc,
        };
        service.create_role(create_input, &actor).await.map(Res::new_data)
    }

    /// W-FW3 transform: POST /systemManage/updateRole (base-web shape → backend domain shape)
    /// FR-007 code-lock：code / pid 沿用既有值（避免 Casbin policy 失聯、避免擾動角色樹）。
    pub async fn update_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateRoleInput>,
    ) -> Result<Res<SysRoleModel>, AppError> {
        let actor = Actor::from(&user);
        let existing = service.get_role(&input.id).await?;
        let update_input = UpdateRoleInput {
            id: input.id,
            role: RoleInput {
                pid: existing.pid,
                code: existing.code,
                name: input.role_name,
                status: map_status(&input.status)?,
                description: input.role_desc,
            },
        };
        service.update_role(update_input, &actor).await.map(Res::new_data)
    }

    /// W-FW3 transform: DELETE /systemManage/deleteRole (body id)
    pub async fn delete_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteRoleByBodyInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        service.delete_role(&input.id, &actor).await.map(|_| Res::new_data(true))
    }

    /// W-FW3 transform: DELETE /systemManage/batchDeleteRole
    /// per-row Err 不快、continue loop（比照 W-FW2 batch_delete_menu 體例）
    pub async fn batch_delete_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteRoleInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for id in input.ids {
            match service.delete_role(&id, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }

    /// W-FW4 transform: GET /systemManage/getRoleMenuIds/:roleId
    /// domain 由 JWT actor 伺服器端注入，base-web 不傳。
    pub async fn get_role_menu_ids_for_systemmanage(
        Path(role_id): Path<String>,
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<Vec<i32>>, AppError> {
        service
            .get_menu_ids_by_role_id(role_id, user.domain())
            .await
            .map(Res::new_data)
    }

    /// W-FW4 transform: POST /systemManage/assignRoleMenus
    /// domain 由 JWT actor 伺服器端注入，base-web 不傳。
    pub async fn assign_role_menus_for_systemmanage(
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        Extension(user): Extension<User>,
        Json(input): Json<AssignRoleMenusInput>,
    ) -> Result<Res<bool>, AppError> {
        service
            .assign_routes(user.domain(), input.role_id, input.menu_ids)
            .await
            .map(|_| Res::new_data(true))
    }
}

/// W-FW5 T010: 密碼空值正規化 — `None` 與 `Some("")`（空字串）皆視為「未提供」。
fn normalize_password(password: Option<String>) -> Option<String> {
    password.filter(|p| !p.is_empty())
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
