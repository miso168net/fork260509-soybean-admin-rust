use async_trait::async_trait;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use server_core::web::{error::AppError, page::PaginatedData};
use server_model::admin::{
    entities::{
        prelude::SysOperationLog,
        sys_operation_log::{Column as SysOperationLogColumn, Model as SysOperationLogModel},
    },
    input::OperationLogPageRequest,
};

use crate::helper::db_helper;

#[async_trait]
pub trait TOperationLogService {
    async fn find_paginated_operation_logs(
        &self,
        params: OperationLogPageRequest,
    ) -> Result<PaginatedData<SysOperationLogModel>, AppError>;
}

pub struct SysOperationLogService;

#[async_trait]
impl TOperationLogService for SysOperationLogService {
    async fn find_paginated_operation_logs(
        &self,
        params: OperationLogPageRequest,
    ) -> Result<PaginatedData<SysOperationLogModel>, AppError> {
        let db = db_helper::get_db_connection().await?;
        let mut query = SysOperationLog::find();

        if let Some(ref keywords) = params.keywords {
            let condition = Condition::any()
                .add(SysOperationLogColumn::Domain.contains(keywords))
                .add(SysOperationLogColumn::Username.contains(keywords))
                .add(SysOperationLogColumn::Ip.contains(keywords))
                .add(SysOperationLogColumn::UserAgent.contains(keywords));
            query = query.filter(condition);
        }

        query = query.order_by_desc(SysOperationLogColumn::CreatedAt);

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

}
