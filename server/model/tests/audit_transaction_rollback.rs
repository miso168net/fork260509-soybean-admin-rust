//! F2.1 audit transaction rollback discipline — business fail → audit row 不寫。
//!
//! 對應 spec.md Acceptance Scenarios 13-14。
//!
//! 跑前置：見 audit_basics.rs header。

mod common;

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    Set, TransactionTrait,
};
use server_core::web::audit::{AuditEvent, AuditOperation, AuditSource};
use server_model::admin::audit_log;
use server_model::admin::audit_serialize::audit_snapshot;
use server_model::admin::entities::sea_orm_active_enums::Status;
use server_model::admin::entities::{sys_operation_log, sys_user};
use ulid::Ulid;

use common::{connect, test_actor};

// Scenario 13 — INSERT 業務 + INSERT audit 同 txn，手動 rollback 模擬 business fail
// → 業務 row 與 audit row 都不留。
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn explicit_rollback_drops_both_business_and_audit_rows() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("G6Rollback_{}", Ulid::new());
    let actor = test_actor("g6_rollback");

    let txn = db.begin().await.unwrap();

    // INSERT 業務 row
    let user_am = sys_user::ActiveModel {
        id: Set(id.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("R".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
    };
    let user_model = user_am.insert(&txn).await.unwrap();

    // INSERT audit row 同 txn
    audit_log::write_in_txn(
        &txn,
        AuditEvent {
            actor: &actor,
            operation: AuditOperation::Insert,
            entity_type: "sys_user",
            entity_id: id.clone(),
            payload_before: None,
            payload_after: Some(audit_snapshot(&user_model)),
            description: None,
            source: AuditSource::Internal,
            request_id: None,
        },
    )
    .await
    .unwrap();

    // 手動 rollback 模擬 business fail
    txn.rollback().await.unwrap();

    // 驗 sys_user 沒留 row
    let user_count = sys_user::Entity::find()
        .filter(sys_user::Column::Id.eq(&id))
        .count(db.as_ref())
        .await
        .unwrap();
    assert_eq!(user_count, 0, "business row should be rolled back");

    // 驗 sys_operation_log 沒留 audit row
    let audit_count = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&id))
        .count(db.as_ref())
        .await
        .unwrap();
    assert_eq!(audit_count, 0, "audit row should be rolled back together");
}

// Scenario 14 — business UPDATE unique violation → audit row 不寫。
// setup 兩 user、txn 內把 bob 改名為 alice（撞 partial unique）→ update fail → drop 自動 rollback。
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn business_unique_violation_drops_audit_row() {
    let db = connect().await;
    let alice_id = Ulid::new().to_string();
    let bob_id = Ulid::new().to_string();
    let alice_name = format!("Alice_{}", Ulid::new());
    let bob_name = format!("Bob_{}", Ulid::new());

    // setup: 兩個 active user
    sys_user::ActiveModel {
        id: Set(alice_id.clone()),
        username: Set(alice_name.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Alice".to_string()),
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
    sys_user::ActiveModel {
        id: Set(bob_id.clone()),
        username: Set(bob_name.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Bob".to_string()),
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

    // 試在 txn 內把 bob 改名為 alice — partial unique 應 fail
    let txn = db.begin().await.unwrap();
    let bob_before = sys_user::Entity::find()
        .filter(sys_user::Column::Id.eq(&bob_id))
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    let mut am = bob_before.clone().into_active_model();
    am.username = Set(alice_name.clone()); // duplicate active username
    let update_result = am.update(&txn).await;
    assert!(update_result.is_err(), "unique violation expected");
    // 不 commit（business fail）— Drop 自動 rollback

    // 驗 bob 的 username 仍是 bob_name
    let bob_after = sys_user::Entity::find()
        .filter(sys_user::Column::Id.eq(&bob_id))
        .one(db.as_ref())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(bob_after.username, bob_name);

    // 驗 audit row 沒寫（因 txn rollback）
    let audit_count = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::EntityId.eq(&bob_id))
        .filter(sys_operation_log::Column::Operation.eq("UPDATE"))
        .count(db.as_ref())
        .await
        .unwrap();
    assert_eq!(audit_count, 0);

    // cleanup
    let _ = sys_user::Entity::delete_many()
        .filter(sys_user::Column::Id.is_in([alice_id, bob_id]))
        .exec(db.as_ref())
        .await;
}
