use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, IntoActiveModel,
    JoinType, PaginatorTrait, QueryFilter, QuerySelect, RelationTrait, Set, TransactionTrait,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_global::snowflake;
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::{
        prelude::SysUserRole,
        sea_orm_active_enums::Gender,
        sys_user_role::{
            ActiveModel as SysUserRoleActiveModel, Column as SysUserRoleColumn,
            Relation as SysUserRoleRelation,
        },
    },
    facade::{
        sys_role::{self, Column as SysRoleColumn},
        sys_user::{self, ActiveModel as SysUserActiveModel, Column as SysUserColumn},
    },
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
    /// 039 T030.5: 新增 `user_id: &str` 參數承接 handler 端 lookup_ulid_by_display_id 結果。
    /// `input.id` 為 i64 wire field、service 內部不使用（identity 走 user_id ULID）。
    async fn update_user(
        &self,
        user_id: &str,
        input: UpdateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError>;
    async fn delete_user(&self, id: &str, actor: &Actor) -> Result<(), AppError>;

    /// W-FW5 US1: user→roles delta 指派。`role_codes` 為 role code 清單，
    /// 空清單為合法（清空該 user 全部角色）。無效 code → 拒絕。
    async fn assign_roles_to_user(
        &self,
        user_id: String,
        role_codes: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError>;

    /// W-FW5 US1: 批次查多個 user 的 role code 集合（一次 query、記憶體 group by user_id，避免 N+1）。
    /// 回傳 map：user_id → role code 清單；無角色的 user 不會出現在 map。
    async fn get_role_codes_for_users(
        &self,
        user_ids: Vec<String>,
    ) -> Result<std::collections::HashMap<String, Vec<String>>, AppError>;

    /// 039 rust-entity-id-numeric-migration C3: by-display_id lookup helper。
    /// base-web 對外傳 numeric display_id；rust 內部 PK/FK 仍走 ULID 字串。
    /// handler 收 Path<i64> 後第一步透過本方法解析回 ULID，再走後續 service 既有路徑。
    /// 軟刪資料不可解析（find_active() filter DeletedAt.is_null）。
    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError>;
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
            display_id: Set(snowflake::next_display_id()),
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
        user_id: &str,
        input: UpdateUserInput,
        actor: &Actor,
    ) -> Result<UserWithoutPassword, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        // 039 T030.5: identity 走 handler 端 lookup 過的 ULID；input.id（i64 wire field）不採用。
        let before = sys_user::find_active()
            .filter(SysUserColumn::Id.eq(user_id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(UserError::UserNotFound))?;

        if input.username != before.username {
            self.check_username_unique_in_txn(&txn, &input.username)
                .await?;
        }

        let mut user = before.clone().into_active_model();
        user.domain = Set(input.domain);
        user.username = Set(input.username);
        if let Some(pw) = input.password {
            // W-FW5 T008: 修正 update_user 密碼未 hash bug — 與 create_user 一致
            let hashed = SecureUtil::hash_password(pw.as_bytes()).map_err(|e| AppError {
                code: server_core::web::code::CODE_SERVER_INTERNAL_ERROR,
                message: format!("password hash failed: {}", e),
            })?;
            user.password = Set(hashed);
        }
        user.nick_name = Set(input.nick_name);
        user.avatar = Set(input.avatar);
        user.email = Set(input.email);
        user.phone_number = Set(input.phone_number);
        user.status = Set(input.status);
        user.gender = Set(input.gender);

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

    async fn assign_roles_to_user(
        &self,
        user_id: String,
        role_codes: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;

        // 去重後解析（防 base-web 異常送重複 code 時誤判 InvalidRoleCode）
        let mut unique_codes: Vec<String> = role_codes;
        unique_codes.sort();
        unique_codes.dedup();

        // role code → role id 解析（無效 code → 拒絕，spec E-6）
        let roles = sys_role::find_active()
            .filter(SysRoleColumn::Code.is_in(unique_codes.clone()))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        if roles.len() != unique_codes.len() {
            return Err(UserError::InvalidRoleCode.into());
        }

        // role id 集合（去重後對齊 sys_user_role 主鍵）
        let target_role_ids: Vec<String> = roles.iter().map(|r| r.id.clone()).collect();

        // 撈該 user 既有 sys_user_role
        let existing = SysUserRole::find()
            .filter(SysUserRoleColumn::UserId.eq(&user_id))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let existing_role_ids: Vec<String> = existing.iter().map(|r| r.role_id.clone()).collect();

        let new_role_ids: Vec<String> = target_role_ids
            .iter()
            .filter(|id| !existing_role_ids.contains(id))
            .cloned()
            .collect();

        let role_ids_to_delete: Vec<String> = existing_role_ids
            .iter()
            .filter(|id| !target_role_ids.contains(id))
            .cloned()
            .collect();

        let txn = db.begin().await.map_err(AppError::from)?;

        if !new_role_ids.is_empty() {
            let user_roles: Vec<SysUserRoleActiveModel> = new_role_ids
                .iter()
                .map(|role_id| SysUserRoleActiveModel {
                    user_id: Set(user_id.clone()),
                    role_id: Set(role_id.clone()),
                })
                .collect();

            SysUserRole::insert_many(user_roles)
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        if !role_ids_to_delete.is_empty() {
            SysUserRole::delete_many()
                .filter(
                    SysUserRoleColumn::UserId
                        .eq(&user_id)
                        .and(SysUserRoleColumn::RoleId.is_in(role_ids_to_delete.clone())),
                )
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        // Constitution II：新寫入路徑必含 audit。payload before/after 為角色 id 集合（key roleIds）。
        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_user_role",
                entity_id: user_id.clone(),
                payload_before: Some(serde_json::json!({ "roleIds": existing_role_ids })),
                payload_after: Some(serde_json::json!({ "roleIds": target_role_ids })),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(())
    }

    async fn get_role_codes_for_users(
        &self,
        user_ids: Vec<String>,
    ) -> Result<std::collections::HashMap<String, Vec<String>>, AppError> {
        use std::collections::HashMap;

        let mut result: HashMap<String, Vec<String>> = HashMap::new();
        if user_ids.is_empty() {
            return Ok(result);
        }

        let db = db_helper::get_db_connection().await?;

        // 一次撈清單所有 user 的關聯（sys_user_role JOIN sys_role 取 role code），記憶體 group by。
        // DeletedAt.is_null() 過濾與 assign_roles_to_user 走的 sys_role::find_active() 一致 ——
        // 避免已軟刪 role 的殘留關聯出現在 getUserList 的 userRoles。
        let pairs: Vec<(String, String)> = SysUserRole::find()
            .select_only()
            .column(SysUserRoleColumn::UserId)
            .column(SysRoleColumn::Code)
            .join(JoinType::InnerJoin, SysUserRoleRelation::SysRole.def())
            .filter(SysUserRoleColumn::UserId.is_in(user_ids))
            .filter(SysRoleColumn::DeletedAt.is_null())
            .into_tuple()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        for (user_id, role_code) in pairs {
            result.entry(user_id).or_default().push(role_code);
        }

        Ok(result)
    }

    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError> {
        let db = db_helper::get_db_connection().await?;
        let user = sys_user::find_active()
            .filter(SysUserColumn::DisplayId.eq(display_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(UserError::UserNotFound))?;
        Ok(user.id)
    }
}
