use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    Extension,
};
use server_core::web::{
    audit::Actor, auth::User, error::AppError, page::PaginatedData, res::Res,
    validator::ValidatedForm,
};
use server_service::admin::{
    CreateRoleInput, RoleDetail, RolePageRequest, SysRoleService, TRoleService, UpdateRoleInput,
};

pub struct SysRoleApi;

impl SysRoleApi {
    /// 040 T010 W-FW9: return type `Res<PaginatedData<SysRoleModel>>` → `Res<PaginatedData<RoleDetail>>`；
    /// records 逐筆 `RoleDetail::from`，page meta (current/size/total) 保留。
    pub async fn get_paginated_roles(
        Query(params): Query<RolePageRequest>,
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<PaginatedData<RoleDetail>>, AppError> {
        service
            .find_paginated_roles(params)
            .await
            .map(|page| PaginatedData {
                current: page.current,
                size: page.size,
                total: page.total,
                records: page.records.into_iter().map(RoleDetail::from).collect(),
            })
            .map(Res::new_data)
    }

    /// 040 T010 W-FW9: return type `Res<SysRoleModel>` → `Res<RoleDetail>`。
    pub async fn create_role(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateRoleInput>,
    ) -> Result<Res<RoleDetail>, AppError> {
        let actor = Actor::from(&user);
        service
            .create_role(input, &actor)
            .await
            .map(RoleDetail::from)
            .map(Res::new_data)
    }

    /// 039 T028: Path<String> → Path<i64> + role_svc.lookup_ulid_by_display_id cascade。
    /// 040 T010 W-FW9: return type `Res<SysRoleModel>` → `Res<RoleDetail>`。
    pub async fn get_role(
        Path(display_id): Path<i64>,
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<RoleDetail>, AppError> {
        let role_ulid = service.lookup_ulid_by_display_id(display_id).await?;
        service
            .get_role(&role_ulid)
            .await
            .map(RoleDetail::from)
            .map(Res::new_data)
    }

    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 service。
    /// 040 T010 W-FW9: return type `Res<SysRoleModel>` → `Res<RoleDetail>`。
    pub async fn update_role(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<UpdateRoleInput>,
    ) -> Result<Res<RoleDetail>, AppError> {
        let actor = Actor::from(&user);
        let role_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        service
            .update_role(&role_ulid, input, &actor)
            .await
            .map(RoleDetail::from)
            .map(Res::new_data)
    }

    /// 039 T028: Path<String> → Path<i64> + role_svc.lookup_ulid_by_display_id cascade。
    pub async fn delete_role(
        Path(display_id): Path<i64>,
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        let role_ulid = service.lookup_ulid_by_display_id(display_id).await?;
        service.delete_role(&role_ulid, &actor).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: GET /systemManage/getAllRoles
    // 取所有 enabled + active 角色（無分頁），base-web role-select 下拉用
    /// 040 T010 W-FW9: return type `Res<Vec<SysRoleModel>>` → `Res<Vec<RoleDetail>>`。
    pub async fn get_all_roles(
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<Vec<RoleDetail>>, AppError> {
        service
            .find_all_enabled()
            .await
            .map(|v| v.into_iter().map(RoleDetail::from).collect::<Vec<_>>())
            .map(Res::new_data)
    }
}
