//! Sea-ORM entity for sys_audit_outbox table (042 audit-outbox-and-http-mount).
//!
//! per spec 042 FR-002、data-model.md §E1。耐久暫存區、由 audit_outbox_drainer 處理。

use sea_orm::entity::prelude::*;
use serde::Serialize;
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize)]
#[sea_orm(table_name = "sys_audit_outbox")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(column_type = "JsonBinary")]
    pub audit_event_json: JsonValue,
    pub published_at: Option<DateTimeWithTimeZone>,
    pub retry_count: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub last_error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
