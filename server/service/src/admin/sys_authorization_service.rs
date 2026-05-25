use async_trait::async_trait;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
};
use server_global::notify_casbin_changed;
use server_model::admin::{
    audit_log,
    entities::{
        casbin_rule::{ActiveModel as CasbinRuleActiveModel, Column as CasbinRuleColumn},
        prelude::{CasbinRule, SysRoleMenu, SysUserRole},
        sys_role_menu::{ActiveModel as SysRoleMenuActiveModel, Column as SysRoleMenuColumn},
        sys_user_role::{ActiveModel as SysUserRoleActiveModel, Column as SysUserRoleColumn},
    },
    facade::{
        sys_domain::{self, Column as SysDomainColumn},
        sys_endpoint::{self, Column as SysEndpointColumn},
        sys_menu::{self, Column as SysMenuColumn},
        sys_role::{self, Column as SysRoleColumn},
        sys_user,
    },
};
use thiserror::Error;

use crate::helper::db_helper;

#[derive(Error, Debug)]
pub enum AuthorizationError {
    #[error("Domain not found")]
    DomainNotFound,
    #[error("Role not found")]
    RoleNotFound,
    #[error("One or more permissions not found")]
    PermissionsNotFound,
    #[error("One or more routes not found")]
    RoutesNotFound,
    #[error("One or more users not found")]
    UsersNotFound,
}

impl From<AuthorizationError> for AppError {
    fn from(error: AuthorizationError) -> Self {
        AppError {
            code: 400,
            message: error.to_string(),
        }
    }
}

#[async_trait]
pub trait TAuthorizationService: Send + Sync {
    /// 为角色分配权限
    async fn assign_permission(
        &self,
        domain: String,
        role_id: String,
        permissions: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError>;

    /// 为角色分配路由
    async fn assign_routes(
        &self,
        domain: String,
        role_id: String,
        route_ids: Vec<i32>,
        actor: &Actor,
    ) -> Result<(), AppError>;

    /// 为角色分配用户
    async fn assign_users(
        &self,
        role_id: String,
        user_ids: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct SysAuthorizationService;

impl SysAuthorizationService {
    async fn check_domain_and_role(
        &self,
        domain_code: &str,
        role_id: &str,
    ) -> Result<(String, String, String), AppError> {
        let db = db_helper::get_db_connection().await?;

        let domain = sys_domain::find_active()
            .filter(SysDomainColumn::Code.eq(domain_code))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let domain = domain.ok_or_else(|| AuthorizationError::DomainNotFound)?;

        let role = sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(role_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let role = role.ok_or_else(|| AuthorizationError::RoleNotFound)?;

        Ok((domain.code, role_id.to_string(), role.code))
    }

    async fn check_role(&self, role_id: &str) -> Result<String, AppError> {
        let db = db_helper::get_db_connection().await?;

        let role = sys_role::find_active()
            .filter(SysRoleColumn::Id.eq(role_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let role = role.ok_or_else(|| AuthorizationError::RoleNotFound)?;

        Ok(role.code)
    }

}

#[async_trait]
impl TAuthorizationService for SysAuthorizationService {
    async fn assign_permission(
        &self,
        domain: String,
        role_id: String,
        permissions: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError> {
        use std::collections::HashMap;

        // ① 既有 validate logic (UNCHANGED)
        let (domain_code, _, role_code) = self.check_domain_and_role(&domain, &role_id).await?;
        let db = db_helper::get_db_connection().await?;

        // Filter input permissions to active endpoints. Empty input (spec E-4 clear-all)
        // OR all-invalid filtered out → both result in empty Vec which we handle correctly
        // (removes all existing rows). Only reject if caller supplied non-empty list but ALL
        // ids were bogus / soft-deleted (mirrors `assign_routes` early-return pattern).
        let raw_input = permissions;
        let valid_permissions = sys_endpoint::find_active()
            .filter(SysEndpointColumn::Id.is_in(raw_input.clone()))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        if !raw_input.is_empty() && valid_permissions.is_empty() {
            return Err(AuthorizationError::PermissionsNotFound.into());
        }

        // ② path/method → endpoint_id 反映表 (txn 外建)
        let all_active_endpoints = sys_endpoint::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;
        let path_method_to_id: HashMap<(String, String), String> = all_active_endpoints
            .iter()
            .map(|ep| ((ep.path.clone(), ep.method.clone()), ep.id.clone()))
            .collect();

        // ③ 開 txn — single all-or-nothing scope
        let txn = db.begin().await.map_err(AppError::from)?;

        // ④ txn 內 SELECT 現有 ptype='p' policies for (role_code, domain_code)
        let existing_rows = CasbinRule::find()
            .filter(CasbinRuleColumn::Ptype.eq("p"))
            .filter(CasbinRuleColumn::V0.eq(&role_code))
            .filter(CasbinRuleColumn::V1.eq(&domain_code))
            .all(&txn)
            .await
            .map_err(AppError::from)?;

        let mut existing_endpoint_ids: Vec<String> = existing_rows
            .iter()
            .filter_map(|r| {
                let v2 = r.v2.clone()?;
                let v3 = r.v3.clone()?;
                path_method_to_id.get(&(v2, v3)).cloned()
            })
            .collect();
        existing_endpoint_ids.sort();
        existing_endpoint_ids.dedup();

        // ⑤ 算 diff
        let mut new_endpoint_ids: Vec<String> =
            valid_permissions.iter().map(|p| p.id.clone()).collect();
        new_endpoint_ids.sort();
        new_endpoint_ids.dedup();

        let new_path_method: Vec<(String, String)> = valid_permissions
            .iter()
            .map(|p| (p.path.clone(), p.method.clone()))
            .collect();
        let existing_path_method: Vec<(String, String)> = existing_rows
            .iter()
            .filter_map(|r| Some((r.v2.clone()?, r.v3.clone()?)))
            .collect();

        let rows_to_add: Vec<CasbinRuleActiveModel> = new_path_method
            .iter()
            .filter(|pm| !existing_path_method.contains(pm))
            .map(|(path, method)| CasbinRuleActiveModel {
                ptype: Set("p".to_string()),
                v0: Set(Some(role_code.clone())),
                v1: Set(Some(domain_code.clone())),
                v2: Set(Some(path.clone())),
                v3: Set(Some(method.clone())),
                // schema reality: v4/v5 為 NOT NULL VARCHAR(125)（sea-orm-adapter migration）;
                // 既有 sea-orm-adapter add_policies 寫 ''（empty string）、本直寫對齊；
                // entity 宣告 Option<String> 為 forward-compat、DB constraint 仍須 non-null。
                v4: Set(Some(String::new())),
                v5: Set(Some(String::new())),
                ..Default::default()
            })
            .collect();

        let ids_to_delete: Vec<i64> = existing_rows
            .iter()
            .filter(|r| match (r.v2.clone(), r.v3.clone()) {
                (Some(v2), Some(v3)) => !new_path_method.contains(&(v2, v3)),
                _ => false,
            })
            .map(|r| r.id)
            .collect();

        // ⑥ 寫 casbin_rule (txn 內)
        if !rows_to_add.is_empty() {
            CasbinRule::insert_many(rows_to_add)
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }
        if !ids_to_delete.is_empty() {
            CasbinRule::delete_many()
                .filter(CasbinRuleColumn::Id.is_in(ids_to_delete))
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        // ⑦ audit 同 txn (payload shape 與 W-FW8 體例完全一致)
        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_role",
                entity_id: role_id.clone(),
                payload_before: Some(serde_json::json!({
                    "roleId": &role_id,
                    "domain": &domain_code,
                    "endpointIds": &existing_endpoint_ids,
                })),
                payload_after: Some(serde_json::json!({
                    "roleId": &role_id,
                    "domain": &domain_code,
                    "endpointIds": &new_endpoint_ids,
                })),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        // ⑧ single commit point
        txn.commit().await.map_err(AppError::from)?;

        // ⑨ notify all replicas (既有 W-F11 pattern, fire-and-log)
        notify_casbin_changed().await;

        Ok(())
    }

    async fn assign_routes(
        &self,
        domain: String,
        role_id: String,
        route_ids: Vec<i32>,
        actor: &Actor,
    ) -> Result<(), AppError> {
        let (domain_code, role_id, _) = self.check_domain_and_role(&domain, &role_id).await?;

        let db = db_helper::get_db_connection().await?;
        let routes = sys_menu::find_active()
            .filter(SysMenuColumn::Id.is_in(route_ids.clone()))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        if !route_ids.is_empty() && routes.is_empty() {
            return Err(AuthorizationError::RoutesNotFound.into());
        }

        let existing_routes = SysRoleMenu::find()
            .filter(
                SysRoleMenuColumn::RoleId
                    .eq(&role_id)
                    .and(SysRoleMenuColumn::Domain.eq(&domain_code)),
            )
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let existing_route_ids: Vec<i32> = existing_routes.iter().map(|r| r.menu_id).collect();

        let new_route_ids: Vec<i32> = route_ids
            .iter()
            .filter(|id| !existing_route_ids.contains(id))
            .cloned()
            .collect();

        let route_ids_to_delete: Vec<i32> = existing_route_ids
            .iter()
            .filter(|id| !route_ids.contains(id))
            .cloned()
            .collect();

        let txn = db.begin().await.map_err(AppError::from)?;

        if !new_route_ids.is_empty() {
            let role_menus: Vec<SysRoleMenuActiveModel> = new_route_ids
                .iter()
                .map(|route_id| SysRoleMenuActiveModel {
                    role_id: sea_orm::Set(role_id.clone()),
                    menu_id: sea_orm::Set(*route_id),
                    domain: sea_orm::Set(domain_code.clone()),
                    ..Default::default()
                })
                .collect();

            SysRoleMenu::insert_many(role_menus)
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        if !route_ids_to_delete.is_empty() {
            SysRoleMenu::delete_many()
                .filter(
                    SysRoleMenuColumn::RoleId
                        .eq(&role_id)
                        .and(SysRoleMenuColumn::Domain.eq(&domain_code))
                        .and(SysRoleMenuColumn::MenuId.is_in(route_ids_to_delete)),
                )
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        // W-FW6 US2 (FR-007): 補 audit_log gap — 在 commit 前寫 sys_role / Update 一筆，
        // payload_before 記既有 menu_ids、payload_after 記新指派 menu_ids。
        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_role",
                entity_id: role_id.clone(),
                payload_before: Some(serde_json::json!({
                    "roleId": &role_id,
                    "domain": &domain_code,
                    "menuIds": &existing_route_ids,
                })),
                payload_after: Some(serde_json::json!({
                    "roleId": &role_id,
                    "domain": &domain_code,
                    "menuIds": &route_ids,
                })),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;

        Ok(())
    }

    async fn assign_users(
        &self,
        role_id: String,
        user_ids: Vec<String>,
        actor: &Actor,
    ) -> Result<(), AppError> {
        let _ = self.check_role(&role_id).await?;

        let db = db_helper::get_db_connection().await?;
        let users = sys_user::find_active()
            .filter(server_model::admin::facade::sys_user::Column::Id.is_in(user_ids.clone()))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        if users.is_empty() {
            return Err(AuthorizationError::UsersNotFound.into());
        }

        let existing_user_roles = SysUserRole::find()
            .filter(SysUserRoleColumn::RoleId.eq(&role_id))
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let existing_user_ids: Vec<String> = existing_user_roles
            .iter()
            .map(|r| r.user_id.clone())
            .collect();

        let new_user_ids: Vec<String> = user_ids
            .iter()
            .filter(|id| !existing_user_ids.contains(id))
            .cloned()
            .collect();

        let user_ids_to_delete: Vec<String> = existing_user_ids
            .iter()
            .filter(|id| !user_ids.contains(id))
            .cloned()
            .collect();

        let txn = db.begin().await.map_err(AppError::from)?;

        if !new_user_ids.is_empty() {
            let user_roles: Vec<SysUserRoleActiveModel> = new_user_ids
                .iter()
                .map(|user_id| SysUserRoleActiveModel {
                    role_id: sea_orm::Set(role_id.clone()),
                    user_id: sea_orm::Set(user_id.clone()),
                    ..Default::default()
                })
                .collect();

            SysUserRole::insert_many(user_roles)
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        if !user_ids_to_delete.is_empty() {
            SysUserRole::delete_many()
                .filter(
                    SysUserRoleColumn::RoleId
                        .eq(&role_id)
                        .and(SysUserRoleColumn::UserId.is_in(user_ids_to_delete)),
                )
                .exec(&txn)
                .await
                .map_err(AppError::from)?;
        }

        // W-FW6 US2 (FR-008): 補 audit_log gap — 在 commit 前寫 sys_role / Update 一筆，
        // payload_before 記既有 user_ids、payload_after 記新指派 user_ids（sys_user_role 無 domain 欄位）。
        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Update,
                entity_type: "sys_role",
                entity_id: role_id.clone(),
                payload_before: Some(serde_json::json!({
                    "roleId": &role_id,
                    "userIds": &existing_user_ids,
                })),
                payload_after: Some(serde_json::json!({
                    "roleId": &role_id,
                    "userIds": &user_ids,
                })),
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
