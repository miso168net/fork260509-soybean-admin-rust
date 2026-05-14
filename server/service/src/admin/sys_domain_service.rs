use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, PaginatorTrait, QueryFilter, Set,
    TransactionTrait,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::sea_orm_active_enums::Status,
    facade::sys_domain::{
        self, ActiveModel as SysDomainActiveModel, Column as SysDomainColumn,
        Model as SysDomainModel,
    },
    input::{CreateDomainInput, DomainPageRequest, UpdateDomainInput},
};
use ulid::Ulid;

use crate::{admin::sys_domain_error::DomainError, helper::db_helper};

#[async_trait]
pub trait TDomainService {
    async fn find_paginated_domains(
        &self,
        params: DomainPageRequest,
    ) -> Result<PaginatedData<SysDomainModel>, AppError>;

    async fn create_domain(
        &self,
        input: CreateDomainInput,
        actor: &Actor,
    ) -> Result<SysDomainModel, AppError>;
    async fn get_domain(&self, id: &str) -> Result<SysDomainModel, AppError>;
    async fn update_domain(
        &self,
        input: UpdateDomainInput,
        actor: &Actor,
    ) -> Result<SysDomainModel, AppError>;
    async fn delete_domain(&self, id: &str, actor: &Actor) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct SysDomainService;

impl SysDomainService {
    async fn check_domain_exists_in_txn<C: ConnectionTrait>(
        &self,
        txn: &C,
        id: Option<&str>,
        code: &str,
        name: &str,
    ) -> Result<(), AppError> {
        let id_str = id.unwrap_or("-1");

        let code_exists = sys_domain::find_active()
            .filter(SysDomainColumn::Code.eq(code))
            .filter(SysDomainColumn::Id.ne(id_str))
            .one(txn)
            .await
            .map_err(AppError::from)?
            .is_some();

        if code_exists {
            return Err(DomainError::DuplicateCode.into());
        }

        let name_exists = sys_domain::find_active()
            .filter(SysDomainColumn::Name.eq(name))
            .filter(SysDomainColumn::Id.ne(id_str))
            .one(txn)
            .await
            .map_err(AppError::from)?
            .is_some();

        if name_exists {
            return Err(DomainError::DuplicateName.into());
        }

        Ok(())
    }
}

#[async_trait]
impl TDomainService for SysDomainService {
    async fn find_paginated_domains(
        &self,
        params: DomainPageRequest,
    ) -> Result<PaginatedData<SysDomainModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_domain::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any().add(SysDomainColumn::Name.contains(keywords));
            query = query.filter(condition);
        }

        let total = query
            .clone()
            .count(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let paginator = query.paginate(db.as_ref(), params.page_details.size);
        let records = paginator
            .fetch_page(params.page_details.current - 1)
            .await
            .map_err(AppError::from)?;

        Ok(PaginatedData {
            current: params.page_details.current,
            size: params.page_details.size,
            total,
            records,
        })
    }

    async fn create_domain(
        &self,
        input: CreateDomainInput,
        actor: &Actor,
    ) -> Result<SysDomainModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        self.check_domain_exists_in_txn(&txn, None, &input.code, &input.name)
            .await?;

        let domain = SysDomainActiveModel {
            id: Set(Ulid::new().to_string()),
            code: Set(input.code),
            name: Set(input.name),
            description: Set(input.description),
            status: Set(Status::Enabled),
            created_at: Set(Local::now().naive_local()),
            created_by: Set("TODO".to_string()),
            ..Default::default()
        };

        let result = domain.insert(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Insert,
                entity_type: "sys_domain",
                entity_id: result.id.clone(),
                payload_before: None,
                payload_after: Some(audit_snapshot(&result)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(result)
    }

    async fn get_domain(&self, id: &str) -> Result<SysDomainModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_domain::find_active()
            .filter(SysDomainColumn::Id.eq(id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| DomainError::DomainNotFound.into())
    }

    async fn update_domain(
        &self,
        input: UpdateDomainInput,
        actor: &Actor,
    ) -> Result<SysDomainModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        let before = sys_domain::find_active()
            .filter(SysDomainColumn::Id.eq(&input.id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(DomainError::DomainNotFound))?;

        if before.code == "built-in" {
            return Err(DomainError::BuiltInDomain.into());
        }

        self.check_domain_exists_in_txn(&txn, Some(&input.id), &input.domain.code, &input.domain.name)
            .await?;

        let mut domain: SysDomainActiveModel = before.clone().into();
        domain.code = Set(input.domain.code);
        domain.name = Set(input.domain.name);
        domain.description = Set(input.domain.description);

        let updated_domain = domain.update(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_domain",
                entity_id: updated_domain.id.clone(),
                payload_before: Some(audit_snapshot(&before)),
                payload_after: Some(audit_snapshot(&updated_domain)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(updated_domain)
    }

    async fn delete_domain(&self, id: &str, actor: &Actor) -> Result<(), AppError> {
        let domain = self.get_domain(id).await?;

        if domain.code == "built-in" {
            return Err(DomainError::BuiltInDomain.into());
        }

        let db = db_helper::get_db_connection().await?;
        sys_domain::soft_delete_by_id(db.as_ref(), id.to_string(), actor).await
    }
}
