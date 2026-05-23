use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbBackend, PaginatorTrait,
    QueryFilter, Set, Statement, TransactionTrait,
};
use server_global::{notify_casbin_changed, snowflake};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::sea_orm_active_enums::Status,
    facade::{
        sys_menu::{self, Column as SysMenuColumn},
        sys_role::{
            self, ActiveModel as SysRoleActiveModel, Column as SysRoleColumn,
            Model as SysRoleModel,
        },
    },
    input::{CreateRoleInput, RolePageRequest, UpdateRoleHomeInput, UpdateRoleInput},
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

    // W-FW6 role-authorization-completion (US1): 讀取角色首頁路由
    async fn get_role_home(&self, role_id: &str) -> Result<Option<String>, AppError>;

    // W-FW6 role-authorization-completion (US1): 更新角色首頁路由（None / Some("") 視為清除）
    async fn update_role_home(
        &self,
        input: UpdateRoleHomeInput,
        actor: &Actor,
    ) -> Result<(), AppError>;
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
            display_id: Set(snowflake::next_display_id()),
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

        // W-FW6 N4: detect code 變動 + 同步 Casbin policy (R-Q3: 只 ptype='p')
        if before.code != updated_role.code {
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE casbin_rule SET v0 = $1 WHERE ptype = 'p' AND v0 = $2",
                [
                    sea_orm::Value::String(Some(Box::new(updated_role.code.clone()))),
                    sea_orm::Value::String(Some(Box::new(before.code.clone()))),
                ],
            ))
            .await
            .map_err(AppError::from)?;
        }

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

        // W-FW6 N4: publish Casbin invalidate (R-Q2: commit 後 publish 避免訊號早於 commit)
        if before.code != updated_role.code {
            notify_casbin_changed().await;
        }

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

    // W-FW6 role-authorization-completion (US1): 讀取角色首頁路由
    async fn get_role_home(&self, role_id: &str) -> Result<Option<String>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let role = sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(role_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(RoleError::RoleNotFound))?;
        Ok(role.home_route_name)
    }

    // W-FW6 role-authorization-completion (US1): 更新角色首頁路由
    // home None / Some("") 皆視為「明示清除 home」, 持久化為 NULL;
    // 否則必須對應 sys_menu 既有 enabled + non-constant route_name (active 行) — 否則回 HomeRouteNotFound。
    async fn update_role_home(
        &self,
        input: UpdateRoleHomeInput,
        actor: &Actor,
    ) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        // fetch before snapshot（active row state；audit + RoleNotFound 共用）
        let before = sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(&input.role_id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(RoleError::RoleNotFound))?;

        // None / Some("") / Some(空白) 都正規化成 None — 持久化為 NULL（明示清除 home）
        let home_normalized: Option<String> = input
            .home
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        // 若有指定 route_name：必須對應 sys_menu 既有 enabled + non-constant menu
        if let Some(ref route_name) = home_normalized {
            let menu = sys_menu::find_active()
                .filter(SysMenuColumn::RouteName.eq(route_name.as_str()))
                .filter(SysMenuColumn::Status.eq(Status::Enabled))
                .filter(SysMenuColumn::Constant.eq(false))
                .one(&txn)
                .await
                .map_err(AppError::from)?;
            if menu.is_none() {
                return Err(RoleError::HomeRouteNotFound.into());
            }
        }

        let role = SysRoleActiveModel {
            home_route_name: Set(home_normalized),
            updated_at: Set(Some(Local::now().naive_local())),
            updated_by: Set(Some(actor.id.clone())),
            ..before.clone().into()
        };

        let updated = role.update(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_role",
                entity_id: updated.id.clone(),
                payload_before: Some(audit_snapshot(&before)),
                payload_after: Some(audit_snapshot(&updated)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(())
    }
}
