//! F3 `SoftDeletable` trait — Sea-ORM 軟刪整合層。
//!
//! 為支援軟刪的 entity 提供統一 SELECT API（`find_active` / `find_with_deleted`）
//! 與 audit-related 常數（`DELETED_AT_COLUMN` / `ENTITY_TYPE`）。
//!
//! `soft_delete_by_id` / `restore_by_id` **不**在 trait 內 — 由各 entity 對應的
//! facade module 自寫（per [`research.md`](../../../../../../specs/002-soft-delete-infrastructure/research.md)
//! R4，避開 Sea-ORM PrimaryKey generic noise + 讓樹狀 entity 自然有 children-check hook 點）。
//!
//! 7 個 entity 的 `impl SoftDeletable` block 由後續 G4 task 落地、本檔此刻僅含 trait 定義。

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Select};

/// 軟刪 entity 必備的常數 + 預設 SELECT helpers。
pub trait SoftDeletable: EntityTrait {
    /// 對應 entity Model 的 `deleted_at` column。
    const DELETED_AT_COLUMN: Self::Column;

    /// audit log 用 — 寫 `sys_operation_log.module_name` 的固定字串（如 `"sys_user"`）。
    const ENTITY_TYPE: &'static str;

    /// SELECT 預設過濾掉軟刪 row（隱含 `WHERE deleted_at IS NULL`）。
    fn find_active() -> Select<Self> {
        Self::find().filter(Self::DELETED_AT_COLUMN.is_null())
    }

    /// SELECT 含全部 row（active + soft-deleted）— admin 場景顯式使用。
    fn find_with_deleted() -> Select<Self> {
        Self::find()
    }
}
