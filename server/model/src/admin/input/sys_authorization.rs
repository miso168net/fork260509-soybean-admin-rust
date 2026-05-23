use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignPermissionDto {
    #[validate(length(min = 1, message = "domain cannot be empty"))]
    pub domain: String,

    #[validate(length(min = 1, message = "Role ID cannot be empty"))]
    pub role_id: String,

    #[validate(length(min = 1, message = "Permissions array cannot be empty"))]
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignRouteDto {
    #[validate(length(min = 1, message = "domain cannot be empty"))]
    pub domain: String,

    #[validate(length(min = 1, message = "Role ID cannot be empty"))]
    pub role_id: String,

    #[validate(length(min = 1, message = "Routes array cannot be empty"))]
    pub route_ids: Vec<i32>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignUserDto {
    #[validate(length(min = 1, message = "Role ID cannot be empty"))]
    pub role_id: String,

    #[validate(length(min = 1, message = "Users array cannot be empty"))]
    pub user_ids: Vec<String>,
}

/// W-FW8 US1: base-web button-auth-modal 端點授權專用輸入。
/// 與 `AssignPermissionDto` 差異：① 不收 domain（由 JWT actor 注入）；
/// ② `endpoint_ids` 允許空陣列（spec E-4：空陣列 = 清空 role 全部 endpoint 授權）。
#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAssignRoleEndpointsInput {
    #[validate(length(min = 1, message = "Role ID cannot be empty"))]
    pub role_id: String,
    /// 空陣列 = 清空 role 全部 endpoint 授權（per spec E-4）
    pub endpoint_ids: Vec<String>,
}
