use server_core::web::code;
use server_core::web::error::{ApiError, AppError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EndpointError {
    #[error("Endpoint not found")]
    EndpointNotFound,
}

impl ApiError for EndpointError {
    fn code(&self) -> u16 {
        match self {
            EndpointError::EndpointNotFound => code::CODE_BUSINESS_ENTITY_NOT_FOUND,
        }
    }

    fn message(&self) -> String {
        format!("{}", self)
    }
}

impl From<EndpointError> for AppError {
    fn from(err: EndpointError) -> Self {
        AppError {
            code: err.code(),
            message: err.message(),
        }
    }
}
