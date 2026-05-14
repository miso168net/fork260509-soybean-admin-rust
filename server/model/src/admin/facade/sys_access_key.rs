//! sys_access_key facade — 範式 A（非樹狀 entity）
//!
//! 故意不 re-export Entity，封死 service code 走 Entity::find() / Entity::delete_*() 路徑。
//! 4 個 bare function 是 service 入口；UPDATE / INSERT 仍走 ActiveModel。

use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Select, TransactionTrait};
use server_core::db::soft_delete::SoftDeletable;
use server_core::web::{
    audit::{Actor, AuditEvent, AuditOperation, AuditSource},
    code,
    error::AppError,
};

use crate::admin::audit_log;
use crate::admin::audit_serialize::audit_snapshot;
use crate::admin::entities::sys_access_key as _entity;

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

    // F2.1: fetch before snapshot（active row state）
    let before = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch before failed: {}", e),
        })?;

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
            entity_type: "sys_access_key",
            entity_id: id.clone(),
            payload_before: before.as_ref().map(audit_snapshot),
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

    // F2.1: fetch before（軟刪態 snapshot）
    let before = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_not_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch before failed: {}", e),
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

    // F2.1: fetch after（active 態 snapshot）
    let after = _entity::Entity::find()
        .filter(_entity::Column::Id.eq(&id))
        .filter(_entity::Column::DeletedAt.is_null())
        .one(&txn)
        .await
        .map_err(|e| AppError {
            code: code::CODE_SERVER_DB_ERROR,
            message: format!("fetch after failed: {}", e),
        })?;

    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor,
            operation: AuditOperation::Restore,
            entity_type: "sys_access_key",
            entity_id: id.clone(),
            payload_before: before.as_ref().map(audit_snapshot),
            payload_after: after.as_ref().map(audit_snapshot),
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
