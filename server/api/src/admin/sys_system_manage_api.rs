//! F7 manage-crud-alignment: 5 個 systemManage alias wrapper handler。
//! per F7 spec FR-007 + FR-008 + data-model E6 — call F9 既有 service method、
//! 用 .into() map raw model → F7 Output DTO、wrap Res::new_data envelope。

use std::sync::Arc;

use axum::{extract::Query, Extension};
use server_core::web::{error::AppError, page::PaginatedData, res::Res};
use server_service::admin::{
    RolePageRequest, SysMenuService, SysRoleService, SysUserService, SystemManageAllRoleOutput,
    SystemManageMenuOutput, SystemManageMenuTreeNodeOutput, SystemManageRoleOutput,
    SystemManageUserOutput, TMenuService, TRoleService, TUserService, UserPageRequest,
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
}
