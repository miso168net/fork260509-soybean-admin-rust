use axum::response::{IntoResponse, Response};
use mongodb::error::Error as MongoError;
use redis::RedisError;
use sea_orm::DbErr;

use crate::web::{code, jwt::JwtError, res::Res};

pub trait ApiError {
    fn code(&self) -> u16;
    fn message(&self) -> String;
}

#[derive(Debug)]
pub struct AppError {
    pub code: u16,
    pub message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        Res::<()>::new_error(self.code, self.message.as_str()).into_response()
    }
}

impl ApiError for AppError {
    fn code(&self) -> u16 {
        self.code
    }

    fn message(&self) -> String {
        self.message.to_string()
    }
}

/// Map a `DbErr` variant to its F4 business code.
///
/// Shared by `ApiError for DbErr` and `From<DbErr> for AppError`
/// so both impls stay in lockstep.
fn db_err_code(err: &DbErr) -> u16 {
    match err {
        DbErr::ConnectionAcquire(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::TryIntoErr { .. } => code::CODE_SERVER_DB_ERROR,
        DbErr::Conn(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Exec(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Query(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::ConvertFromU64(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::UnpackInsertId => code::CODE_SERVER_DB_ERROR,
        DbErr::UpdateGetPrimaryKey => code::CODE_SERVER_DB_ERROR,
        DbErr::RecordNotFound(_) => code::CODE_BUSINESS_ENTITY_NOT_FOUND,
        DbErr::AttrNotSet(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Custom(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Type(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Json(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::Migration(_) => code::CODE_SERVER_DB_ERROR,
        DbErr::RecordNotInserted => code::CODE_BUSINESS_STATE_CONFLICT,
        DbErr::RecordNotUpdated => code::CODE_BUSINESS_ENTITY_NOT_FOUND,
    }
}

impl ApiError for DbErr {
    fn code(&self) -> u16 {
        db_err_code(self)
    }

    fn message(&self) -> String {
        self.to_string()
    }
}

impl From<DbErr> for AppError {
    fn from(err: DbErr) -> Self {
        AppError {
            code: db_err_code(&err),
            message: err.to_string(),
        }
    }
}

impl From<JwtError> for AppError {
    fn from(err: JwtError) -> Self {
        let message = err.to_string();
        let code = match err {
            JwtError::KeysNotInitialized => code::CODE_SERVER_CONFIGURATION_ERROR,
            JwtError::ValidationNotInitialized => code::CODE_SERVER_CONFIGURATION_ERROR,
            JwtError::TokenCreationError(_) => code::CODE_SERVER_INTERNAL_ERROR,
            JwtError::TokenValidationError(_) => code::CODE_EXPIRED_TOKEN_SIGNATURE,
        };
        AppError { code, message }
    }
}

impl From<RedisError> for AppError {
    fn from(err: RedisError) -> Self {
        let message = if let Some(redis_code) = err.code() {
            format!("[{}] {}", redis_code, err)
        } else {
            err.to_string()
        };

        AppError {
            code: code::CODE_SERVER_CACHE_ERROR,
            message,
        }
    }
}

impl From<MongoError> for AppError {
    fn from(err: MongoError) -> Self {
        AppError {
            code: code::CODE_SERVER_EXTERNAL_SERVICE_ERROR,
            message: err.to_string(),
        }
    }
}
