use std::any::Any;

use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    error::AppError,
    page::PaginatedData,
};
use server_global::{global::OperationLogContext, project_error};
use server_model::admin::{
    audit_log,
    entities::{
        prelude::SysOperationLog,
        sys_operation_log::{Column as SysOperationLogColumn, Model as SysOperationLogModel},
    },
    input::OperationLogPageRequest,
};
use tracing::instrument;

use crate::helper::db_helper;

#[async_trait]
pub trait TOperationLogService {
    async fn find_paginated_operation_logs(
        &self,
        params: OperationLogPageRequest,
    ) -> Result<PaginatedData<SysOperationLogModel>, AppError>;

    /// HTTP middleware audit event handler — F2.1 起改透過 `audit_log::write_in_txn`
    /// 統一寫入路徑（per spec FR-014/015 + data-model.md §E9）。
    ///
    /// 此方法為 F2.1 內部 forwarder：event-channel subscriber 收到 `OperationLogContext`
    /// 後仍呼叫此 method、由 method body 構造 `AuditEvent` 走統一 audit 入口。
    /// F2.2 outbox 階段可進一步移除 event-channel architecture、把 audit 寫入內聯
    /// 進 middleware response 後處理。
    #[deprecated(
        since = "F2.1",
        note = "HTTP audit 已透過 audit_log::write_in_txn unified path、此 method 視為 F2.1 內部 forwarder、F2.2 outbox 階段可進一步重構"
    )]
    async fn handle_operation_log_event(event: &OperationLogContext) -> Result<(), AppError>;
}

pub struct SysOperationLogService;

#[async_trait]
impl TOperationLogService for SysOperationLogService {
    async fn find_paginated_operation_logs(
        &self,
        params: OperationLogPageRequest,
    ) -> Result<PaginatedData<SysOperationLogModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = SysOperationLog::find();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any()
                .add(SysOperationLogColumn::Domain.contains(keywords))
                .add(SysOperationLogColumn::Username.contains(keywords))
                .add(SysOperationLogColumn::Ip.contains(keywords))
                .add(SysOperationLogColumn::UserAgent.contains(keywords));
            query = query.filter(condition);
        }

        query = query.order_by_desc(SysOperationLogColumn::CreatedAt);

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

    async fn handle_operation_log_event(event: &OperationLogContext) -> Result<(), AppError> {
        // GET / HEAD / OPTIONS / TRACE 等 read path 不寫 audit
        // (per spec FR-015 + Constitution §II + clarify Q1)
        let operation = match event.method.to_uppercase().as_str() {
            "POST" => AuditOperation::Insert,
            "PUT" | "PATCH" => AuditOperation::Update,
            "DELETE" => AuditOperation::SoftDelete,
            _ => return Ok(()),
        };

        // Hybrid entity_type rule (per spec FR-015 + clarify Q2):
        // 7 條 admin URL prefix → 對應 sys_<x>；不 match → "http_event"
        let url = event.url.as_str();
        let entity_type: &'static str = if url.starts_with("/api/sys-user") {
            "sys_user"
        } else if url.starts_with("/api/sys-role") {
            "sys_role"
        } else if url.starts_with("/api/sys-menu") {
            "sys_menu"
        } else if url.starts_with("/api/sys-domain") {
            "sys_domain"
        } else if url.starts_with("/api/sys-organization") {
            "sys_organization"
        } else if url.starts_with("/api/sys-endpoint") {
            "sys_endpoint"
        } else if url.starts_with("/api/sys-access-key") {
            "sys_access_key"
        } else {
            "http_event"
        };

        // entity_id 從 URL path 第 3 段抽取（best-effort、可能空字串）
        // 例：/api/sys-user/123 → split('/') = ["", "api", "sys-user", "123"]，nth(3) = "123"
        let entity_id = url
            .split('?')
            .next()
            .unwrap_or(url)
            .split('/')
            .nth(3)
            .unwrap_or("")
            .to_string();

        let actor = Actor {
            id: event.user_id.clone().unwrap_or_default(),
            username: event.username.clone().unwrap_or_default(),
            domain: event.domain.clone().unwrap_or_default(),
        };

        let audit_event = AuditEvent {
            actor: &actor,
            operation,
            entity_type,
            entity_id,
            payload_before: None, // middleware 看不到 SQL-level snapshot
            payload_after: None,
            description: Some(format!("HTTP {} {}", event.method, event.url)),
            source: AuditSource::Http {
                method: event.method.clone(),
                url: event.url.clone(),
                ip: event.ip.clone(),
                user_agent: event.user_agent.clone(),
            },
            request_id: Some(event.request_id.clone()),
        };

        // middleware 是 post-execution hook、業務已 commit、此處自行 wrap txn。
        // 任何失敗一律 tracing::warn! + 不返 error
        // (per spec edge case「跨資源 side effect 失敗不影響 audit log 主要正確性」+ clarify Q1)。
        let db = db_helper::get_db_connection().await?;
        let txn = match db.begin().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    target: "operation_log_middleware",
                    error = ?e,
                    "begin txn failed for HTTP audit"
                );
                return Ok(());
            }
        };

        if let Err(e) = audit_log::write_in_txn(&txn, audit_event).await {
            tracing::warn!(
                target: "operation_log_middleware",
                error = ?e,
                "audit insert failed for HTTP audit"
            );
            let _ = txn.rollback().await;
            return Ok(());
        }

        if let Err(e) = txn.commit().await {
            tracing::warn!(
                target: "operation_log_middleware",
                error = ?e,
                "commit failed for HTTP audit"
            );
            return Ok(());
        }

        Ok(())
    }
}

#[instrument(skip(rx))]
pub async fn sys_operation_log_listener(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Box<dyn Any + Send>>,
) {
    while let Some(event) = rx.recv().await {
        if let Some(operation_log_context) = event.downcast_ref::<OperationLogContext>() {
            // F2.1: handle_operation_log_event 已標 #[deprecated]、仍為 F2.1 內部 forwarder。
            // 此 callsite 允許 deprecated 呼叫，F2.2 outbox 階段重構後此 allow 連同 method 一併移除。
            #[allow(deprecated)]
            let result =
                SysOperationLogService::handle_operation_log_event(operation_log_context).await;
            if let Err(e) = result {
                project_error!("Failed to handle operation log event: {:?}", e);
            }
        } else {
            project_error!("Received unknown event type in operation log listener");
        }
    }
}
