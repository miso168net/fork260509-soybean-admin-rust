use std::sync::Arc;

use axum::{extract::{Path, Query}, Extension};
use server_core::web::{audit::Actor, auth::User, error::AppError, res::Res, validator::ValidatedForm};
use server_service::admin::{
    CreateMenuInput, IsRouteExistInput, MenuRoute, MenuTree, SysMenuModel, SysMenuService,
    TMenuService, UpdateMenuInput,
};

pub struct SysMenuApi;

impl SysMenuApi {
    pub async fn tree_menu(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<MenuTree>>, AppError> {
        service.tree_menu().await.map(Res::new_data)
    }

    pub async fn get_menu_list(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<MenuTree>>, AppError> {
        service.get_menu_list().await.map(Res::new_data)
    }

    pub async fn get_constant_routes(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<MenuRoute>>, AppError> {
        service.get_constant_routes().await.map(Res::new_data)
    }

    pub async fn create_menu(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<CreateMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        service.create_menu(input, &actor).await.map(Res::new_data)
    }

    pub async fn get_menu(
        Path(id): Path<i32>,
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        service.get_menu(id).await.map(Res::new_data)
    }

    pub async fn update_menu(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        ValidatedForm(input): ValidatedForm<UpdateMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        service.update_menu(input, &actor).await.map(Res::new_data)
    }

    pub async fn delete_menu(
        Path(id): Path<i32>,
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<()>, AppError> {
        let actor = Actor::from(&user);
        service.delete_menu(id, &actor).await.map(Res::new_data)
    }

    pub async fn get_auth_routes(
        Path(role_id): Path<String>,
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<Vec<i32>>, AppError> {
        service
            .get_menu_ids_by_role_id(role_id, user.domain())
            .await
            .map(Res::new_data)
    }

    pub async fn is_route_exist(
        Extension(service): Extension<Arc<SysMenuService>>,
        Query(input): Query<IsRouteExistInput>,
    ) -> Result<Res<bool>, AppError> {
        service.is_route_exist(&input.route_name).await.map(Res::new_data)
    }

    // F9 systemManage-alias-router: GET /systemManage/getAllPages
    // 回 sys_menu route_name list (distinct + active rows)、base-web menu page binding 下拉用
    pub async fn get_all_pages(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<String>>, AppError> {
        service.find_all_page_keys().await.map(Res::new_data)
    }
}
