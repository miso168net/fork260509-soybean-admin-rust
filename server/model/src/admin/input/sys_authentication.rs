use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct LoginInput {
    #[validate(length(min = 5, message = "Username cannot be empty"))]
    #[serde(alias = "userName")]
    pub identifier: String,
    #[validate(length(min = 6, message = "Password cannot be empty"))]
    pub password: String,
}

// F11 extracted-stubs: 3 個 stub DTO(per spec FR-001 / FR-002 / FR-003、FR-015 不加 validator）
#[derive(Debug, Deserialize)]
pub struct SendCaptchaInput {
    pub phone: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCaptchaInput {
    pub phone: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthErrorQuery {
    pub code: Option<String>,
    pub msg: Option<String>,
}
