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
    AccessKeyDetail, AccessKeyPageRequest, CreateAccessKeyInput, SysAccessKeyService,
    TAccessKeyService,
};

pub struct SysAccessKeyApi;

impl SysAccessKeyApi {
    /// 040 T012 W-FW9: return type `Res<PaginatedData<SysAccessKeyModel>>` → `Res<PaginatedData<AccessKeyDetail>>`；
    /// records 逐筆 `AccessKeyDetail::from`、page meta 保留。
    pub async fn get_paginated_access_keys(
        Query(params): Query<AccessKeyPageRequest>,
        Extension(service): Extension<Arc<SysAccessKeyService>>,
    ) -> Result<Res<PaginatedData<AccessKeyDetail>>, AppError> {
        service
            .find_paginated_access_keys(params)
            .await
            .map(|page| PaginatedData {
                current: page.current,
                size: page.size,
                total: page.total,
                records: page.records.into_iter().map(AccessKeyDetail::from).collect(),
            })
            .map(Res::new_data)
    }

    /// 040 T012 W-FW9: return type `Res<SysAccessKeyModel>` → `Res<AccessKeyDetail>`。
    pub async fn create_access_key(
        Extension(service): Extension<Arc<SysAccessKeyService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateAccessKeyInput>,
    ) -> Result<Res<AccessKeyDetail>, AppError> {
        let actor = Actor::from(&user);
        service
            .create_access_key(input, &actor)
            .await
            .map(AccessKeyDetail::from)
            .map(Res::new_data)
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
