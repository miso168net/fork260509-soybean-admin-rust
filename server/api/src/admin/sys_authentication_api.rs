use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{ConnectInfo, Query},
    http::HeaderMap,
    Extension, Json,
};
use axum_casbin::CasbinAxumLayer;
use axum_extra::{headers::UserAgent, TypedHeader};
use serde_json::json;
use server_core::web::{
    audit::Actor, auth::User, error::AppError, res::Res, util::ClientIp, validator::ValidatedForm,
    RequestId,
};
use server_service::{
    admin::{
        dto::sys_auth_dto::LoginContext, AssignPermissionDto, AssignRouteDto, AssignUserDto,
        AuthErrorQuery, AuthOutput, ChangePasswordInput, LoginInput, RefreshTokenInput,
        SendCaptchaInput, SysAuthService, SysAuthorizationService, TAuthService,
        TAuthorizationService, UserInfoOutput, UserRoute, VerifyCaptchaInput,
    },
    Audience,
};

pub struct SysAuthenticationApi;

impl SysAuthenticationApi {
    pub async fn login_handler(
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        headers: HeaderMap,
        TypedHeader(user_agent): TypedHeader<UserAgent>,
        Extension(request_id): Extension<RequestId>,
        Extension(service): Extension<Arc<SysAuthService>>,
        ValidatedForm(input): ValidatedForm<LoginInput>,
    ) -> Result<Res<AuthOutput>, AppError> {
        let client_ip = {
            let header_ip = ClientIp::get_real_ip(&headers);
            if header_ip == "unknown" {
                addr.ip().to_string()
            } else {
                header_ip
            }
        };

        let address = xdb::searcher::search_by_ip(client_ip.as_str())
            .unwrap_or_else(|_| "Unknown Location".to_string());

        let login_context = LoginContext {
            client_ip,
            client_port: Some(addr.port() as i32),
            address,
            user_agent: user_agent.as_str().to_string(),
            request_id: request_id.to_string(),
            audience: Audience::ManagementPlatform,
            login_type: "PC".to_string(),
            domain: "built-in".to_string(),
        };

        service
            .pwd_login(input, login_context)
            .await
            .map(Res::new_data)
    }

    /// F13: Refresh token rotation handler — same extractor pattern as login_handler.
    pub async fn refresh_token_handler(
        ConnectInfo(addr): ConnectInfo<SocketAddr>,
        headers: HeaderMap,
        TypedHeader(user_agent): TypedHeader<UserAgent>,
        Extension(request_id): Extension<RequestId>,
        Extension(service): Extension<Arc<SysAuthService>>,
        ValidatedForm(input): ValidatedForm<RefreshTokenInput>,
    ) -> Result<Res<AuthOutput>, AppError> {
        let client_ip = {
            let header_ip = ClientIp::get_real_ip(&headers);
            if header_ip == "unknown" {
                addr.ip().to_string()
            } else {
                header_ip
            }
        };

        let address = xdb::searcher::search_by_ip(client_ip.as_str())
            .unwrap_or_else(|_| "Unknown Location".to_string());

        let refresh_context = LoginContext {
            client_ip,
            client_port: Some(addr.port() as i32),
            address,
            user_agent: user_agent.as_str().to_string(),
            request_id: request_id.to_string(),
            audience: Audience::ManagementPlatform,
            login_type: "PC".to_string(),
            domain: "built-in".to_string(),
        };

        service
            .refresh_token(input.refresh_token, refresh_context)
            .await
            .map(Res::new_data)
    }

    pub async fn get_user_info(
        Extension(user): Extension<User>,
    ) -> Result<Res<UserInfoOutput>, AppError> {
        let user_info = UserInfoOutput {
            user_id: user.user_id(),
            user_name: user.username(),
            // F7.2: 映射 role code 對齊 base-web static route filter(見 map_role_alias)
            roles: user.subject().iter().map(|c| map_role_alias(c)).collect(),
            buttons: vec![],
        };

        Ok(Res::new_data(user_info))
    }

    pub async fn get_user_routes(
        Extension(user): Extension<User>,
        Extension(service): Extension<Arc<SysAuthService>>,
    ) -> Result<Res<UserRoute>, AppError> {
        let routes = service
            .get_user_routes(&user.subject(), &user.domain())
            .await?;

        Ok(Res::new_data(routes))
    }

    /// 为角色分配权限
    ///
    /// 将指定的权限分配给指定域中的角色。
    pub async fn assign_permission(
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        Extension(mut cache_enforcer): Extension<CasbinAxumLayer>,
        ValidatedForm(input): ValidatedForm<AssignPermissionDto>,
    ) -> Result<Res<()>, AppError> {
        let enforcer = cache_enforcer.get_enforcer();

        service
            .assign_permission(input.domain, input.role_id, input.permissions, enforcer)
            .await?;

        Ok(Res::new_data(()))
    }

    /// 为角色分配路由
    ///
    /// 将指定的路由分配给指定域中的角色。
    /// W-FW6 US2 (FR-007): handler 注入 Actor 給 service 寫 audit_log。
    pub async fn assign_routes(
        Extension(user): Extension<User>,
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        ValidatedForm(input): ValidatedForm<AssignRouteDto>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service
            .assign_routes(input.domain, input.role_id, input.route_ids, &actor)
            .await?;

        Ok(Res::new_data(()))
    }

    /// 为角色分配用户
    ///
    /// 将指定的用户分配给指定角色。
    /// W-FW6 US2 (FR-008): handler 注入 Actor 給 service 寫 audit_log。
    pub async fn assign_users(
        Extension(user): Extension<User>,
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        ValidatedForm(input): ValidatedForm<AssignUserDto>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service
            .assign_users(input.role_id, input.user_ids, &actor)
            .await?;
        Ok(Res::new_data(()))
    }

    /// W-FW5 US3: 自助改密碼 handler — `Extension<User>` 取「我是誰」，
    /// 驗舊密碼後改自己的密碼。
    pub async fn change_password(
        Extension(user): Extension<User>,
        Extension(service): Extension<Arc<SysAuthService>>,
        ValidatedForm(input): ValidatedForm<ChangePasswordInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        service
            .change_password(
                &user.user_id(),
                &input.current_password,
                &input.new_password,
                &actor,
            )
            .await
            .map(Res::new_data)
    }

    // F11 extracted-stubs: 3 個 stub handler(per spec FR-001 / FR-002 / FR-003）
    pub async fn send_captcha(
        Json(input): Json<SendCaptchaInput>,
    ) -> Result<Res<serde_json::Value>, AppError> {
        tracing::info!(phone = %input.phone, "F11 stub: sendCaptcha called");
        Ok(Res::new_data(json!({ "code": "000000" })))
    }

    pub async fn verify_captcha(
        Json(input): Json<VerifyCaptchaInput>,
    ) -> Result<Res<serde_json::Value>, AppError> {
        let verified = input.code == "000000";
        Ok(Res::new_data(json!({ "verified": verified })))
    }

    pub async fn auth_error(
        Query(q): Query<AuthErrorQuery>,
    ) -> Result<Res<serde_json::Value>, AppError> {
        Ok(Res::new_data(json!({
            "code": q.code.unwrap_or_default(),
            "msg":  q.msg.unwrap_or_default(),
        })))
    }
}

/// F7.2 role-code-alignment:rust sys_role.code(ROLE_*)→ base-web example
/// static route filter 期望的 role code(R_*)alias 映射。
/// 只用於 getUserInfo response 邊界;JWT Claims.role / casbin_rule.v0 /
/// sys_role.code DB 維持 ROLE_*、Casbin enforce 不受影響。
fn map_role_alias(code: &str) -> String {
    match code {
        "ROLE_SUPER" => "R_SUPER".to_string(),
        "ROLE_ADMIN" => "R_ADMIN".to_string(),
        "ROLE_USER" => "R_USER".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::map_role_alias;

    #[test]
    fn test_map_role_alias() {
        // 3 known role code → R_* alias
        assert_eq!(map_role_alias("ROLE_SUPER"), "R_SUPER");
        assert_eq!(map_role_alias("ROLE_ADMIN"), "R_ADMIN");
        assert_eq!(map_role_alias("ROLE_USER"), "R_USER");
        // unknown code → pass through 原樣
        assert_eq!(map_role_alias("ROLE_FUTURE"), "ROLE_FUTURE");
    }
}
