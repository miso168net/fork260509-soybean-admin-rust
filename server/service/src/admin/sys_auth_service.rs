use std::any::Any;

use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, JoinType,
    QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set, TransactionTrait,
    sea_query::Expr,
};
use server_constant::definition::{
    consts::{SystemEvent, TokenStatus},
    Audience,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    auth::Claims,
    code,
    error::AppError,
    jwt::{JwtError, JwtUtils},
};
use server_global::global;
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    entities::{
        sea_orm_active_enums::Status,
        sys_role_menu::{Column as SysRoleMenuColumn, Entity as SysRoleMenuEntity},
        sys_tokens::{
            ActiveModel as SysTokensActiveModel, Column as SysTokensColumn,
            Entity as SysTokensEntity,
        },
        sys_user_role::Relation as SysUserRoleRelation,
    },
    facade::{
        sys_domain::Column as SysDomainColumn,
        sys_menu::{self, Column as SysMenuColumn, Model as SysMenuModel},
        sys_role::{self, Column as SysRoleColumn, Relation as SysRoleRelation},
        sys_user::{self, Column as SysUserColumn, Relation as SysUserRelation},
    },
    input::LoginInput,
    output::{AuthOutput, MenuRoute, RouteMeta, UserRoute, UserWithDomainAndOrgOutput},
};
use server_utils::{SecureUtil, TreeBuilder};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, instrument};
use ulid::Ulid;
use super::{
    dto::sys_auth_dto::LoginContext, event_handlers::auth_event_handler::AuthEventHandler,
};
use crate::{
    admin::{event_handlers::auth_event_handler::AuthEvent, sys_user_error::UserError},
    helper::db_helper,
    project_error, project_info,
};

macro_rules! select_user_with_domain_and_org_info {
    ($query:expr) => {{
        $query
            .select_only()
            .column_as(SysUserColumn::Id, "id")
            .column_as(SysUserColumn::Domain, "domain")
            .column_as(SysUserColumn::Username, "username")
            .column_as(SysUserColumn::Password, "password")
            .column_as(SysUserColumn::NickName, "nick_name")
            .column_as(SysUserColumn::Avatar, "avatar")
            .column_as(SysDomainColumn::Code, "domain_code")
            .column_as(SysDomainColumn::Name, "domain_name")
    }};
}
#[derive(Error, Debug)]
pub enum EventError {
    #[error("Failed to send event: {0}")]
    SendError(#[from] tokio::sync::mpsc::error::SendError<Box<dyn std::any::Any + Send>>),
    #[error("Failed to handle login event: {0}")]
    LoginHandlerError(String),
}

#[async_trait]
pub trait TAuthService: Send + Sync {
    async fn pwd_login(
        &self,
        input: LoginInput,
        context: LoginContext,
    ) -> Result<AuthOutput, AppError>;

    async fn get_user_routes(
        &self,
        role_codes: &[String],
        domain: &str,
    ) -> Result<UserRoute, AppError>;
}

#[derive(Clone)]
pub struct SysAuthService;

impl SysAuthService {
    /// 查找第一个有效的路由路径
    #[allow(dead_code)]
    fn find_first_valid_route(routes: &[MenuRoute]) -> Option<String> {
        for route in routes {
            if !route.path.is_empty() && route.path != "/" {
                return Some(route.path.clone());
            }
            if let Some(children) = &route.children {
                if let Some(path) = Self::find_first_valid_route(children) {
                    return Some(path);
                }
            }
        }
        None
    }
}

#[async_trait]
impl TAuthService for SysAuthService {
    #[instrument(skip(self, input), fields(username = %input.identifier, domain = %context.domain))]
    async fn pwd_login(
        &self,
        input: LoginInput,
        context: LoginContext,
    ) -> Result<AuthOutput, AppError> {
        // 验证用户并获取角色
        let (user, role_codes) = self
            .verify_user(&input.identifier, &input.password, &context.domain)
            .await?;

        // 生成认证输出
        let auth_output = generate_auth_output(
            user.id.clone(),
            user.username.clone(),
            role_codes,
            user.domain_code.clone(),
            None,
            context.audience,
        )
        .await?;

        // 发送认证事件
        self.send_login_event(&user, &auth_output, &context).await;

        Ok(auth_output)
    }

    #[instrument(skip(self), fields(roles = ?role_codes, domain = %domain))]
    async fn get_user_routes(
        &self,
        role_codes: &[String],
        domain: &str,
    ) -> Result<UserRoute, AppError> {
        if role_codes.is_empty() {
            return Ok(UserRoute {
                routes: vec![],
                home: "/home".to_string(),
            });
        }

        let db = db_helper::get_db_connection().await?;

        let menu_ids = SysRoleMenuEntity::find()
            .select_only()
            .column(SysRoleMenuColumn::MenuId)
            .join_rev(
                JoinType::InnerJoin,
                SysRoleRelation::SysRoleMenu.def(),
            )
            .filter(SysRoleColumn::Code.is_in(role_codes.to_vec()))
            .filter(SysRoleMenuColumn::Domain.eq(domain))
            .distinct()
            .into_tuple::<i32>()
            .all(db.as_ref())
            .await?;

        let menus = sys_menu::find_active()
            .filter(SysMenuColumn::Id.is_in(menu_ids))
            .filter(SysMenuColumn::Status.eq(Status::Enabled))
            .order_by_asc(SysMenuColumn::Sequence)
            .into_model::<SysMenuModel>()
            .all(db.as_ref())
            .await?;

        let menu_routes: Vec<MenuRoute> = menus
            .into_iter()
            .map(|menu| MenuRoute {
                name: menu.route_name,
                path: menu.route_path,
                component: menu.component,
                meta: RouteMeta {
                    title: menu.menu_name,
                    i18n_key: menu.i18n_key,
                    keep_alive: menu.keep_alive,
                    constant: menu.constant,
                    icon: menu.icon,
                    order: menu.sequence,
                    href: menu.href,
                    hide_in_menu: menu.hide_in_menu,
                    active_menu: menu.active_menu,
                    multi_tab: menu.multi_tab,
                },
                children: Some(vec![]),
                id: menu.id,
                pid: menu.pid,
            })
            .collect();

        let menu_routes_ref = menu_routes.clone();

        let routes = TreeBuilder::build(
            menu_routes,
            |route| route.name.clone(),
            |route| {
                if route.pid == "0" {
                    None
                } else {
                    menu_routes_ref
                        .iter()
                        .find(|m| m.id.to_string() == route.pid)
                        .map(|m| m.name.clone())
                }
            },
            |route| route.meta.order,
            |route, children| {
                route.children = Some(children);
            },
        );

        // let home = Self::find_first_valid_route(&routes).unwrap_or_else(|| "/home".to_string());
        let home = "home".to_string();

        Ok(UserRoute { routes, home })
    }
}

impl SysAuthService {
    /// F13: Refresh token rotation — validate JWT + DB state, re-issue token pair, mark old as used.
    pub async fn refresh_token(
        &self,
        refresh_token: String,
        context: LoginContext,
    ) -> Result<AuthOutput, AppError> {
        let db = db_helper::get_db_connection().await?;

        // Step 1: Validate refresh token JWT signature + expiry
        let token_data = JwtUtils::validate_refresh_token(&refresh_token)
            .await
            .map_err(|_| AppError {
                code: code::CODE_EXPIRED_TOKEN_SIGNATURE,
                message: "invalid or expired refresh token".to_string(),
            })?;
        let user_id = token_data.claims.sub().to_string();

        // Step 2: Find sys_tokens row by refresh_token value
        let token_row = SysTokensEntity::find()
            .filter(SysTokensColumn::RefreshToken.eq(&refresh_token))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError {
                code: code::CODE_EXPIRED_TOKEN_SIGNATURE,
                message: "refresh token not found".to_string(),
            })?;

        // Step 3: Check status is Active ("unused")
        if token_row.status != TokenStatus::Active.to_string() {
            return Err(AppError {
                code: code::CODE_EXPIRED_TOKEN_SIGNATURE,
                message: "refresh token already used or revoked".to_string(),
            });
        }

        // Step 4: Re-query user (also acts as soft-delete check per R-Q7)
        let user = sys_user::find_active()
            .filter(SysUserColumn::Id.eq(&user_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError {
                code: code::CODE_LOGOUT_SESSION_INVALIDATED,
                message: "user not found or deleted".to_string(),
            })?;

        let username = user.username.clone();
        let domain_code = user.domain.clone(); // sys_user.domain IS the domain code (FK → sys_domain.code)

        // Step 5: Re-query user roles
        let role_codes = self.get_user_roles(&user_id, &db).await?;

        // Step 6: Generate new access + refresh token pair
        let auth_output = generate_auth_output(
            user_id.clone(),
            username.clone(),
            role_codes,
            domain_code.clone(),
            None,
            Audience::ManagementPlatform,
        )
        .await
        .map_err(AppError::from)?;

        // Step 7: Rotation transaction (E4) — UPDATE old row + INSERT new row atomically
        let txn = db.begin().await.map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("begin txn failed: {}", e),
        })?;

        // (a) Mark old row as used, filter on status=Active to handle concurrency race (E-7)
        let update_res = SysTokensEntity::update_many()
            .col_expr(
                SysTokensColumn::Status,
                Expr::value(TokenStatus::Refreshed.to_string()),
            )
            .filter(SysTokensColumn::RefreshToken.eq(&refresh_token))
            .filter(SysTokensColumn::Status.eq(TokenStatus::Active.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| AppError {
                code: code::CODE_SERVER_DB_ERROR,
                message: format!("update old token status failed: {}", e),
            })?;

        if update_res.rows_affected != 1 {
            txn.rollback().await.ok();
            return Err(AppError {
                code: code::CODE_EXPIRED_TOKEN_SIGNATURE,
                message: "refresh token concurrency conflict".to_string(),
            });
        }

        // (b) Insert new token pair row
        let now = Local::now().naive_local();
        SysTokensActiveModel {
            id: Set(Ulid::new().to_string()),
            access_token: Set(auth_output.token.clone()),
            refresh_token: Set(auth_output.refresh_token.clone()),
            status: Set(TokenStatus::Active.to_string()),
            user_id: Set(user_id),
            username: Set(username.clone()),
            domain: Set(domain_code),
            login_time: Set(now),
            ip: Set(context.client_ip),
            port: Set(context.client_port),
            address: Set(context.address),
            user_agent: Set(context.user_agent),
            request_id: Set(context.request_id),
            r#type: Set(context.login_type),
            created_at: Set(now),
            created_by: Set(username),
        }
        .insert(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("insert new token row failed: {}", e),
        })?;

        txn.commit().await.map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("commit txn failed: {}", e),
        })?;

        // Step 8: Return new token pair
        Ok(auth_output)
    }

    /// W-FW5 US3: 自助改密碼 — 驗舊密碼 → hash 新密碼 → 同 txn update + audit。
    ///
    /// 舊密碼錯誤 → `UserError::WrongPassword`（對應 spec edge case E-5）。
    pub async fn change_password(
        &self,
        user_id: &str,
        current_password: &str,
        new_password: &str,
        actor: &Actor,
    ) -> Result<bool, AppError> {
        let db = db_helper::get_db_connection().await?;

        let txn = db.begin().await.map_err(AppError::from)?;

        let before = sys_user::find_active()
            .filter(SysUserColumn::Id.eq(user_id))
            .one(&txn)
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(UserError::UserNotFound))?;

        // 驗舊密碼（錯誤 → WrongPassword、對應 spec E-5）
        if !SecureUtil::verify_password(current_password.as_bytes(), &before.password)
            .map_err(|_| AppError::from(UserError::AuthenticationFailed))?
        {
            txn.rollback().await.ok();
            return Err(AppError::from(UserError::WrongPassword));
        }

        // hash 新密碼
        let hashed = SecureUtil::hash_password(new_password.as_bytes()).map_err(|e| AppError {
            code: code::CODE_SERVER_INTERNAL_ERROR,
            message: format!("password hash failed: {}", e),
        })?;

        let mut user = before.clone().into_active_model();
        user.password = Set(hashed);
        let updated_user = user.update(&txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            &txn,
            AuditEvent {
                actor: actor.clone(),
                operation: AuditOperation::Update,
                entity_type: "sys_user",
                entity_id: updated_user.id.clone(),
                payload_before: Some(audit_snapshot(&before)),
                payload_after: Some(audit_snapshot(&updated_user)),
                description: Some("self-service change password".to_string()),
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        txn.commit().await.map_err(AppError::from)?;
        Ok(true)
    }

    /// 验证用户身份
    async fn verify_user(
        &self,
        identifier: &str,
        password: &str,
        domain: &str,
    ) -> Result<(UserWithDomainAndOrgOutput, Vec<String>), AppError> {
        let db = db_helper::get_db_connection().await?;

        let user = select_user_with_domain_and_org_info!(sys_user::find_active())
            .filter(SysUserColumn::Username.eq(identifier))
            .filter(SysDomainColumn::Code.eq(domain))
            .join(JoinType::InnerJoin, SysUserRelation::SysDomain.def())
            .into_model::<UserWithDomainAndOrgOutput>()
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(UserError::UserNotFound))?;

        //TODO validate user status and domain status

        // 验证密码
        if !SecureUtil::verify_password(password.as_bytes(), &user.password)
            .map_err(|_| AppError::from(UserError::AuthenticationFailed))?
        {
            return Err(AppError::from(UserError::WrongPassword));
        }

        // 获取角色
        let role_codes = self.get_user_roles(&user.id, &db).await?;

        Ok((user, role_codes))
    }

    /// 获取用户角色
    async fn get_user_roles(
        &self,
        user_id: &str,
        db: &DatabaseConnection,
    ) -> Result<Vec<String>, AppError> {
        sys_role::find_active()
            .join(JoinType::InnerJoin, SysRoleRelation::SysUserRole.def())
            .join(JoinType::InnerJoin, SysUserRoleRelation::SysUser.def())
            .filter(SysUserColumn::Id.eq(user_id))
            .all(db)
            .await
            .map(|roles| roles.iter().map(|role| role.code.clone()).collect())
            .map_err(AppError::from)
    }

    async fn send_login_event(
        &self,
        user: &UserWithDomainAndOrgOutput,
        auth_output: &AuthOutput,
        context: &LoginContext,
    ) {
        let auth_event = AuthEvent {
            user_id: user.id.clone(),
            username: user.username.clone(),
            domain: user.domain_code.clone(),
            access_token: auth_output.token.clone(),
            refresh_token: auth_output.refresh_token.clone(),
            client_ip: context.client_ip.clone(),
            client_port: context.client_port,
            address: context.address.clone(),
            user_agent: context.user_agent.clone(),
            request_id: context.request_id.clone(),
            login_type: context.login_type.clone(),
        };

        global::send_dyn_event(
            SystemEvent::AuthLoggedInEvent.as_ref(),
            Box::new(auth_event),
        );
    }

    async fn check_login_security(
        &self,
        _username: &str,
        _client_ip: &str,
    ) -> Result<(), AppError> {
        // TODO: 实现登录安全检查
        // 1. 检查登录失败次数
        // 2. 检查 IP 黑名单
        // 3. 检查账号是否被锁定
        // 4. 检查是否在允许的时间范围内
        Ok(())
    }

    #[allow(dead_code)]
    async fn pwd_login_with_security(
        &self,
        input: LoginInput,
        context: LoginContext,
    ) -> Result<AuthOutput, AppError> {
        self.check_login_security(&input.identifier, &context.client_ip)
            .await?;

        self.pwd_login(input, context).await
    }
}

#[allow(dead_code)]
#[instrument(skip(sender, auth_event))]
async fn send_auth_event(
    sender: mpsc::UnboundedSender<Box<dyn std::any::Any + Send>>,
    auth_event: AuthEvent,
) -> Result<(), EventError> {
    sender
        .send(Box::new(auth_event))
        .map_err(EventError::from)?;
    Ok(())
}

pub async fn generate_auth_output(
    user_id: String,
    username: String,
    role_codes: Vec<String>,
    domain_code: String,
    organization_name: Option<String>,
    audience: Audience,
) -> Result<AuthOutput, JwtError> {
    // Clone user_id before it is moved into Claims::new (F10.1 T028)
    let user_id_for_refresh = user_id.clone();
    let claims = Claims::new(
        user_id,
        audience.as_str().to_string(),
        username,
        role_codes,
        domain_code,
        organization_name,
    );

    let token = JwtUtils::generate_token(&claims).await?;

    Ok(AuthOutput {
        token,
        refresh_token: JwtUtils::generate_refresh_token(user_id_for_refresh).await?,
    })
}

#[instrument(skip(rx))]
pub async fn auth_login_listener(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>,
) {
    while let Some(event) = rx.recv().await {
        if let Some(auth_event) = event.downcast_ref::<AuthEvent>() {
            if let Err(e) = handle_auth_event(auth_event).await {
                project_error!("Failed to handle AuthEvent: {:?}", e);
            }
        }
    }
}

#[instrument(skip(auth_event), fields(user_id = %auth_event.user_id, username = %auth_event.username))]
async fn handle_auth_event(auth_event: &AuthEvent) -> Result<(), EventError> {
    AuthEventHandler::handle_login(AuthEvent {
        user_id: auth_event.user_id.clone(),
        username: auth_event.username.clone(),
        domain: auth_event.domain.clone(),
        access_token: auth_event.access_token.clone(),
        refresh_token: auth_event.refresh_token.clone(),
        client_ip: auth_event.client_ip.clone(),
        address: auth_event.address.clone(),
        client_port: auth_event.client_port,
        user_agent: auth_event.user_agent.clone(),
        request_id: auth_event.request_id.clone(),
        login_type: auth_event.login_type.clone(),
    })
    .await
    .map_err(|e| EventError::LoginHandlerError(format!("{:?}", e)))
}

#[instrument(skip(rx))]
pub async fn jwt_created_listener(mut rx: tokio::sync::mpsc::UnboundedReceiver<String>) {
    while let Some(jwt) = rx.recv().await {
        project_info!("JWT created: {}", jwt);
        // TODO: Consider storing the token into the database
    }
}
