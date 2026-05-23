use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, IntoActiveModel, PaginatorTrait,
    QueryFilter, Set, TransactionTrait,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    facade::sys_endpoint::{
        self, ActiveModel as SysEndpointActiveModel, Column as SysEndpointColumn,
        Model as SysEndpointModel,
    },
    input::EndpointPageRequest,
    output::EndpointTree,
};

use super::sys_endpoint_error::EndpointError;
use crate::helper::db_helper;

/// 039 EndpointTree synthetic group id：controller 名稱 hash → 負 i64 sentinel。
/// 前端 button-auth-modal 只用 leaf endpoint display_id，group 節點 id 僅作為 NTree key 區分。
fn synthetic_group_id(controller: &str) -> i64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    controller.hash(&mut hasher);
    // 取低 63 bit 後加負號 → 永遠 < 0、區隔 Snowflake-generated display_id（永遠 > 0）
    let h = hasher.finish() & 0x7FFF_FFFF_FFFF_FFFF;
    -(h as i64) - 1
}

#[async_trait]
pub trait TEndpointService {
    async fn sync_endpoints(&self, endpoints: Vec<SysEndpointModel>) -> Result<(), AppError>;
    async fn find_paginated_endpoints(
        &self,
        params: EndpointPageRequest,
    ) -> Result<PaginatedData<SysEndpointModel>, AppError>;

    async fn tree_endpoint(&self) -> Result<Vec<EndpointTree>, AppError>;

    /// 039 rust-entity-id-numeric-migration C3: by-display_id lookup helper。
    /// base-web 對外傳 numeric display_id；rust 內部 PK/FK 仍走 ULID 字串。
    /// handler 收 Path<i64> 後第一步透過本方法解析回 ULID，再走後續 service 既有路徑。
    /// 軟刪資料不可解析（find_active() filter DeletedAt.is_null）。
    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError>;
}

pub struct SysEndpointService;

impl SysEndpointService {
    /// F2.1 N-row 策略（per spec Edge Case + clarify Q3-extra）：sync 內每筆 endpoint
    /// 變動寫 1 個對應 audit row、不 batch summary。原 batch insert_many.on_conflict
    /// upsert 改 per-entity INSERT/UPDATE loop、犧牲性能換 audit 粒度。
    async fn upsert_endpoint_with_audit(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        endpoint: SysEndpointModel,
        actor: &Actor,
    ) -> Result<(), AppError> {
        let now = Local::now().naive_local();
        let existing = sys_endpoint::find_active()
            .filter(SysEndpointColumn::Id.eq(&endpoint.id))
            .one(txn)
            .await
            .map_err(AppError::from)?;

        if let Some(existing_row) = existing {
            let mut am: SysEndpointActiveModel = existing_row.clone().into_active_model();
            am.path = Set(endpoint.path);
            am.method = Set(endpoint.method);
            am.action = Set(endpoint.action);
            am.resource = Set(endpoint.resource);
            am.controller = Set(endpoint.controller);
            am.summary = Set(endpoint.summary);
            am.updated_at = Set(Some(now));

            let updated = am.update(txn).await.map_err(AppError::from)?;

            audit_log::write_in_txn(
                txn,
                AuditEvent {
                    actor,
                    operation: AuditOperation::Update,
                    entity_type: "sys_endpoint",
                    entity_id: updated.id.clone(),
                    payload_before: Some(audit_snapshot(&existing_row)),
                    payload_after: Some(audit_snapshot(&updated)),
                    description: None,
                    source: AuditSource::Internal,
                    request_id: None,
                },
            )
            .await?;
        } else {
            let am: SysEndpointActiveModel = endpoint.into_active_model();
            let new_row = am.insert(txn).await.map_err(AppError::from)?;

            audit_log::write_in_txn(
                txn,
                AuditEvent {
                    actor,
                    operation: AuditOperation::Insert,
                    entity_type: "sys_endpoint",
                    entity_id: new_row.id.clone(),
                    payload_before: None,
                    payload_after: Some(audit_snapshot(&new_row)),
                    description: None,
                    source: AuditSource::Internal,
                    request_id: None,
                },
            )
            .await?;
        }

        Ok(())
    }

    async fn batch_remove_endpoints(
        &self,
        db: &DatabaseConnection,
        endpoints_to_remove: Vec<String>,
    ) -> Result<(), AppError> {
        // endpoint_sync 是 periodic job、採 log-and-continue：個別 endpoint 軟刪失敗
        // （含已被前次 sync 軟刪導致返 6001）不阻斷後續 ID 處理，下次 sync 自然 retry。
        let actor = Actor::system("endpoint_sync");
        for id in endpoints_to_remove {
            if let Err(e) = sys_endpoint::soft_delete_by_id(db, id.clone(), &actor).await {
                tracing::warn!(target: "endpoint_sync", id = %id, error = ?e, "soft_delete failed");
            }
        }
        Ok(())
    }

    fn create_endpoint_tree(&self, endpoints: &[SysEndpointModel]) -> Vec<EndpointTree> {
        let mut controller_map: BTreeMap<String, EndpointTree> = BTreeMap::new();

        for endpoint in endpoints {
            let controller = endpoint.controller.clone();

            let controller_node =
                controller_map
                    .entry(controller.clone())
                    .or_insert(EndpointTree {
                        // 039: 合成 controller group 節點無 endpoint display_id；
                        // 取 controller 名稱 hash 後映為負 i64 sentinel（前端只用 leaf 節點 id）
                        id: synthetic_group_id(&controller),
                        path: String::new(),
                        method: String::new(),
                        action: String::new(),
                        resource: String::new(),
                        controller: controller.clone(),
                        summary: None,
                        children: Some(Vec::new()),
                    });

            if let Some(children) = &mut controller_node.children {
                children.push(EndpointTree {
                    id: endpoint.display_id,
                    path: endpoint.path.clone(),
                    method: endpoint.method.clone(),
                    action: endpoint.action.clone(),
                    resource: endpoint.resource.clone(),
                    controller: endpoint.controller.clone(),
                    summary: endpoint.summary.clone(),
                    children: Some(Vec::new()),
                });
            }
        }

        controller_map.into_values().collect()
    }
}

#[async_trait]
impl TEndpointService for SysEndpointService {
    async fn sync_endpoints(&self, new_endpoints: Vec<SysEndpointModel>) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;
        let actor = Actor::system("endpoint_sync");

        // 获取数据库中现有的所有端点
        let existing_endpoints = sys_endpoint::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        // F2.1: per-entity upsert + audit（N-row 策略、取代既有 batch insert_many.on_conflict）
        let txn = db.begin().await.map_err(AppError::from)?;
        for endpoint in new_endpoints.iter() {
            self.upsert_endpoint_with_audit(&txn, endpoint.clone(), &actor)
                .await?;
        }
        txn.commit().await.map_err(AppError::from)?;

        // 只有在数据库中已经存在端点的情况下才执行删除操作
        if !existing_endpoints.is_empty() {
            // 找出需要删除的端点
            let endpoints_to_remove: Vec<String> = existing_endpoints
                .iter()
                .filter(|existing_endpoint| {
                    !new_endpoints.iter().any(|e| {
                        e.path == existing_endpoint.path && e.method == existing_endpoint.method
                    })
                })
                .map(|endpoint| endpoint.id.clone())
                .collect();

            // 批量删除不再存在的端点（facade soft_delete_by_id 內部已 audit）
            if !endpoints_to_remove.is_empty() {
                self.batch_remove_endpoints(db.as_ref(), endpoints_to_remove)
                    .await?;
            }
        }

        Ok(())
    }

    async fn find_paginated_endpoints(
        &self,
        params: EndpointPageRequest,
    ) -> Result<PaginatedData<SysEndpointModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_endpoint::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any()
                .add(SysEndpointColumn::Path.contains(keywords))
                .add(SysEndpointColumn::Method.contains(keywords))
                .add(SysEndpointColumn::Controller.contains(keywords));
            query = query.filter(condition);
        }

        let total = query
            .clone()
            .count(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let paginator = query.paginate(db.as_ref(), params.page_details.size);
        let records = paginator
            .fetch_page(params.page_details.current - 1)
            .await
            .map_err(AppError::from)?;

        Ok(PaginatedData {
            current: params.page_details.current,
            size: params.page_details.size,
            total,
            records,
        })
    }

    async fn tree_endpoint(&self) -> Result<Vec<EndpointTree>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let endpoints = sys_endpoint::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        Ok(self.create_endpoint_tree(&endpoints))
    }

    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError> {
        let db = db_helper::get_db_connection().await?;
        let endpoint = sys_endpoint::find_active()
            .filter(SysEndpointColumn::DisplayId.eq(display_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(EndpointError::EndpointNotFound))?;
        Ok(endpoint.id)
    }
}
