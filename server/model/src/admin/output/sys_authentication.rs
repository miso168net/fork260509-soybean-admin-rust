use serde::Serialize;

use super::MenuRoute;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthOutput {
    pub token: String,
    // 为了复用soybean-admin-nestjs前端,暂时弃用
    // pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoOutput {
    pub user_id: String,
    pub user_name: String,
    pub roles: Vec<String>,
    /// F4 階段預設輸出空陣列；button-level RBAC 內容由 F7+ feature 填入。
    pub buttons: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRoute {
    pub routes: Vec<MenuRoute>,
    pub home: String,
}
