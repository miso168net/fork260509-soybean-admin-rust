use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignPermissionDto {
    #[validate(length(min = 1, message = "domain cannot be empty"))]
    pub domain: String,

    // 039: role_id 改 numeric display_id（i64），handler 先 lookup_ulid_by_display_id 後再走 service
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,

    // 039: permissions 改 endpoint display_id 陣列；length(min=1) 對 Vec 仍適用
    #[validate(length(min = 1, message = "Permissions array cannot be empty"))]
    pub permissions: Vec<i64>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignRouteDto {
    #[validate(length(min = 1, message = "domain cannot be empty"))]
    pub domain: String,

    // 039: role_id 改 numeric display_id
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,

    // 不動：route_ids = sys_menu.id（本身就是 i32 PK）
    #[validate(length(min = 1, message = "Routes array cannot be empty"))]
    pub route_ids: Vec<i32>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AssignUserDto {
    // 039: role_id 改 numeric display_id
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,

    // 039: user_ids 改 user display_id 陣列
    #[validate(length(min = 1, message = "Users array cannot be empty"))]
    pub user_ids: Vec<i64>,
}

/// W-FW8 US1: base-web button-auth-modal 端點授權專用輸入。
/// 與 `AssignPermissionDto` 差異：① 不收 domain（由 JWT actor 注入）；
/// ② `endpoint_ids` 允許空陣列（spec E-4：空陣列 = 清空 role 全部 endpoint 授權）。
#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAssignRoleEndpointsInput {
    // 039: role_id 改 numeric display_id
    #[validate(range(min = 1, message = "Role ID must be positive"))]
    pub role_id: i64,
    /// 039: endpoint_ids 改 endpoint display_id 陣列；空陣列 = 清空 role 全部 endpoint 授權（per spec E-4）
    pub endpoint_ids: Vec<i64>,
}
