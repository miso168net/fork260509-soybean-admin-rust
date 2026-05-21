//! F3 SC-010 軟刪 user + JWT auth gate building block 驗證。
//!
//! Scenario 9：軟刪 user 後，login 流程透過 `find_active().filter(Username.eq)` 查不到。
//! Scenario 10：JWT middleware 透過 `find_active().filter(Id.eq)` 查不到（FR-028 building block）。
//!
//! HTTP-level envelope 8888 驗證留 quickstart Step 7c（G12 T053 範圍）。
//!
//! 跑前置：見 soft_delete_basics.rs header。

mod common;

use chrono::Local;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use server_model::admin::entities::sea_orm_active_enums::Status;
use server_model::admin::entities::sys_operation_log;
use server_model::admin::entities::sys_user as user_entity;
use server_model::admin::facade::sys_user;
use ulid::Ulid;

use common::{connect, test_actor};

// Scenario 9 + 10 building block：軟刪 user 走 find_active().filter(Username|Id.eq) → None
#[tokio::test]
#[ignore = "requires real postgres + migration up"]
async fn soft_deleted_user_not_findable_by_active_filter() {
    let db = connect().await;
    let id = Ulid::new().to_string();
    let username = format!("Banned_{}", Ulid::new());

    user_entity::ActiveModel {
        id: Set(id.clone()),
        username: Set(username.clone()),
        password: Set("pwd".to_string()),
        domain: Set("default".to_string()),
        built_in: Set(false),
        avatar: Set(None),
        email: Set(None),
        phone_number: Set(None),
        nick_name: Set("B".to_string()),
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

    let actor = test_actor("auth_gate");
    sys_user::soft_delete_by_id(db.as_ref(), id.clone(), &actor)
        .await
        .unwrap();

    // Scenario 9 building block：login flow 用 username 查 active user → None
    let by_username = sys_user::find_active()
        .filter(user_entity::Column::Username.eq(&username))
        .one(db.as_ref())
        .await
        .unwrap();
    assert!(
        by_username.is_none(),
        "soft-deleted user should not be findable by username via find_active"
    );

    // Scenario 10 building block：JWT middleware 透過 id 查 → None
    let by_id = sys_user::find_active()
        .filter(user_entity::Column::Id.eq(&id))
        .one(db.as_ref())
        .await
        .unwrap();
    assert!(
        by_id.is_none(),
        "soft-deleted user should not be findable by id via find_active"
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
