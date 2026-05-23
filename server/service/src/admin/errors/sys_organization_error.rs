use server_core::web::code;
use server_core::web::error::{ApiError, AppError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OrganizationError {
    #[error("Organization not found")]
    OrganizationNotFound,
}

impl ApiError for OrganizationError {
    fn code(&self) -> u16 {
        match self {
            OrganizationError::OrganizationNotFound => code::CODE_BUSINESS_ENTITY_NOT_FOUND,
        }
    }

    fn message(&self) -> String {
        format!("{}", self)
    }
}

impl From<OrganizationError> for AppError {
    fn from(err: OrganizationError) -> Self {
        AppError {
            code: err.code(),
            message: err.message(),
        }
    }
}
