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
    facade::sys_role::{
        self, ActiveModel as SysRoleActiveModel, Column as SysRoleColumn, Model as SysRoleModel,
    },
    input::{CreateRoleInput, RolePageRequest, UpdateRoleInput},
};

use super::sys_role_error::RoleError;
use crate::helper::db_helper;
use ulid::Ulid;

#[async_trait]
pub trait TRoleService {
    async fn find_paginated_roles(
        &self,
        params: RolePageRequest,
    ) -> Result<PaginatedData<SysRoleModel>, AppError>;

    async fn create_role(
        &self,
        input: CreateRoleInput,
        actor: &Actor,
    ) -> Result<SysRoleModel, AppError>;
    async fn get_role(&self, id: &str) -> Result<SysRoleModel, AppError>;
    async fn update_role(
        &self,
        input: UpdateRoleInput,
        actor: &Actor,
    ) -> Result<SysRoleModel, AppError>;
    async fn delete_role(&self, id: &str, actor: &Actor) -> Result<(), AppError>;

    // F9 systemManage-alias-router: 取所有 enabled + active 角色（無分頁），供 /systemManage/getAllRoles 使用
    async fn find_all_enabled(&self) -> Result<Vec<SysRoleModel>, AppError>;
}

#[derive(Clone)]
pub struct SysRoleService;

impl SysRoleService {
    async fn check_role_exists_in_txn<C: ConnectionTrait>(
        &self,
        txn: &C,
        id: Option<&str>,
        code: &str,
    ) -> Result<(), AppError> {
        let mut query = sys_role::find_active().filter(SysRoleColumn::Code.eq(code));

        if let Some(id) = id {
            query = query.filter(SysRoleColumn::Id.ne(id));
        }

        let existing_role = query.one(txn).await.map_err(AppError::from)?;

        if existing_role.is_some() {
            return Err(RoleError::DuplicateRoleCode.into());
        }

        Ok(())
    }
}

#[async_trait]
impl TRoleService for SysRoleService {
    async fn find_paginated_roles(
        &self,
        params: RolePageRequest,
    ) -> Result<PaginatedData<SysRoleModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_role::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any().add(SysRoleColumn::Name.contains(keywords));
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

    async fn create_role(
        &self,
        input: CreateRoleInput,
        actor: &Actor,
    ) -> Result<SysRoleModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        self.check_role_exists_in_txn(&txn, None, &input.code).await?;

        let role = SysRoleActiveModel {
            id: Set(Ulid::new().to_string()),
            pid: Set(input.pid),
            code: Set(input.code),
            name: Set(input.name),
            status: Set(input.status),
            description: Set(input.description),
            created_at: Set(Local::now().naive_local()),
            created_by: Set("TODO".to_string()),
            ..Default::default()
        };

        let result = role.insert(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Insert,
                entity_type: "sys_role",
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

    async fn get_role(&self, id: &str) -> Result<SysRoleModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| RoleError::RoleNotFound.into())
    }

    async fn update_role(
        &self,
        input: UpdateRoleInput,
        actor: &Actor,
    ) -> Result<SysRoleModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        self.check_role_exists_in_txn(&txn, Some(&input.id), &input.role.code)
            .await?;

        let before = sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(&input.id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(RoleError::RoleNotFound))?;

        let role = SysRoleActiveModel {
            id: Set(input.id.clone()),
            pid: Set(input.role.pid),
            code: Set(input.role.code),
            name: Set(input.role.name),
            status: Set(input.role.status),
            description: Set(input.role.description),

            updated_at: Set(Some(Local::now().naive_local())),
            ..before.clone().into()
        };

        let updated_role = role.update(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_role",
                entity_id: updated_role.id.clone(),
                payload_before: Some(audit_snapshot(&before)),
                payload_after: Some(audit_snapshot(&updated_role)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(updated_role)
    }

    async fn delete_role(&self, id: &str, actor: &Actor) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_role::soft_delete_by_id(db.as_ref(), id.to_string(), actor).await
    }

    // F9 systemManage-alias-router: 取所有 enabled + active 角色（沿用 sys_role::find_active() facade、
    // status filter 對齊 sys_menu_service 既有 Status::Enabled 慣用 pattern）
    async fn find_all_enabled(&self) -> Result<Vec<SysRoleModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_role::find_active()
            .filter(SysRoleColumn::Status.eq(Status::Enabled))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)
    }
}
