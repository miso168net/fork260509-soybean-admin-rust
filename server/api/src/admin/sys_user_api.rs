use std::sync::Arc;

use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use axum_casbin::{casbin::MgmtApi, CasbinAxumLayer};
use serde_json::{json, Value};
use server_core::web::{
    audit::Actor, auth::User, error::AppError, page::PaginatedData, res::Res,
    validator::ValidatedForm,
};
use server_global::notify_casbin_changed;
use server_service::admin::{
    BatchDeleteUserInput, CreateUserInput, DeleteUserByBodyInput, SysUserService, TUserService,
    UpdateUserInput, UserDetail, UserPageRequest,
};

pub struct SysUserApi;

impl SysUserApi {
    /// 040 T011 W-FW9: return type `Res<Vec<UserWithoutPassword>>` → `Res<Vec<UserDetail>>`。
    pub async fn get_all_users(
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<Vec<UserDetail>>, AppError> {
        service
            .find_all()
            .await
            .map(|v| v.into_iter().map(UserDetail::from).collect::<Vec<_>>())
            .map(Res::new_data)
    }

    /// 040 T011 W-FW9: return type `Res<PaginatedData<UserWithoutPassword>>` → `Res<PaginatedData<UserDetail>>`；
    /// records 逐筆 `UserDetail::from`、page meta 保留。
    pub async fn get_paginated_users(
        Query(params): Query<UserPageRequest>,
        Extension(service): Extension<Arc<SysUserService>>,
        user: User,
    ) -> Result<Res<PaginatedData<UserDetail>>, AppError> {
        tracing::debug!(?user, "get_paginated_users: caller user info");
        service
            .find_paginated_users(params)
            .await
            .map(|page| PaginatedData {
                current: page.current,
                size: page.size,
                total: page.total,
                records: page.records.into_iter().map(UserDetail::from).collect(),
            })
            .map(Res::new_data)
    }

    pub async fn remove_policies(
        Extension(mut cache_enforcer): Extension<CasbinAxumLayer>,
    ) -> Res<bool> {
        let enforcer = cache_enforcer.get_enforcer();
        let mut enforcer_write = enforcer.write().await;
        let rule = vec![
            "1".to_string(),
            "built-in".to_string(),
            "/user/users".to_string(),
            "GET".to_string(),
        ];
        let _ = enforcer_write.remove_policies(vec![rule]).await;
        notify_casbin_changed().await;
        Res::new_data(true)
    }

    pub async fn add_policies(
        Extension(mut cache_enforcer): Extension<CasbinAxumLayer>,
    ) -> Res<bool> {
        let enforcer = cache_enforcer.get_enforcer();
        let mut enforcer_write = enforcer.write().await;
        let rule = vec![
            "1".to_string(),
            "built-in".to_string(),
            "/user/users".to_string(),
            "GET".to_string(),
        ];
        let _ = enforcer_write.add_policy(rule).await;
        notify_casbin_changed().await;
        Res::new_data(true)
    }

    /// 040 T011 W-FW9: return type `Res<UserWithoutPassword>` → `Res<UserDetail>`。
    pub async fn create_user(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateUserInput>,
    ) -> Result<Res<UserDetail>, AppError> {
        let actor = Actor::from(&user);
        service
            .create_user(input, &actor)
            .await
            .map(UserDetail::from)
            .map(Res::new_data)
    }

    /// 039 T029: Path<String> → Path<i64> + user_svc.lookup_ulid_by_display_id cascade。
    /// 040 T011 W-FW9: return type `Res<UserWithoutPassword>` → `Res<UserDetail>`。
    pub async fn get_user(
        Path(display_id): Path<i64>,
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<UserDetail>, AppError> {
        let user_ulid = service.lookup_ulid_by_display_id(display_id).await?;
        service
            .get_user(&user_ulid)
            .await
            .map(UserDetail::from)
            .map(Res::new_data)
    }

    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 service。
    /// 040 T011 W-FW9: return type `Res<UserWithoutPassword>` → `Res<UserDetail>`。
    pub async fn update_user(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<UpdateUserInput>,
    ) -> Result<Res<UserDetail>, AppError> {
        let actor = Actor::from(&user);
        let user_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        service
            .update_user(&user_ulid, input, &actor)
            .await
            .map(UserDetail::from)
            .map(Res::new_data)
    }

    /// 039 T029: Path<String> → Path<i64> + user_svc.lookup_ulid_by_display_id cascade。
    pub async fn delete_user(
        Path(display_id): Path<i64>,
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        let user_ulid = service.lookup_ulid_by_display_id(display_id).await?;
        service.delete_user(&user_ulid, &actor).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: DELETE /systemManage/deleteUser (body-id payload variant)
    // 重用既有 delete_user service method；handler 差異只在 extractor (Json body 取代 Path)
    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 service。
    pub async fn delete_user_by_body(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteUserByBodyInput>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        let user_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        service.delete_user(&user_ulid, &actor).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: DELETE /systemManage/batchDeleteUser
    // per-row Err 不快、continue loop（per F9 spec brainstorm Q2 + R-2）
    /// 039 T030.5: input.ids 改 Vec<i64>；handler 逐筆 lookup_ulid_by_display_id 後餵 service；
    /// 單筆 lookup 失敗（display_id 無對應 user）視為一般 per-row Err、continue loop（不中斷批次）。
    pub async fn batch_delete_users(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteUserInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for display_id in &input.ids {
            let ulid = match service.lookup_ulid_by_display_id(*display_id).await {
                Ok(u) => u,
                Err(_) => continue,
            };
            match service.delete_user(&ulid, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }
}
