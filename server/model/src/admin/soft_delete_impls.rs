//! F3 `SoftDeletable` trait impls — 7 個 admin entity 的對應實作。
//!
//! Trait 本身定義在 `server-core::db::soft_delete::SoftDeletable`；impl block 落在
//! 本 crate（`server-model`）是因為 `server-model` 已 dep `server-core`，反向加 dep
//! 會形成循環。orphan rules 允許 own-crate-type ⨯ foreign-trait 的組合。
//!
//! per [`data-model.md`](../../../../../specs/002-soft-delete-infrastructure/data-model.md) §E4
//! （§E4 範例假設此 impl 落在 server-core；實際上必須改放在 server-model 才能 build）。

use server_core::db::soft_delete::SoftDeletable;

use crate::admin::entities::{
    sys_access_key, sys_domain, sys_endpoint, sys_menu, sys_organization, sys_role, sys_user,
};

impl SoftDeletable for sys_user::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_user::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_user";
}

impl SoftDeletable for sys_role::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_role::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_role";
}

impl SoftDeletable for sys_menu::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_menu::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_menu";
}

impl SoftDeletable for sys_domain::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_domain::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_domain";
}

impl SoftDeletable for sys_organization::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_organization::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_organization";
}

impl SoftDeletable for sys_endpoint::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_endpoint::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_endpoint";
}

impl SoftDeletable for sys_access_key::Entity {
    const DELETED_AT_COLUMN: Self::Column = sys_access_key::Column::DeletedAt;
    const ENTITY_TYPE: &'static str = "sys_access_key";
}
