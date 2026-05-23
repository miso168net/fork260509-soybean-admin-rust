use std::any::Any;

use async_trait::async_trait;
use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, PaginatorTrait, QueryFilter,
    Set, TransactionTrait,
};
use server_core::{
    sign::{ApiKeyEvent, ValidatorType},
    web::{
        audit::{Actor, AuditEvent, AuditOperation, AuditSource},
        error::AppError,
        page::PaginatedData,
    },
};
use server_global::{project_info, snowflake};
use server_model::admin::{
    audit_log,
    audit_serialize::audit_snapshot,
    facade::sys_access_key::{
        self, ActiveModel as SysAccessKeyActiveModel, Column as SysAccessKeyColumn,
        Model as SysAccessKeyModel,
    },
    input::{AccessKeyPageRequest, CreateAccessKeyInput},
};
use tracing::instrument;
use ulid::Ulid;

use crate::helper::db_helper;

use super::sys_access_key_error::AccessKeyError;

#[async_trait]
pub trait TAccessKeyService {
    async fn find_paginated_access_keys(
        &self,
        params: AccessKeyPageRequest,
    ) -> Result<PaginatedData<SysAccessKeyModel>, AppError>;
    async fn create_access_key(
        &self,
        input: CreateAccessKeyInput,
        actor: &Actor,
    ) -> Result<SysAccessKeyModel, AppError>;
    async fn delete_access_key(&self, id: &str, actor: &Actor) -> Result<(), AppError>;

    async fn initialize_access_key(&self) -> Result<(), AppError>;

    /// 039 rust-entity-id-numeric-migration C3: by-display_id lookup helper。
    /// base-web 對外傳 numeric display_id；rust 內部 PK/FK 仍走 ULID 字串。
    /// handler 收 Path<i64> 後第一步透過本方法解析回 ULID，再走後續 service 既有路徑。
    /// 軟刪資料不可解析（find_active() filter DeletedAt.is_null）。
    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError>;
}

#[derive(Clone)]
pub struct SysAccessKeyService;

impl SysAccessKeyService {
    async fn create_access_key_in_transaction(
        &self,
        txn: &DatabaseTransaction,
        access_key: SysAccessKeyActiveModel,
        actor: &Actor,
    ) -> Result<SysAccessKeyModel, AppError> {
        let result = access_key.insert(txn).await.map_err(AppError::from)?;

        audit_log::write_in_txn(
            txn,
            AuditEvent {
                actor,
                operation: AuditOperation::Insert,
                entity_type: "sys_access_key",
                entity_id: result.id.clone(),
                payload_before: None,
                payload_after: Some(audit_snapshot(&result)),
                description: None,
                source: AuditSource::Internal,
                request_id: None,
            },
        )
        .await?;

        // 添加到验证器
        server_core::sign::add_key(ValidatorType::Simple, &result.access_key_id, None).await;
        server_core::sign::add_key(
            ValidatorType::Complex,
            &result.access_key_id,
            Some(&result.access_key_secret),
        )
        .await;

        Ok(result)
    }

}

#[async_trait]
impl TAccessKeyService for SysAccessKeyService {
    async fn find_paginated_access_keys(
        &self,
        params: AccessKeyPageRequest,
    ) -> Result<PaginatedData<SysAccessKeyModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_access_key::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any().add(SysAccessKeyColumn::Domain.contains(keywords));
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

    async fn create_access_key(
        &self,
        input: CreateAccessKeyInput,
        actor: &Actor,
    ) -> Result<SysAccessKeyModel, AppError> {
        let db = db_helper::get_db_connection().await?;
        let txn = db.begin().await.map_err(AppError::from)?;

        let access_key_id = format!("AK{}", Ulid::new().to_string());
        let access_key_secret = format!("SK{}", Ulid::new().to_string());

        let access_key = SysAccessKeyActiveModel {
            id: Set(Ulid::new().to_string()),
            display_id: Set(snowflake::next_display_id()),
            domain: Set(input.domain),
            status: Set(input.status),
            description: Set(input.description),
            access_key_id: Set(access_key_id),
            access_key_secret: Set(access_key_secret),
            created_at: Set(Local::now().naive_local()),
            created_by: Set("TODO".to_string()),
            ..Default::default()
        };

        let result = match self
            .create_access_key_in_transaction(&txn, access_key, actor)
            .await
        {
            Ok(result) => {
                txn.commit().await.map_err(AppError::from)?;
                result
            },
            Err(e) => {
                txn.rollback().await.map_err(AppError::from)?;
                return Err(e);
            },
        };

        Ok(result)
    }

    async fn delete_access_key(&self, id: &str, actor: &Actor) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;

        // 先获取 access key 信息（用於後續從 validator 移除）
        let access_key = sys_access_key::find_active()
            .filter(SysAccessKeyColumn::Id.eq(id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(AccessKeyError::AccessKeyNotFound))?;

        // soft delete（facade 內部開 txn、同時寫 audit log）。
        //
        // NOTE: facade commit 後到 validator remove_key 兩行間若 process 崩潰、validator
        // 在 in-memory 仍保有該 key 至下次 process 重啟 / initialize_access_keys 再次同步。
        // 既有 hard-delete 版本同 txn 內呼 remove_key 也只能避免 DB 改了但記憶體沒清的窗口、
        // 改善需把 validator 改為 DB-as-truth 模式（initialize 期間 reload）— 留 F12+。
        sys_access_key::soft_delete_by_id(db.as_ref(), id.to_string(), actor).await?;

        // 从验证器中移除
        server_core::sign::remove_key(ValidatorType::Simple, &access_key.access_key_id).await;
        server_core::sign::remove_key(ValidatorType::Complex, &access_key.access_key_id).await;

        Ok(())
    }

    async fn initialize_access_key(&self) -> Result<(), AppError> {
        let db = db_helper::get_db_connection().await?;

        let access_keys = sys_access_key::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        for access_key in access_keys {
            server_core::sign::add_key(ValidatorType::Simple, &access_key.access_key_id, None)
                .await;
            server_core::sign::add_key(
                ValidatorType::Complex,
                &access_key.access_key_id,
                Some(&access_key.access_key_secret),
            )
            .await;
        }

        Ok(())
    }

    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError> {
        let db = db_helper::get_db_connection().await?;
        let key = sys_access_key::find_active()
            .filter(SysAccessKeyColumn::DisplayId.eq(display_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(AccessKeyError::AccessKeyNotFound))?;
        Ok(key.id)
    }
}

#[instrument(skip(rx))]
pub async fn api_key_validate_listener(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>,
) {
    while let Some(event) = rx.recv().await {
        if let Some(api_key_event) = event.downcast_ref::<ApiKeyEvent>() {
            project_info!("API key validated: {:?}", api_key_event);
        }
    }
}
