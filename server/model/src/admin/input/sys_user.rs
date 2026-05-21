use serde::{Deserialize, Serialize};
use server_core::web::page::PageRequest;
use validator::Validate;

use crate::admin::entities::sea_orm_active_enums::{Gender, Status};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPageRequest {
    #[serde(flatten)]
    pub page_details: PageRequest,
    pub keywords: Option<String>,
    pub user_gender: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    pub domain: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Username must be between 1 and 50 characters"
    ))]
    pub username: String,
    #[validate(length(
        min = 6,
        max = 100,
        message = "Password must be between 6 and 100 characters"
    ))]
    pub password: String,
    #[validate(length(
        min = 1,
        max = 50,
        message = "Nick name must be between 1 and 50 characters"
    ))]
    pub nick_name: String,
    pub avatar: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    #[validate(length(max = 20, message = "Phone number must not exceed 20 characters"))]
    pub phone_number: Option<String>,
    pub status: Status,
    pub gender: Option<Gender>,
}

pub type CreateUserInput = UserInput;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserInput {
    pub id: String,
    pub domain: String,
    #[validate(length(min = 1, max = 50, message = "Username must be between 1 and 50 characters"))]
    pub username: String,
    #[validate(length(min = 6, max = 100, message = "Password must be between 6 and 100 characters"))]
    pub password: Option<String>,
    #[validate(length(min = 1, max = 50, message = "Nick name must be between 1 and 50 characters"))]
    pub nick_name: String,
    pub avatar: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub email: Option<String>,
    #[validate(length(max = 20, message = "Phone number must not exceed 20 characters"))]
    pub phone_number: Option<String>,
    pub status: Status,
    pub gender: Option<Gender>,
}

// F9 systemManage-alias-router: DELETE /systemManage/deleteUser body-id payload
// per F9 spec FR-018 — 不加 validator::Validate derive（沿用既有 delete_user service 自有 id 檢查）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserByBodyInput {
    pub id: String,
}

// F9 systemManage-alias-router: DELETE /systemManage/batchDeleteUser payload
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteUserInput {
    pub ids: Vec<String>,
}

// W-FW1 systemManage transform layer: base-web-shaped add/update user DTOs。
// 僅 Deserialize（無 Validate）— 必填欄位空值由 base-web 表單驗證把關（spec E-1），
// 後端轉換層只負責形狀對映。Add / Update 分為兩型：Update 多 id、語意不同，刻意不共用。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageAddUserInput {
    pub user_name: String,
    pub user_gender: Option<String>,
    pub nick_name: String,
    pub user_phone: Option<String>,
    pub user_email: Option<String>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemManageUpdateUserInput {
    pub id: String,
    pub user_name: String,
    pub user_gender: Option<String>,
    pub nick_name: String,
    pub user_phone: Option<String>,
    pub user_email: Option<String>,
    pub status: String,
}
