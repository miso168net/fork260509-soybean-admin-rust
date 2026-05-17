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
