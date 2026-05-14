//! F2.1 HTTP middleware audit building block — AuditEvent { source: Http {...} }
//! + AuditSource::Internal vs Http method 區分。
//!
//! 對應 spec.md Acceptance Scenario 5 + Q2 Hybrid entity_type building block。
//!
//! 注：HTTP middleware 整合 e2e 在 quickstart Step 7、本檔 unit-test 級驗 building block。
//!     Hybrid entity_type rule（從 url 推導 entity_type）邏輯在 sys_operation_log_service
//!     handle_operation_log_event 內、屬 service-level test，不在 G6 範圍。
//!
//! 跑前置：見 audit_basics.rs header。

mod common;

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use server_core::web::audit::{AuditEvent, AuditOperation, AuditSource};
use server_model::admin::audit_log;
use server_model::admin::entities::sys_operation_log;
use ulid::Ulid;

use common::{connect, test_actor};

#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn http_audit_event_writes_with_method_url_ip_useragent_filled() {
    let db = connect().await;
    let actor = test_actor("g6_http");
    let entity_id = format!("u-{}", Ulid::new());

    let txn = db.begin().await.unwrap();
    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor: &actor,
            operation: AuditOperation::Insert,
            entity_type: "sys_user",
            entity_id: entity_id.clone(),
            payload_before: None,
            payload_after: None, // HTTP middleware 視角無 SQL-level snapshot
            description: Some("HTTP POST /api/sys-user".to_string()),
            source: AuditSource::Http {
                method: "POST".to_string(),
                url: "/api/sys-user".to_string(),
                ip: "10.0.0.5".to_string(),
                user_agent: Some("test/1.0".to_string()),
            },
            request_id: Some("req-123".to_string()),
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    let audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&entity_id))
        .one(db.as_ref())
        .await
        .unwrap()
        .expect("audit row should exist");
    assert_eq!(audit.operation, "INSERT");
    assert_eq!(audit.method, "POST"); // HTTP method
    assert_eq!(audit.url, "/api/sys-user");
    assert_eq!(audit.ip, "10.0.0.5");
    assert_eq!(audit.user_agent.as_deref(), Some("test/1.0"));
    assert_eq!(audit.module_name, "sys_user");
    assert_eq!(audit.request_id, "req-123");

    let _ = sys_operation_log::Entity::delete_by_id(audit.id)
        .exec(db.as_ref())
        .await;
}

// AuditSource::Internal vs Http 驗證 method 欄不同
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn http_source_writes_post_method_internal_writes_internal() {
    let db = connect().await;
    let actor = test_actor("g6_source");
    let internal_id = format!("int-{}", Ulid::new());
    let http_id = format!("http-{}", Ulid::new());

    let txn = db.begin().await.unwrap();
    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor: &actor,
            operation: AuditOperation::Insert,
            entity_type: "sys_user",
            entity_id: internal_id.clone(),
            payload_before: None,
            payload_after: None,
            description: None,
            request_id: None,
            source: AuditSource::Internal,
        },
    )
    .await
    .unwrap();
    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor: &actor,
            operation: AuditOperation::Insert,
            entity_type: "sys_user",
            entity_id: http_id.clone(),
            payload_before: None,
            payload_after: None,
            description: None,
            request_id: None,
            source: AuditSource::Http {
                method: "POST".to_string(),
                url: "/api/sys-user".to_string(),
                ip: "127.0.0.1".to_string(),
                user_agent: None,
            },
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    let int_audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&internal_id))
        .one(db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(int_audit.method, "INTERNAL");

    let http_audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&http_id))
        .one(db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(http_audit.method, "POST");

    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::EntityId.is_in([internal_id, http_id]))
        .exec(db.as_ref())
        .await;
}
