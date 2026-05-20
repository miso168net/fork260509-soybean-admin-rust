use std::error::Error;

use axum_casbin::CasbinAxumLayer;
use casbin::DefaultModel;
use sea_orm::Database;
use sea_orm_adapter::SeaOrmAdapter;

use crate::{project_info, spawn_casbin_sync_subscriber};

pub async fn initialize_casbin(
    model_path: &str,
    db_url: &str,
) -> Result<CasbinAxumLayer, Box<dyn Error>> {
    project_info!("Initializing Casbin with model: {}", model_path);
    let model = DefaultModel::from_file(model_path).await?;
    let db = Database::connect(db_url).await?;
    let adapter = SeaOrmAdapter::new(db).await?;

    let mut casbin_axum_layer = CasbinAxumLayer::new(model, adapter).await?;

    // 起 Casbin pub-sub subscriber 背景 task,與 axum casbin layer 共用同一個
    // `Arc<RwLock<CachedEnforcer>>`(per research R-Q1 — 不新增全域)。
    spawn_casbin_sync_subscriber(casbin_axum_layer.get_enforcer());

    project_info!("Casbin initialization completed successfully");
    Ok(casbin_axum_layer)
}
