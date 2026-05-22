use server_core::web::code;
use server_core::web::error::{ApiError, AppError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UserError {
    #[error("User not found")]
    UserNotFound,
    #[error("Authentication failed: Wrong password")]
    WrongPassword,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Username already exists")]
    UsernameAlreadyExists,
    #[error("Invalid user status")]
    InvalidUserStatus,
    #[error("One or more role codes are invalid")]
    InvalidRoleCode,
}

impl ApiError for UserError {
    fn code(&self) -> u16 {
        match self {
            UserError::UserNotFound => code::CODE_USER_NOT_FOUND,
            UserError::WrongPassword => code::CODE_USER_WRONG_PASSWORD,
            UserError::AuthenticationFailed => code::CODE_USER_AUTHENTICATION_FAILED,
            UserError::UsernameAlreadyExists => code::CODE_USER_USERNAME_ALREADY_EXISTS,
            UserError::InvalidUserStatus => code::CODE_USER_INVALID_STATUS,
            UserError::InvalidRoleCode => code::CODE_VALIDATION_FORMAT_INVALID,
        }
    }

    fn message(&self) -> String {
        format!("{}", self)
    }
}

impl From<UserError> for AppError {
    fn from(err: UserError) -> Self {
        AppError {
            code: err.code(),
            message: err.message(),
        }
    }
}
