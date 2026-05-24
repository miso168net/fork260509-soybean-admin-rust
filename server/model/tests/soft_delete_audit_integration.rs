//! F3 SC-006 audit transaction discipline + edge cases。
//!
//! 覆 spec.md：
//! - 對已軟刪 row 再 soft_delete → 6001（rows_affected = 0）
//! - 對 active row restore → 6001（找不到 deleted_at IS NOT NULL）
//! - 多次 SOFT_DELETE/RESTORE → audit row 數應對應 operation 數
//!
//! 跑前置：見 soft_delete_basics.rs header。

mod common;

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use server_core::web::code;
use server_model::admin::entities::sea_orm_active_enums::Status;
use server_model::admin::entities::sys_operation_log;
use server_model::admin::entities::sys_user as user_entity;
use server_model::admin::facade::sys_user;
use ulid::Ulid;

use common::{audit_pipeline::wait_for_audit_count, connect, test_actor};

// 對已軟刪 row 再次 soft_delete → CODE_BUSINESS_ENTITY_NOT_FOUND (6001)
#[tokio::test]
#[ignore = "requires dev stack drainer running (audit outbox → sys_operation_log async pipeline)"]
async fn double_soft_delete_returns_not_found() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(format!("Double_{}", Ulid::new())),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("D".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
        display_id: Set(Local::now().timestamp_nanos_opt().unwrap_or(1)),
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("double");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();

    // 第二次軟刪 — 應 6001
    let err = sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .expect_err("double soft delete should fail");
    assert_eq!(err.code, code::CODE_BUSINESS_ENTITY_NOT_FOUND);

    // cleanup
    let _ = user_entity::Entity::delete_by_id(id.clone())
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::Description.contains(&id))
        .exec(db.as_ref())
        .await;
}

// 對 active row restore → CODE_BUSINESS_ENTITY_NOT_FOUND (6001)
// （restore_by_id WHERE deleted_at IS NOT NULL 找不到）
#[tokio::test]
#[ignore = "requires dev stack drainer running (audit outbox → sys_operation_log async pipeline)"]
async fn restore_active_row_returns_not_found() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(format!("RestoreActive_{}", Ulid::new())),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("RA".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
        display_id: Set(Local::now().timestamp_nanos_opt().unwrap_or(1)),
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("restore_active");
    // 對 active row restore — 應 6001
    let err = sys_user::restore_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .expect_err("restoring active row should fail");
    assert_eq!(err.code, code::CODE_BUSINESS_ENTITY_NOT_FOUND);

    // cleanup（restore 失敗→ 不會寫 audit；只清 user row）
    let _ = user_entity::Entity::delete_by_id(id)
        .exec(db.as_ref())
        .await;
}

// 多次 SOFT_DELETE / RESTORE / SOFT_DELETE → 應有 3 個 audit row
#[tokio::test]
#[ignore = "requires dev stack drainer running (audit outbox → sys_operation_log async pipeline)"]
async fn audit_row_count_matches_operations() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(format!("Counted_{}", Ulid::new())),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("C".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
        display_id: Set(Local::now().timestamp_nanos_opt().unwrap_or(1)),
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("counted");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();
    sys_user::restore_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();
    // 預期 3 個 audit row（SOFT_DELETE / RESTORE / SOFT_DELETE）

    let count = wait_for_audit_count(
        || {
            let db = db.clone();
            let id = id.clone();
            async move {
                sys_operation_log::Entity::find()
                    .filter(sys_operation_log::Column::ModuleName.eq("sys_user"))
                    .filter(sys_operation_log::Column::Description.contains(&id))
                    .filter(sys_operation_log::Column::Method.eq("INTERNAL"))
                    .count(db.as_ref())
                    .await
            }
        },
        3,
        500,
    )
    .await
    .expect("expected 3 audit rows (SOFT_DELETE + RESTORE + SOFT_DELETE) before 500ms timeout");
    assert_eq!(
        count, 3,
        "expected 3 audit rows (SOFT_DELETE + RESTORE + SOFT_DELETE)"
    );

    // cleanup
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::Description.contains(&id))
        .exec(db.as_ref())
        .await;
    let _ = user_entity::Entity::delete_by_id(id)
        .exec(db.as_ref())
        .await;
}
