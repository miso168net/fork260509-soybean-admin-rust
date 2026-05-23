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
    AccessKeyPageRequest, CreateAccessKeyInput, SysAccessKeyModel, SysAccessKeyService,
    TAccessKeyService,
};

pub struct SysAccessKeyApi;

impl SysAccessKeyApi {
    pub async fn get_paginated_access_keys(
        Query(params): Query<AccessKeyPageRequest>,
        Extension(service): Extension<Arc<SysAccessKeyService>>,
    ) -> Result<Res<PaginatedData<SysAccessKeyModel>>, AppError> {
        service
            .find_paginated_access_keys(params)
            .await
            .map(Res::new_data)
    }

    pub async fn create_access_key(
        Extension(service): Extension<Arc<SysAccessKeyService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateAccessKeyInput>,
    ) -> Result<Res<SysAccessKeyModel>, AppError> {
        let actor = Actor::from(&user);
        service.create_access_key(input, &actor).await.map(Res::new_data)
    }

    /// 039 T030: Path<String> → Path<i64> + access_key_svc.lookup_ulid_by_display_id cascade。
    pub async fn delete_access_key(
        Path(display_id): Path<i64>,
        Extension(service): Extension<Arc<SysAccessKeyService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        let key_ulid = service.lookup_ulid_by_display_id(display_id).await?;
        service.delete_access_key(&key_ulid, &actor).await.map(Res::new_data)
    }
}
