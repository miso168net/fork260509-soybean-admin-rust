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
use server_service::admin::{
    BatchDeleteUserInput, CreateUserInput, DeleteUserByBodyInput, SysUserService, TUserService,
    UpdateUserInput, UserPageRequest, UserWithoutPassword,
};

pub struct SysUserApi;

impl SysUserApi {
    pub async fn get_all_users(
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<Vec<UserWithoutPassword>>, AppError> {
        service.find_all().await.map(Res::new_data)
    }

    pub async fn get_paginated_users(
        Query(params): Query<UserPageRequest>,
        Extension(service): Extension<Arc<SysUserService>>,
        user: User,
    ) -> Result<Res<PaginatedData<UserWithoutPassword>>, AppError> {
        print!("user is {:#?}", user);
        service
            .find_paginated_users(params)
            .await
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
        Res::new_data(true)
    }

    pub async fn create_user(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        service.create_user(input, &actor).await.map(Res::new_data)
    }

    pub async fn get_user(
        Path(id): Path<String>,
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        service.get_user(&id).await.map(Res::new_data)
    }

    pub async fn update_user(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<UpdateUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        service.update_user(input, &actor).await.map(Res::new_data)
    }

    pub async fn delete_user(
        Path(id): Path<String>,
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service.delete_user(&id, &actor).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: DELETE /systemManage/deleteUser (body-id payload variant)
    // 重用既有 delete_user service method；handler 差異只在 extractor (Json body 取代 Path)
    pub async fn delete_user_by_body(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteUserByBodyInput>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service.delete_user(&input.id, &actor).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: DELETE /systemManage/batchDeleteUser
    // per-row Err 不快、continue loop（per F9 spec brainstorm Q2 + R-2）
    pub async fn batch_delete_users(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteUserInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for id in &input.ids {
            match service.delete_user(id, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }
}
