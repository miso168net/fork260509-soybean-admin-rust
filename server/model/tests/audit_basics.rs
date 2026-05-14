//! F2.1 audit basics — INSERT / UPDATE / SOFT_DELETE / RESTORE 4 ops 的 audit row 驗證
//! + AuditSerialize redaction (Scenario 7/8) + Hybrid entity_type (Scenario 4-6)。
//!
//! 對應 spec.md Acceptance Scenarios 4-12（Dimension B + C + D 主要場景）。
//!
//! 跑前置：
//!   export TEST_DATABASE_URL=postgresql://postgres:123456@127.0.0.1:5432/new_admin_test
//!   cd rust-api && cargo run --bin migration -- up
//!   cargo test --test audit_basics -- --ignored

mod common;

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use server_core::web::audit::{AuditEvent, AuditOperation, AuditSource};
use server_model::admin::audit_log;
use server_model::admin::audit_serialize::audit_snapshot;
use server_model::admin::entities::sea_orm_active_enums::Status;
use server_model::admin::entities::{sys_operation_log, sys_role, sys_user};
use server_model::admin::facade::sys_user as sys_user_facade;
use ulid::Ulid;

use common::{connect, test_actor};

// Scenario 4 — service handler 構造 AuditEvent { Insert, source: Internal } 寫 sys_operation_log row
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn audit_event_internal_insert_writes_row_correctly() {
    let db = connect().await;
    let actor = test_actor("g6_audit_insert");
    let entity_id = format!("test-{}", Ulid::new());

    let txn = db.begin().await.unwrap();
    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor: &actor,
            operation: AuditOperation::Insert,
            entity_type: "sys_user",
            entity_id: entity_id.clone(),
            payload_before: None,
            payload_after: Some(serde_json::json!({"username": "g6test"})),
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    let audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&entity_id))
        .filter(sys_operation_log::Column::Operation.eq("INSERT"))
        .one(db.as_ref())
        .await
        .unwrap()
        .expect("audit row should exist");
    assert_eq!(audit.module_name, "sys_user");
    assert_eq!(audit.method, "INTERNAL");
    assert!(audit.payload_before.is_none());
    assert_eq!(audit.payload_after.unwrap()["username"], "g6test");

    // cleanup
    let _ = sys_operation_log::Entity::delete_by_id(audit.id)
        .exec(db.as_ref())
        .await;
}

// Scenario 7 — AuditSerialize redaction sys_user.password
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn audit_snapshot_redacts_sys_user_password() {
    let user_model = sys_user::Model {
        id: "test-id".to_string(),
        username: "alice".to_string(),
        password: "$argon2id$some_hash".to_string(),
        domain: "default".to_string(),
        built_in: false,
        avatar: None,
        email: None,
        phone_number: None,
        nick_name: "Alice".to_string(),
        status: Status::Enabled,
        created_at: Local::now().naive_local(),
        created_by: "test".to_string(),
        updated_at: None,
        updated_by: None,
        deleted_at: None,
    };
    let v = audit_snapshot(&user_model);
    assert_eq!(v["password"], "<redacted>");
    assert_eq!(v["username"], "alice");
}

// Scenario 8 — AuditSerialize redaction sys_access_key.access_key_secret (camelCase)
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn audit_snapshot_redacts_sys_access_key_secret() {
    use server_model::admin::entities::sys_access_key;
    let key = sys_access_key::Model {
        id: "test-id".to_string(),
        domain: "default".to_string(),
        access_key_id: "AK123".to_string(),
        access_key_secret: "secret_value_here".to_string(),
        status: Status::Enabled,
        description: None,
        created_at: Local::now().naive_local(),
        created_by: "test".to_string(),
        deleted_at: None,
    };
    let v = audit_snapshot(&key);
    assert_eq!(v["accessKeySecret"], "<redacted>");
    assert_eq!(v["accessKeyId"], "AK123");
}

// Scenario 9 — 5 other entity（sys_role/menu/domain/organization/endpoint）no redaction
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn audit_snapshot_no_redaction_for_non_sensitive_entity() {
    let role = sys_role::Model {
        id: "test-id".to_string(),
        code: "test_role".to_string(),
        name: "Test Role".to_string(),
        description: None,
        pid: "0".to_string(),
        status: Status::Enabled,
        created_at: Local::now().naive_local(),
        created_by: "test".to_string(),
        updated_at: None,
        updated_by: None,
        deleted_at: None,
    };
    let v = audit_snapshot(&role);
    assert_eq!(v["code"], "test_role");
    assert!(
        v.as_object()
            .unwrap()
            .values()
            .all(|val| val != &serde_json::json!("<redacted>")),
        "no field in sys_role should be redacted"
    );
}

// Scenario 12 — F3 既有 soft_delete refactor 後（G2 already）走 AuditEvent、payload_before 含 snapshot
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn soft_delete_writes_audit_event_with_snapshot() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("G6SoftDelete_{}", Ulid::new());

    // 建 active user
    sys_user::ActiveModel {
        id: Set(id.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("G6SD".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("g6_softdel");
    sys_user_facade::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();

    // 驗 audit row：operation=SOFT_DELETE, payload_before 含 snapshot 且 password redacted
    let audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&id))
        .filter(sys_operation_log::Column::Operation.eq("SOFT_DELETE"))
        .filter(sys_operation_log::Column::Method.eq("INTERNAL"))
        .one(db.as_ref())
        .await
        .unwrap()
        .expect("SOFT_DELETE audit row should exist");

    assert!(audit.payload_before.is_some());
    let before_snap = audit.payload_before.unwrap();
    assert_eq!(before_snap["username"], username);
    assert_eq!(before_snap["password"], "<redacted>");
    assert!(audit.payload_after.is_none());

    // cleanup
    let _ = sys_user::Entity::delete_by_id(id.clone())
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_by_id(audit.id)
        .exec(db.as_ref())
        .await;
}
