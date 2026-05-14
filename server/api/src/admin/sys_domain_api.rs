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
    CreateDomainInput, DomainPageRequest, SysDomainModel, SysDomainService, TDomainService,
    UpdateDomainInput,
};

pub struct SysDomainApi;

impl SysDomainApi {
    pub async fn get_paginated_domains(
        Query(params): Query<DomainPageRequest>,
        Extension(service): Extension<Arc<SysDomainService>>,
    ) -> Result<Res<PaginatedData<SysDomainModel>>, AppError> {
        service
            .find_paginated_domains(params)
            .await
            .map(Res::new_data)
    }

    pub async fn create_domain(
        Extension(service): Extension<Arc<SysDomainService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateDomainInput>,
    ) -> Result<Res<SysDomainModel>, AppError> {
        let actor = Actor::from(&user);
        service.create_domain(input, &actor).await.map(Res::new_data)
    }

    pub async fn get_domain(
        Path(id): Path<String>,
        Extension(service): Extension<Arc<SysDomainService>>,
    ) -> Result<Res<SysDomainModel>, AppError> {
        service.get_domain(&id).await.map(Res::new_data)
    }

    pub async fn update_domain(
        Extension(service): Extension<Arc<SysDomainService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<UpdateDomainInput>,
    ) -> Result<Res<SysDomainModel>, AppError> {
        let actor = Actor::from(&user);
        service.update_domain(input, &actor).await.map(Res::new_data)
    }

    pub async fn delete_domain(
        Path(id): Path<String>,
        Extension(service): Extension<Arc<SysDomainService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service.delete_domain(&id, &actor).await.map(Res::new_data)
    }
}
