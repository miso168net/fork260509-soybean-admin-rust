use serde::Deserialize;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct LoginInput {
    #[validate(length(min = 5, message = "Username cannot be empty"))]
    pub identifier: String,
    #[validate(length(min = 6, message = "Password cannot be empty"))]
    pub password: String,
}

#[derive(Deserialize, Validate)]
pub struct RefreshTokenInput {
    #[serde(rename = "refreshToken")]
    #[validate(length(min = 1, message = "Refresh token cannot be empty"))]
    pub refresh_token: String,
}
