use axum::{body::Body, extract::Request, middleware::Next, response::IntoResponse};
use axum_casbin::CasbinVals;
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use sea_orm::{ColumnTrait, QueryFilter};
use server_core::web::{auth::User, code, error::AppError, jwt::JwtUtils, res::Res};
use server_global::global::GLOBAL_PRIMARY_DB;
use server_model::admin::facade::sys_user;

pub async fn jwt_auth_middleware(
    mut req: Request<Body>,
    next: Next,
    audience: &str,
) -> impl IntoResponse {
    let token = match req.headers().typed_get::<Authorization<Bearer>>() {
        Some(auth) => auth.token().to_string(),
        None => {
            return Res::<String>::new_error(
                code::CODE_PERMISSION_CASBIN_DENY,
                "No token provided or invalid token type",
            )
            .into_response();
        },
    };

    match JwtUtils::validate_token(&token, audience).await {
        Ok(data) => {
            let claims = data.claims;
            let user = User::from(claims);

            // FR-028: 軟刪 user 檢查 — 持舊 JWT 的軟刪 user 應返 8888 觸發 base 端 immediate logout
            let db_arc = match GLOBAL_PRIMARY_DB.read().await.as_ref().cloned() {
                Some(db) => db,
                None => {
                    return Res::<String>::new_error(
                        code::CODE_SERVER_DB_ERROR,
                        "DB connection not available",
                    )
                    .into_response();
                },
            };

            let active_user_check = sys_user::find_active()
                .filter(sys_user::Column::Id.eq(user.user_id()))
                .one(db_arc.as_ref())
                .await;

            match active_user_check {
                Ok(Some(_)) => { /* fall through to existing path */ },
                Ok(None) => {
                    return Res::<String>::new_error(
                        code::CODE_LOGOUT_SESSION_INVALIDATED,
                        "session invalidated: user no longer active",
                    )
                    .into_response();
                },
                Err(e) => {
                    return Res::<String>::new_error(
                        code::CODE_SERVER_DB_ERROR,
                        &format!("user lookup failed: {}", e),
                    )
                    .into_response();
                },
            }

            let vals = CasbinVals {
                subject: user.subject(),
                domain: Option::from(user.domain()),
            };
            req.extensions_mut().insert(user);
            req.extensions_mut().insert(vals);
            next.run(req).await.into_response()
        },
        Err(err) => AppError::from(err).into_response(),
    }
}
