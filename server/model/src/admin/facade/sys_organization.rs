//! sys_organization facade — 範式 B（樹狀 entity、FR-026 tree-cascade）
//!
//! 故意不 re-export Entity，封死 service code 走 Entity::find() / Entity::delete_*() 路徑。
//! 4 個 bare function 是 service 入口；UPDATE / INSERT 仍走 ActiveModel。
//!
//! PK 型別為 `String`、pid 型別也為 `String`（不需 to_string 轉換）。
//! `soft_delete_by_id` 在 UPDATE 前加 active children check（per FR-026）；
//! `restore_by_id` 不加 tree-check（per FR-027 — restore 屬資料修復、不約束樹狀完整性）。

use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Select,
    TransactionTrait,
};
use server_core::db::soft_delete::SoftDeletable;
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    code,
    error::AppError,
};

use crate::admin::audit_log;
use crate::admin::entities::sys_organization as _entity;

pub use _entity::{ActiveModel, Column, Model, Relation};
// pub use _entity::Entity;  ← 故意不 re-export

pub fn find_active() -> Select<_entity::Entity> {
    <_entity::Entity as SoftDeletable>::find_active()
}
pub fn find_with_deleted() -> Select<_entity::Entity> {
    <_entity::Entity as SoftDeletable>::find_with_deleted()
}

pub async fn soft_delete_by_id<C>(db: &C, id: String, actor: &Actor) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("begin txn failed: {}", e),
    })?;

    // FR-026: 樹狀 entity active children check
    let active_children_count = _entity::Entity::find()
        .filter(_entity::Column::Pid.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .count(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("count active children failed: {}", e),
        })?;
    if active_children_count > 0 {
        return Err(AppError {
            code: code::CODE_BUSINESS_STATE_CONFLICT,
            message: format!(
                "cannot delete: {} active children exist",
                active_children_count
            ),
        });
    }

    let res = _entity::Entity::update_many()
        .col_expr(_entity::Column::DeletedAt, Expr::current_timestamp().into())
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .exec(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("soft delete failed: {}", e),
        })?;

    if res.rows_affected == 0 {
        return Err(AppError {
            code: code::CODE_BUSINESS_ENTITY_NOT_FOUND,
            message: format!("entity not found or already deleted: id={}", id),
        });
    }

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation: AuditOperation::SoftDelete,
            entity_type: "sys_organization",
            entity_id: id.clone(),
            payload_before: None,
            payload_after: None,
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await?;

    txn.commit().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("commit failed: {}", e),
    })?;
    Ok(())
}

pub async fn restore_by_id<C>(db: &C, id: String, actor: &Actor) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("begin txn failed: {}", e),
    })?;

    let res = _entity::Entity::update_many()
        .col_expr(
            _entity::Column::DeletedAt,
            Expr::value(None::<chrono::NaiveDateTime>),
        )
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_not_null())
        .exec(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("restore failed: {}", e),
        })?;

    if res.rows_affected == 0 {
        return Err(AppError {
            code: code::CODE_BUSINESS_ENTITY_NOT_FOUND,
            message: format!("entity not found or already active: id={}", id),
        });
    }

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation: AuditOperation::Restore,
            entity_type: "sys_organization",
            entity_id: id.clone(),
            payload_before: None,
            payload_after: None,
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await?;

    txn.commit().await.map_err(|e| AppError {
        code: code::CODE_SERVER_DB_ERROR,
        message: format!("commit failed: {}", e),
    })?;
    Ok(())
}
