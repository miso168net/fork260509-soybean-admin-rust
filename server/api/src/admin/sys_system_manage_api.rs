//! F7 manage-crud-alignment: 5 個 systemManage alias wrapper handler。
//! per F7 spec FR-007 + FR-008 + data-model E6 — call F9 既有 service method、
//! 用 .into() map raw model → F7 Output DTO、wrap Res::new_data envelope。
//! W-FW1: add/update user transform handlers (base-web shape → backend domain shape).

use std::sync::Arc;

use axum::{extract::Query, Extension, Json};
use server_core::web::{audit::Actor, auth::User, code, error::AppError, page::PaginatedData, res::Res};
use server_service::admin::{
    CreateUserInput, Gender, RolePageRequest, Status, SysMenuService, SysRoleService,
    SysUserService, SystemManageAddUserInput, SystemManageAllRoleOutput, SystemManageMenuOutput,
    SystemManageMenuTreeNodeOutput, SystemManageRoleOutput, SystemManageUpdateUserInput,
    SystemManageUserOutput, TMenuService, TRoleService, TUserService, UpdateUserInput,
    UserPageRequest, UserWithoutPassword,
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
