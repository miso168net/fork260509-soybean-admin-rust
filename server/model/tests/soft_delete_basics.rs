//! F3 SC-006 / SC-007 / SC-009 — soft delete basics + partial unique + tree cascade。
//!
//! 對應 spec.md Acceptance Scenarios 2-7、11-12（Dimension A: sys_user 非樹狀；
//! Dimension B: sys_menu 樹狀 + FR-026 active children check）。
//!
//! 跑前置：
//!   export TEST_DATABASE_URL=postgresql://postgres:123456@127.0.0.1:5432/new_admin_test
//!   cd rust-api && cargo run --bin migration -- up
//!   cargo test --test soft_delete_basics -- --ignored

mod common;

use chrono::Local;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use server_core::web::code;
use server_model::admin::entities::sea_orm_active_enums::{MenuType, Status};
use server_model::admin::entities::sys_menu as menu_entity;
use server_model::admin::entities::sys_operation_log;
use server_model::admin::entities::sys_user as user_entity;
use server_model::admin::facade::{sys_menu, sys_user};
use ulid::Ulid;

use common::{connect, test_actor};

// Scenario 2 + 3 — partial unique index：
// (2) 同 username 已存在 active row 不允許再 INSERT
// (3) 軟刪後同 username 可再 INSERT
#[tokio::test]
#[ignore = "requires real postgres + migration up (run: cargo test --test soft_delete_basics -- --ignored)"]
async fn partial_unique_index_allows_reuse_after_soft_delete() {
    let db = connect().await;
    let username = format!("TestAlice_{}", Ulid::new());

    // 建第一個 active row
    let id1 = Ulid::new().to_string();
    user_entity::ActiveModel {
        id: Set(id1.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Alice1".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await
    .expect("insert 1 ok");

    // 建第二個 active row 同 username — 應 fail（partial unique）
    let id2 = Ulid::new().to_string();
    let res = user_entity::ActiveModel {
        id: Set(id2.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Alice2".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await;
    assert!(res.is_err(), "active 同 username 應被 partial unique 阻擋");

    // 軟刪第一個
    let actor = test_actor("test");
    sys_user::soft_delete_by_id(db.as_ref(), id1.clone(), &actor)
        .await
        .expect("soft delete ok");

    // 再 INSERT 第二個 — 應 OK（partial unique 允許 reuse）
    user_entity::ActiveModel {
        id: Set(id2.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Alice2".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await
    .expect("partial unique 允許 reuse");

    // cleanup
    let _ = user_entity::Entity::delete_many()
        .filter(user_entity::Column::Id.is_in([id1.clone(), id2.clone()]))
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::Description.contains(&id1))
        .exec(db.as_ref())
        .await;
}

// Scenario 4 + 5 — find_active 過濾掉軟刪 row；find_with_deleted 含軟刪 row。
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn find_active_excludes_soft_deleted() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("Excluded_{}", Ulid::new());

    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Excluded".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await
    .expect("insert ok");

    let actor = test_actor("test");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .expect("soft delete ok");

    // find_active 應找不到
    let found = sys_user::find_active()
        .filter(user_entity::Column::Id.eq(&id))
        .one(db.as_ref())
        .await
        .expect("query ok");
    assert!(found.is_none(), "find_active 應過濾軟刪 row");

    // find_with_deleted 應找到
    let found_with = sys_user::find_with_deleted()
        .filter(user_entity::Column::Id.eq(&id))
        .one(db.as_ref())
        .await
        .expect("query ok");
    assert!(found_with.is_some(), "find_with_deleted 應含軟刪 row");

    // cleanup
    let _ = user_entity::Entity::delete_by_id(id.clone())
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::Description.contains(&id))
        .exec(db.as_ref())
        .await;
}

// Scenario 6 — soft_delete_by_id 在同一 transaction 內寫 audit。
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn soft_delete_writes_audit_in_same_transaction() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("AuditTest_{}", Ulid::new());

    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(username),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("AuditTest".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("audit_test");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();

    // 驗 audit row 存在（method="INTERNAL" 標記 service-level audit）
    let audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::ModuleName.eq("sys_user"))
        .filter(
            sys_operation_log::Column::Description.eq(format!("SOFT_DELETE id={}", id)),
        )
        .filter(sys_operation_log::Column::Method.eq("INTERNAL"))
        .one(db.as_ref())
        .await
        .unwrap();
    assert!(audit.is_some(), "audit row should exist for SOFT_DELETE");

    // cleanup
    let _ = user_entity::Entity::delete_by_id(id.clone())
        .exec(db.as_ref())
        .await;
    if let Some(a) = audit {
        let _ = sys_operation_log::Entity::delete_by_id(a.id)
            .exec(db.as_ref())
            .await;
    }
}

// Scenario 7 — restore_by_id 把 deleted_at 設回 NULL + 寫 RESTORE audit。
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn restore_resets_deleted_at_and_writes_audit() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("RestoreTest_{}", Ulid::new());

    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(username),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("Restore".to_string()),
        status: Set(Status::Enabled),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        gender: sea_orm::ActiveValue::NotSet,
    }
    .insert(db.as_ref())
    .await
    .unwrap();

    let actor = test_actor("restore_test");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();
    sys_user::restore_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();

    // active row 應再找得到、deleted_at = NULL
    let found = sys_user::find_active()
        .filter(user_entity::Column::Id.eq(&id))
        .one(db.as_ref())
        .await
        .unwrap();
    assert!(found.is_some(), "restored row should be active again");
    assert!(
        found.unwrap().deleted_at.is_none(),
        "deleted_at should be NULL after restore"
    );

    let restore_audit = sys_operation_log::Entity::find()
        .filter(sys_operation_log::Column::Description.eq(format!("RESTORE id={}", id)))
        .filter(sys_operation_log::Column::Method.eq("INTERNAL"))
        .one(db.as_ref())
        .await
        .unwrap();
    assert!(
        restore_audit.is_some(),
        "RESTORE audit row should exist"
    );

    // cleanup
    let _ = user_entity::Entity::delete_by_id(id.clone())
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::Description.contains(&id))
        .exec(db.as_ref())
        .await;
}

// Scenario 11 + 12 — tree-cascade FR-026：
// (11) parent 有 active child 時 soft_delete 應拒絕（CODE_BUSINESS_STATE_CONFLICT 6003）
// (12) 先刪 child 再刪 parent → 兩次都成功
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn tree_cascade_blocks_soft_delete_with_active_children() {
    let db = connect().await;

    // 建 parent menu（i32 PK auto_increment，由 DB assign）
    let parent_route = format!("test-parent-{}", Ulid::new());
    let parent_active = menu_entity::ActiveModel {
        menu_type: Set(MenuType::Directory),
        menu_name: Set("TestParent".to_string()),
        icon_type: Set(None),
        icon: Set(None),
        route_name: Set(parent_route),
        route_path: Set("/test-parent".to_string()),
        component: Set("layout.base".to_string()),
        path_param: Set(None),
        status: Set(Status::Enabled),
        active_menu: Set(None),
        hide_in_menu: Set(None),
        pid: Set("0".to_string()),
        sequence: Set(99),
        i18n_key: Set(None),
        keep_alive: Set(None),
        constant: Set(false),
        href: Set(None),
        multi_tab: Set(None),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(db.as_ref())
    .await
    .unwrap();
    let parent_id = parent_active.id;

    // 建 child menu（pid = parent_id.to_string()）
    let child_route = format!("test-child-{}", Ulid::new());
    let child_active = menu_entity::ActiveModel {
        menu_type: Set(MenuType::Menu),
        menu_name: Set("TestChild".to_string()),
        icon_type: Set(None),
        icon: Set(None),
        route_name: Set(child_route),
        route_path: Set("/test-child".to_string()),
        component: Set("view.test".to_string()),
        path_param: Set(None),
        status: Set(Status::Enabled),
        active_menu: Set(None),
        hide_in_menu: Set(None),
        pid: Set(parent_id.to_string()),
        sequence: Set(1),
        i18n_key: Set(None),
        keep_alive: Set(None),
        constant: Set(false),
        href: Set(None),
        multi_tab: Set(None),
        created_at: Set(Local::now().naive_local()),
        created_by: Set("test".to_string()),
        updated_at: Set(None),
        updated_by: Set(None),
        deleted_at: Set(None),
        ..Default::default()
    }
    .insert(db.as_ref())
    .await
    .unwrap();
    let child_id = child_active.id;

    let actor = test_actor("tree_test");

    // Scenario 11 — 嘗試刪 parent，應 fail with 6003
    let err = sys_menu::soft_delete_by_id(db.as_ref(), parent_id, &actor)
        .await
        .expect_err("parent with active child should reject");
    assert_eq!(err.code, code::CODE_BUSINESS_STATE_CONFLICT);
    assert!(
        err.message.contains("active children"),
        "error message: {}",
        err.message
    );

    // Scenario 12 — 先刪 child 再刪 parent，兩次都應 OK
    sys_menu::soft_delete_by_id(db.as_ref(), child_id, &actor)
        .await
        .expect("delete child ok");
    sys_menu::soft_delete_by_id(db.as_ref(), parent_id, &actor)
        .await
        .expect("delete parent ok after child gone");

    // cleanup
    let _ = menu_entity::Entity::delete_many()
        .filter(menu_entity::Column::Id.is_in([parent_id, child_id]))
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::ModuleName.eq("sys_menu"))
        .filter(
            sys_operation_log::Column::Description
                .contains(&format!("id={}", parent_id)),
        )
        .exec(db.as_ref())
        .await;
    let _ = sys_operation_log::Entity::delete_many()
        .filter(sys_operation_log::Column::ModuleName.eq("sys_menu"))
        .filter(
            sys_operation_log::Column::Description
                .contains(&format!("id={}", child_id)),
        )
        .exec(db.as_ref())
        .await;
}
