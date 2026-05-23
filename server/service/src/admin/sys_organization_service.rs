use async_trait::async_trait;
use sea_orm::{ColumnTrait, Condition, PaginatorTrait, QueryFilter};
use server_core::web::{error::AppError, page::PaginatedData};
use server_model::admin::{
    facade::sys_organization::{
        self, Column as SysOrganizationColumn, Model as SysOrganizationModel,
    },
    input::OrganizationPageRequest,
};

use super::sys_organization_error::OrganizationError;
use crate::helper::db_helper;

#[async_trait]
pub trait TOrganizationService {
    async fn find_paginated_organizations(
        &self,
        params: OrganizationPageRequest,
    ) -> Result<PaginatedData<SysOrganizationModel>, AppError>;

    /// 039 rust-entity-id-numeric-migration C3: by-display_id lookup helper。
    /// base-web 對外傳 numeric display_id；rust 內部 PK/FK 仍走 ULID 字串。
    /// handler 收 Path<i64> 後第一步透過本方法解析回 ULID，再走後續 service 既有路徑。
    /// 軟刪資料不可解析（find_active() filter DeletedAt.is_null）。
    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError>;
}

pub struct SysOrganizationService;

#[async_trait]
impl TOrganizationService for SysOrganizationService {
    async fn find_paginated_organizations(
        &self,
        params: OrganizationPageRequest,
    ) -> Result<PaginatedData<SysOrganizationModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = sys_organization::find_active();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any()
                .add(SysOrganizationColumn::Code.contains(keywords))
                .add(SysOrganizationColumn::Name.contains(keywords))
                .add(SysOrganizationColumn::Description.contains(keywords));
            query = query.filter(condition);
        }

        let total = query
            .clone()
            .count(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let paginator = query.paginate(db.as_ref(), params.page_details.size);
        let records = paginator
            .fetch_page(params.page_details.current - 1)
            .await
            .map_err(AppError::from)?;

        Ok(PaginatedData {
            current: params.page_details.current,
            size: params.page_details.size,
            total,
            records,
        })
    }

    async fn lookup_ulid_by_display_id(&self, display_id: i64) -> Result<String, AppError> {
        let db = db_helper::get_db_connection().await?;
        let org = sys_organization::find_active()
            .filter(SysOrganizationColumn::DisplayId.eq(display_id))
            .one(db.as_ref())
            .await
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::from(OrganizationError::OrganizationNotFound))?;
        Ok(org.id)
    }
}
