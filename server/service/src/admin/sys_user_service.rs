use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, IntoActiveModel, PaginatorTrait,
    QueryFilter, Set, TransactionTrait,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::sea_orm_active_enums::Gender,
    facade::sys_user::{self, ActiveModel as SysUserActiveModel, Column as SysUserColumn},
    input::{CreateUserInput, UpdateUserInput, UserPageRequest},
    output::UserWithoutPassword,
};
use server_utils::SecureUtil;
use ulid::Ulid;

use super::sys_user_error::UserError;
use crate::helper::db_helper;

#[async_trait]
pub trait TUserService {
    async fn find_all(&self) -> Result<Vec<UserWithoutPassword>, AppError>;
    async fn find_paginated_users(
        &self,
        params: UserPageRequest,
    ) -> Result<PaginatedData<UserWithoutPassword>, AppError>;

    async fn create_user(
        &self,
        input: CreateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError>;
    async fn get_user(&self, id: &str) -> Result<UserWithoutPassword, AppError>;
    async fn update_user(
        &self,
        input: UpdateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError>;
    async fn delete_user(&self, id: &str, actor: &Actor) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct SysUserService;

impl SysUserService {
    async fn check_username_unique_in_txn<C: ConnectionTrait>(
        &self,
        txn: &C,
        username: &str,
    ) -> Result<(), AppError> {
        let existing_user = sys_user::find_active()
            .filter(SysUserColumn::Username.eq(username))
            .one(txn)
            .await
            .map_err(AppError::from)?;

        if existing_user.is_some() {
            return Err(UserError::UsernameAlreadyExists.into());
        }
        Ok(())
    }

}

#[async_trait]
impl TUserService for SysUserService {
    async fn find_all(&self) -> Result<Vec<UserWithoutPassword>, AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_user::find_active()
            .all(db.as_ref())
            .await
            .map(|users| users.into_iter().map(UserWithoutPassword::from).collect())
            .map_err(AppError::from)
    }

    async fn find_paginated_users(
        &self,
        params: UserPageRequest,
    ) -> Result<PaginatedData<UserWithoutPassword>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_user::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any().add(SysUserColumn::Username.contains(keywords));
            query = query.filter(condition);
        }

        match params.user_gender.as_deref() {
            Some("1") => {
                query = query.filter(SysUserColumn::Gender.eq(Gender::Male));
            }
            Some("2") => {
                query = query.filter(SysUserColumn::Gender.eq(Gender::Female));
            }
            _ => {}
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
            .map_err(AppError::from)?
            .into_iter()
            .map(UserWithoutPassword::from)
            .collect();

        Ok(PaginatedData {
            current: params.page_details.current,
            size: params.page_details.size,
            total,
            records,
        })
    }

    async fn create_user(
        &self,
        input: CreateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        self.check_username_unique_in_txn(&txn, &input.username)
            .await?;

        let user = SysUserActiveModel {
            id: Set(Ulid::new().to_string()),
            domain: Set(input.domain),
            username: Set(input.username),
            password: Set(SecureUtil::hash_password(input.password.as_bytes()).unwrap()),
            built_in: Set(false),
            nick_name: Set(input.nick_name),
            avatar: Set(input.avatar),
            email: Set(input.email),
            phone_number: Set(input.phone_number),
            status: Set(input.status),
            gender: Set(input.gender),
            created_at: Set(Local::now().naive_local()),
            created_by: Set("TODO".to_string()),
            ..Default::default()
        };

        let user_model = user.insert(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Insert,
                entity_type: "sys_user",
                entity_id: user_model.id.clone(),
                payload_before: None,
                payload_after: Some(audit_snapshot(&user_model)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(UserWithoutPassword::from(user_model))
    }

    async fn get_user(&self, id: &str) -> Result<UserWithoutPassword, AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_user::find_active()
            .filter(SysUserColumn::Id.eq(id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .map(UserWithoutPassword::from)
            .ok_or_else(|| UserError::UserNotFound.into())
    }

    async fn update_user(
        &self,
        input: UpdateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        let before = sys_user::find_active()
            .filter(SysUserColumn::Id.eq(&input.id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(UserError::UserNotFound))?;

        if input.user.username != before.username {
            self.check_username_unique_in_txn(&txn, &input.user.username)
                .await?;
        }

        let mut user = before.clone().into_active_model();
        user.domain = Set(input.user.domain);
        user.username = Set(input.user.username);
        user.password = Set(input.user.password); // TODO: Note: In a real application, you should hash the password
        user.nick_name = Set(input.user.nick_name);
        user.avatar = Set(input.user.avatar);
        user.email = Set(input.user.email);
        user.phone_number = Set(input.user.phone_number);
        user.status = Set(input.user.status);
        user.gender = Set(input.user.gender);

        let updated_user = user.update(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_user",
                entity_id: updated_user.id.clone(),
                payload_before: Some(audit_snapshot(&before)),
                payload_after: Some(audit_snapshot(&updated_user)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(UserWithoutPassword::from(updated_user))
    }

    async fn delete_user(&self, id: &str, actor: &Actor) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;
        sys_user::soft_delete_by_id(db.as_ref(), id.to_string(), actor).await
    }
}
