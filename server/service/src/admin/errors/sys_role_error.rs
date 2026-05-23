use server_core::web::error::{ApiError, AppError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RoleError {
    #[error("Role not found")]
    RoleNotFound,

    #[error("Duplicate role code")]
    DuplicateRoleCode,

    // W-FW6 role-authorization-completion (US1): home route_name 不對應任何 enabled + non-constant menu
    #[error("Home route not found")]
    HomeRouteNotFound,
}

impl ApiError for RoleError {
    fn code(&self) -> u16 {
        match self {
            RoleError::RoleNotFound => 4001,
            RoleError::DuplicateRoleCode => 4002,
            RoleError::HomeRouteNotFound => 4003,
        }
    }

    fn message(&self) -> String {
        format!("{}", self)
    }
}

impl From<RoleError> for AppError {
    fn from(err: RoleError) -> Self {
        AppError {
            code: err.code(),
            message: err.message(),
        }
    }
}
