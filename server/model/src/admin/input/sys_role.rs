use serde::{Deserialize, Serialize};
use server_core::web::page::PageRequest;
use validator::Validate;

use crate::admin::entities::sea_orm_active_enums::Status;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePageRequest {
    #[serde(flatten)]
    pub page_details: PageRequest,
    pub keywords: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RoleInput {
    pub pid: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Code must be between 1 and 50 characters"
    ))]
    pub code: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Name must be between 1 and 50 characters"
    ))]
    pub name: String,
    pub status: Status,
    #[validate(length(max = 200, message = "Description must not exceed 200 characters"))]
    pub description: Option<String>,
}

pub type CreateRoleInput = RoleInput;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleInput {
    pub id: String,
    #[serde(flatten)]
    pub role: RoleInput,
}
