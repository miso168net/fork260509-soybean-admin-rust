use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use server_constant::definition::consts::TokenStatus;
use server_core::web::error::AppError;
use server_model::admin::entities::sys_tokens::ActiveModel as SysTokensActiveModel;
use ulid::Ulid;

pub struct AccessTokenEvent {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub username: String,
    pub domain: String,
    pub ip: String,
    pub port: Option<i32>,
    pub address: String,
    pub user_agent: String,
    pub request_id: String,
    pub login_type: String,
    pub expires_at: NaiveDateTime,
}

impl AccessTokenEvent {
    pub async fn handle<C: ConnectionTrait>(self, db: &C) -> Result<(), AppError> {
        let now = chrono::Local::now().naive_local();

        SysTokensActiveModel {
            id: Set(Ulid::new().to_string()),
            access_token: Set(self.access_token),
            refresh_token: Set(self.refresh_token),
            status: Set(TokenStatus::Active.to_string()),
            user_id: Set(self.user_id),
            username: Set(self.username.clone()),
            domain: Set(self.domain),
            login_time: Set(now),
            ip: Set(self.ip),
            port: Set(self.port),
            address: Set(self.address),
            user_agent: Set(self.user_agent),
            request_id: Set(self.request_id),
            r#type: Set(self.login_type),
            expires_at: Set(self.expires_at),
            created_at: Set(now),
            created_by: Set(self.username),
        }
        .insert(db)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }
}
