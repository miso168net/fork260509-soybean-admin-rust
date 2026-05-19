//! F11 extracted-stubs: `/mock/getLastTime` stub handler。
//! per spec FR-004:回 F4 envelope `{data: {time: chrono::Utc::now().to_rfc3339()}}`。

use serde_json::json;
use server_core::web::{error::AppError, res::Res};

pub struct SysMockApi;

impl SysMockApi {
    pub async fn get_last_time() -> Result<Res<serde_json::Value>, AppError> {
        Ok(Res::new_data(json!({
            "time": chrono::Utc::now().to_rfc3339(),
        })))
    }
}
