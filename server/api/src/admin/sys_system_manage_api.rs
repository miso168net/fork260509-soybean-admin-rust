//! F7 manage-crud-alignment: 5 個 systemManage alias wrapper handler。
//! per F7 spec FR-007 + FR-008 + data-model E6 — call F9 既有 service method、
//! 用 .into() map raw model → F7 Output DTO、wrap Res::new_data envelope。
//! W-FW1: add/update user transform handlers (base-web shape → backend domain shape).
//! W-FW2: add/update/delete/batchDelete menu transform handlers (base-web shape → backend domain shape).
//! W-FW4: getRoleMenuIds/assignRoleMenus 角色菜單授權 alias handlers (domain 由 JWT actor 注入).
//! W-FW8 US1: getAllEndpoints / getRoleEndpointIds/:roleId / assignRoleEndpoints transform handlers.

use std::sync::Arc;

use axum::{extract::{Path, Query}, Extension, Json};
use axum_casbin::{casbin::MgmtApi, CasbinAxumLayer};
use serde_json::{json, Value};
use server_core::web::{audit::Actor, auth::User, code, error::AppError, page::PaginatedData, res::Res};
use server_model::admin::facade::sys_endpoint;
use server_service::admin::{
    AssignRoleMenusInput, BatchDeleteMenuInput, BatchDeleteRoleInput, CreateRoleInput,
    CreateUserInput, DeleteMenuByBodyInput, DeleteRoleByBodyInput, EndpointTreeNode, Gender,
    MenuInput, MenuType, RoleInput, RolePageRequest, Status, SysAuthorizationService,
    SysEndpointService, SysMenuModel, SysMenuService, SysRoleModel, SysRoleService, SysUserService,
    SystemManageAddMenuInput, SystemManageAddRoleInput, SystemManageAddUserInput,
    SystemManageAllRoleOutput, SystemManageAssignRoleEndpointsInput, SystemManageMenuOutput,
    SystemManageMenuTreeNodeOutput, SystemManageRoleOutput, SystemManageUpdateMenuInput,
    SystemManageUpdateRoleInput, SystemManageUpdateUserInput, SystemManageUserOutput,
    TAuthorizationService, TEndpointService, TMenuService, TRoleService, TUserService,
    UpdateMenuInput, UpdateRoleHomeInput, UpdateRoleInput, UpdateUserInput, UserPageRequest,
    UserWithoutPassword,
};
use server_service::helper::db_helper;

pub struct SysSystemManageApi;

impl SysSystemManageApi {
    /// F7 alias: GET /systemManage/getRoleList
    pub async fn list_roles_for_systemmanage(
        Query(params): Query<RolePageRequest>,
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<PaginatedData<SystemManageRoleOutput>>, AppError> {
        let raw = service.find_paginated_roles(params).await?;
        Ok(Res::new_data(PaginatedData {
            current: raw.current,
            size: raw.size,
            total: raw.total,
            records: raw.records.into_iter().map(Into::into).collect(),
        }))
    }

    /// F7 alias: GET /systemManage/getAllRoles
    pub async fn list_all_roles_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<Vec<SystemManageAllRoleOutput>>, AppError> {
        let raw = service.find_all_enabled().await?;
        Ok(Res::new_data(raw.into_iter().map(Into::into).collect()))
    }

    /// F7 alias: GET /systemManage/getUserList
    /// W-FW5 T002: user_roles 由批次查詢真實填充（取代 From impl 硬寫 vec![]）。
    /// 039 cascade: UserWithoutPassword.id 已改 i64 display_id；handler 先逐筆 lookup ULID
    /// （admin 列表通量低、sequential await 簡單即可），再批次撈 role code 填回 records。
    pub async fn list_users_for_systemmanage(
        Query(params): Query<UserPageRequest>,
        Extension(service): Extension<Arc<SysUserService>>,
    ) -> Result<Res<PaginatedData<SystemManageUserOutput>>, AppError> {
        let raw = service.find_paginated_users(params).await?;
        let mut records: Vec<SystemManageUserOutput> =
            raw.records.into_iter().map(Into::into).collect();

        // 039 cascade: display_id (i64) → ULID (String) 配對；後續 get_role_codes_for_users
        // 仍走 ULID（避免改 service signature），最後用配對表把 role code 填回 record。
        let mut display_to_ulid: std::collections::HashMap<i64, String> =
            std::collections::HashMap::with_capacity(records.len());
        for record in &records {
            let ulid = service.lookup_ulid_by_display_id(record.id).await?;
            display_to_ulid.insert(record.id, ulid);
        }
        let user_ulids: Vec<String> = display_to_ulid.values().cloned().collect();
        let roles_map = service.get_role_codes_for_users(user_ulids).await?;
        for record in &mut records {
            if let Some(ulid) = display_to_ulid.get(&record.id) {
                record.user_roles = roles_map.get(ulid).cloned().unwrap_or_default();
            }
        }

        Ok(Res::new_data(PaginatedData {
            current: raw.current,
            size: raw.size,
            total: raw.total,
            records,
        }))
    }

    /// F7 alias: GET /systemManage/getMenuList/v2
    pub async fn list_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<PaginatedData<SystemManageMenuOutput>>, AppError> {
        // F7 follow-up: base-web `Api.SystemManage.MenuList = PaginatingQueryRecord<Menu>`
        // 預期 paginated 包裝。F7 原 wrapper 回扁平 array、base view 顯示「无数据」。
        // 改為 paginated envelope:size=total=records.len()(rust 端不分頁、一次回全)。
        let raw = service.get_menu_list().await?;
        let records: Vec<SystemManageMenuOutput> = raw.into_iter().map(Into::into).collect();
        let total = records.len() as u64;
        Ok(Res::new_data(PaginatedData {
            current: 1,
            size: total,
            total,
            records,
        }))
    }

    /// F7 alias: GET /systemManage/getMenuTree
    pub async fn tree_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
    ) -> Result<Res<Vec<SystemManageMenuTreeNodeOutput>>, AppError> {
        let raw = service.tree_menu().await?;
        Ok(Res::new_data(raw.into_iter().map(Into::into).collect()))
    }

    /// W-FW1 transform: POST /systemManage/addUser (base-web shape → backend domain shape)
    /// W-FW5: 收 userRoles（角色指派）+ 選填 password。
    pub async fn add_user_for_systemmanage(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        // W-FW5 T010: None / Some("") 皆視為未提供 → 沿用預設密碼 123456
        let password = normalize_password(input.password)
            .unwrap_or_else(|| "123456".to_string());
        let create_input = CreateUserInput {
            domain: "built-in".to_string(),
            username: input.user_name,
            password,
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        let created = service.create_user(create_input, &actor).await?;
        // W-FW5 T005: user→roles 指派（空清單為合法清空）
        // 039 cascade: created.id 已改 i64 display_id；先 lookup ULID 才能餵 assign_roles_to_user(user_id: String)。
        let created_ulid = service.lookup_ulid_by_display_id(created.id).await?;
        service
            .assign_roles_to_user(created_ulid, input.user_roles, &actor)
            .await?;
        Ok(Res::new_data(created))
    }

    /// W-FW1 transform: POST /systemManage/updateUser (base-web shape → backend domain shape)
    /// W-FW5: 收 userRoles（角色指派）+ 選填 password。
    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 update_user(user_ulid, …)
    /// 與 assign_roles_to_user(user_ulid, …)，避免二次 lookup。
    pub async fn update_user_for_systemmanage(
        Extension(service): Extension<Arc<SysUserService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateUserInput>,
    ) -> Result<Res<UserWithoutPassword>, AppError> {
        let actor = Actor::from(&user);
        let user_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        let update_input = UpdateUserInput {
            id: input.id,
            domain: "built-in".to_string(),
            username: input.user_name,
            // W-FW5 T010: None / Some("") 皆視為未提供 → 不動 password
            password: normalize_password(input.password),
            nick_name: input.nick_name,
            avatar: None,
            email: input.user_email,
            phone_number: input.user_phone,
            status: map_status(&input.status)?,
            gender: map_gender(input.user_gender.as_deref())?,
        };
        let updated = service.update_user(&user_ulid, update_input, &actor).await?;
        // W-FW5 T005: user→roles 指派（空清單為合法清空）
        service
            .assign_roles_to_user(user_ulid, input.user_roles, &actor)
            .await?;
        Ok(Res::new_data(updated))
    }

    /// W-FW2 transform: POST /systemManage/addMenu (base-web shape → backend domain shape)
    pub async fn add_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        let create_input = MenuInput {
            menu_type: map_menu_type(&input.menu_type)?,
            menu_name: input.menu_name,
            icon_type: map_icon_type(input.icon_type.as_deref())?,
            icon: input.icon,
            route_name: input.route_name,
            route_path: input.route_path,
            component: input.component,
            path_param: None, // MenuInput 既有欄位，base-web 表單無對應，刻意填 None
            status: map_status(&input.status)?,
            active_menu: input.active_menu,
            hide_in_menu: input.hide_in_menu,
            pid: input.parent_id.to_string(),
            sequence: input.order,
            i18n_key: input.i18n_key,
            keep_alive: input.keep_alive,
            constant: input.constant,
            href: input.href,
            multi_tab: input.multi_tab,
            query: input.query,
            buttons: input.buttons,
            fixed_index_in_tab: input.fixed_index_in_tab,
        };
        service.create_menu(create_input, &actor).await.map(Res::new_data)
    }

    /// W-FW2 transform: POST /systemManage/updateMenu (base-web shape → backend domain shape)
    pub async fn update_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateMenuInput>,
    ) -> Result<Res<SysMenuModel>, AppError> {
        let actor = Actor::from(&user);
        let update_input = UpdateMenuInput {
            id: input.id,
            menu: MenuInput {
                menu_type: map_menu_type(&input.menu_type)?,
                menu_name: input.menu_name,
                icon_type: map_icon_type(input.icon_type.as_deref())?,
                icon: input.icon,
                route_name: input.route_name,
                route_path: input.route_path,
                component: input.component,
                path_param: None, // MenuInput 既有欄位，base-web 表單無對應，刻意填 None
                status: map_status(&input.status)?,
                active_menu: input.active_menu,
                hide_in_menu: input.hide_in_menu,
                pid: input.parent_id.to_string(),
                sequence: input.order,
                i18n_key: input.i18n_key,
                keep_alive: input.keep_alive,
                constant: input.constant,
                href: input.href,
                multi_tab: input.multi_tab,
                query: input.query,
                buttons: input.buttons,
                fixed_index_in_tab: input.fixed_index_in_tab,
            },
        };
        service.update_menu(update_input, &actor).await.map(Res::new_data)
    }

    /// W-FW2 transform: DELETE /systemManage/deleteMenu (body id)
    pub async fn delete_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteMenuByBodyInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        service.delete_menu(input.id, &actor).await.map(|_| Res::new_data(true))
    }

    /// W-FW2 transform: DELETE /systemManage/batchDeleteMenu
    /// per-row Err 不快、continue loop（比照 W-FW1/F9 batch_delete_users 體例）
    pub async fn batch_delete_menu_for_systemmanage(
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteMenuInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for id in input.ids {
            match service.delete_menu(id, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }

    /// W-FW3 transform: POST /systemManage/addRole (base-web shape → backend domain shape)
    pub async fn add_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageAddRoleInput>,
    ) -> Result<Res<SysRoleModel>, AppError> {
        let actor = Actor::from(&user);
        let create_input = CreateRoleInput {
            // R-Q1 root convention：base-web 角色頁是扁平表格，新角色一律掛 root
            pid: "0".to_string(),
            code: input.role_code,
            name: input.role_name,
            status: map_status(&input.status)?,
            description: input.role_desc,
        };
        service.create_role(create_input, &actor).await.map(Res::new_data)
    }

    /// W-FW3 transform: POST /systemManage/updateRole (base-web shape → backend domain shape)
    /// W-FW6 N4：pid 沿用既有值（避免擾動角色樹）；code 直送 input（rust update_role 同步 Casbin policy）。
    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 get_role / update_role。
    pub async fn update_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<SystemManageUpdateRoleInput>,
    ) -> Result<Res<SysRoleModel>, AppError> {
        let actor = Actor::from(&user);
        let role_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        let existing = service.get_role(&role_ulid).await?;
        let update_input = UpdateRoleInput {
            id: input.id,
            role: RoleInput {
                pid: existing.pid,
                code: input.role_code,
                name: input.role_name,
                status: map_status(&input.status)?,
                description: input.role_desc,
            },
        };
        service
            .update_role(&role_ulid, update_input, &actor)
            .await
            .map(Res::new_data)
    }

    /// W-FW3 transform: DELETE /systemManage/deleteRole (body id)
    /// 039 T030.5: input.id 改 i64；handler 先 lookup_ulid_by_display_id 再餵 delete_role。
    pub async fn delete_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<DeleteRoleByBodyInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        let role_ulid = service.lookup_ulid_by_display_id(input.id).await?;
        service.delete_role(&role_ulid, &actor).await.map(|_| Res::new_data(true))
    }

    /// W-FW3 transform: DELETE /systemManage/batchDeleteRole
    /// per-row Err 不快、continue loop（比照 W-FW2 batch_delete_menu 體例）
    /// 039 T030.5: input.ids 改 Vec<i64>；逐筆 lookup_ulid_by_display_id 後餵 delete_role；
    /// 單筆 lookup 失敗（display_id 無對應 role）視為一般 per-row Err、continue loop（不中斷批次）。
    pub async fn batch_delete_role_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<BatchDeleteRoleInput>,
    ) -> Result<Res<Value>, AppError> {
        let actor = Actor::from(&user);
        let mut deleted_count: usize = 0;
        for display_id in input.ids {
            let ulid = match service.lookup_ulid_by_display_id(display_id).await {
                Ok(u) => u,
                Err(_) => continue,
            };
            match service.delete_role(&ulid, &actor).await {
                Ok(_) => deleted_count += 1,
                Err(_) => continue,
            }
        }
        Ok(Res::new_data(json!({ "deletedCount": deleted_count })))
    }

    /// W-FW4 transform: GET /systemManage/getRoleMenuIds/:roleId
    /// domain 由 JWT actor 伺服器端注入，base-web 不傳。
    /// 039 T024: Path<String> → Path<i64> + role_svc.lookup_ulid_by_display_id cascade。
    pub async fn get_role_menu_ids_for_systemmanage(
        Path(role_display_id): Path<i64>,
        Extension(service): Extension<Arc<SysMenuService>>,
        Extension(role_svc): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
    ) -> Result<Res<Vec<i32>>, AppError> {
        let role_ulid = role_svc.lookup_ulid_by_display_id(role_display_id).await?;
        service
            .get_menu_ids_by_role_id(role_ulid, user.domain())
            .await
            .map(Res::new_data)
    }

    /// W-FW4 transform: POST /systemManage/assignRoleMenus
    /// domain 由 JWT actor 伺服器端注入，base-web 不傳。
    /// W-FW6 US2 (FR-007): handler 注入 Actor 給 service 寫 audit_log。
    /// 039 T024: input.role_id 改 i64；先 lookup ULID 再走 service（menu_ids 仍 i32，不變）。
    pub async fn assign_role_menus_for_systemmanage(
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        Extension(role_svc): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<AssignRoleMenusInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        let role_ulid = role_svc.lookup_ulid_by_display_id(input.role_id).await?;
        service
            .assign_routes(user.domain(), role_ulid, input.menu_ids, &actor)
            .await
            .map(|_| Res::new_data(true))
    }

    /// W-FW6 transform: GET /systemManage/getRoleHome/:roleId
    /// 讀取角色首頁路由（無設定 → null）。RoleNotFound → 4001。
    /// 039 T024: Path<String> → Path<i64> + role_svc.lookup_ulid_by_display_id cascade。
    pub async fn get_role_home_for_systemmanage(
        Path(role_display_id): Path<i64>,
        Extension(service): Extension<Arc<SysRoleService>>,
    ) -> Result<Res<Option<String>>, AppError> {
        let role_ulid = service.lookup_ulid_by_display_id(role_display_id).await?;
        service.get_role_home(&role_ulid).await.map(Res::new_data)
    }

    /// W-FW6 transform: POST /systemManage/updateRoleHome
    /// 寫入角色首頁路由（None / Some("") → NULL 清除；非空 → 必為 enabled + non-constant menu route_name）。
    pub async fn update_role_home_for_systemmanage(
        Extension(service): Extension<Arc<SysRoleService>>,
        Extension(user): Extension<User>,
        Json(input): Json<UpdateRoleHomeInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        service
            .update_role_home(input, &actor)
            .await
            .map(|_| Res::new_data(true))
    }

    /// W-FW8 US1 transform: GET /systemManage/getAllEndpoints
    /// 回 NTree-friendly tree（按 resource 分組、leaf label = `{summary}（{method}）` 或 fallback `{method} {path}`）。
    pub async fn get_all_endpoints_for_systemmanage(
        Extension(_user): Extension<User>,
    ) -> Result<Res<Vec<EndpointTreeNode>>, AppError> {
        use std::collections::BTreeMap;
        let db = db_helper::get_db_connection().await?;
        let endpoints = sys_endpoint::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;

        let mut by_resource: BTreeMap<String, Vec<sys_endpoint::Model>> = BTreeMap::new();
        for ep in endpoints {
            by_resource.entry(ep.resource.clone()).or_default().push(ep);
        }

        let tree: Vec<EndpointTreeNode> = by_resource
            .into_iter()
            .map(|(resource, mut eps)| {
                eps.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));
                EndpointTreeNode {
                    key: format!("resource:{}", resource),
                    label: resource.clone(),
                    children: Some(
                        eps.into_iter()
                            .map(|ep| {
                                let label = match ep.summary.as_deref() {
                                    Some(s) if !s.is_empty() => format!("{}（{}）", s, ep.method),
                                    _ => format!("{} {}", ep.method, ep.path),
                                };
                                EndpointTreeNode {
                                    // 039: NTree key 仍為 String，但 base-web 對外傳遞 endpoint id 已改 numeric display_id
                                    key: ep.display_id.to_string(),
                                    label,
                                    children: None,
                                    method: Some(ep.method.clone()),
                                    path: Some(ep.path.clone()),
                                    is_leaf: true,
                                }
                            })
                            .collect(),
                    ),
                    method: None,
                    path: None,
                    is_leaf: false,
                }
            })
            .collect();

        Ok(Res::new_data(tree))
    }

    /// W-FW8 US1 transform: GET /systemManage/getRoleEndpointIds/:roleId
    /// 反查 role 已分配 endpoint.id 集合（從 Casbin policy v2/v3 reverse-map）。
    /// 039 T024: Path<String> → Path<i64> + role_svc.lookup_ulid_by_display_id cascade。
    pub async fn get_role_endpoint_ids_for_systemmanage(
        Path(role_display_id): Path<i64>,
        Extension(user): Extension<User>,
        Extension(role_service): Extension<Arc<SysRoleService>>,
        Extension(mut cache_enforcer): Extension<CasbinAxumLayer>,
    ) -> Result<Res<Vec<String>>, AppError> {
        use std::collections::HashMap;

        // 1. validate role + get role.code (RoleNotFound → 4001 既有 behavior)
        let role_ulid = role_service.lookup_ulid_by_display_id(role_display_id).await?;
        let role = role_service.get_role(&role_ulid).await?;
        let role_code = role.code;
        let domain = user.domain().to_string();

        // 2. fetch Casbin policy rows for this role/domain
        let enforcer = cache_enforcer.get_enforcer();
        let enforcer_read = enforcer.read().await;
        let policies = enforcer_read.get_filtered_policy(0, vec![role_code, domain]);
        drop(enforcer_read);

        // 3. build (path, method) → endpoint.display_id(string) map (R-Q3 in-memory optimization)
        // 039: base-web 端對 endpoint id 統一改 numeric display_id（EndpointTreeNode.key = display_id.to_string()），
        // 所以反查結果也須回 display_id 字串而非 ULID，前端才能 match prop 的 checked keys。
        let db = db_helper::get_db_connection().await?;
        let endpoints = sys_endpoint::find_active()
            .all(db.as_ref())
            .await
            .map_err(AppError::from)?;
        let mut path_method_to_id: HashMap<(String, String), String> = HashMap::new();
        for ep in &endpoints {
            path_method_to_id.insert(
                (ep.path.clone(), ep.method.clone()),
                ep.display_id.to_string(),
            );
        }

        // 4. reverse-map policies → endpoint.ids, dedup + sort
        let mut ids: Vec<String> = policies
            .into_iter()
            .filter_map(|p| {
                let v2 = p.get(2)?.clone();
                let v3 = p.get(3)?.clone();
                path_method_to_id.get(&(v2, v3)).cloned()
            })
            .collect();
        ids.sort();
        ids.dedup();

        Ok(Res::new_data(ids))
    }

    /// W-FW8 US1 transform: POST /systemManage/assignRoleEndpoints
    /// 透過既有 assign_permission service 寫 Casbin policy；audit 由 service-side 補（US2 phase）。
    /// 注：T002+T003 已把 assign_permission signature 末尾改 `actor: &Actor`。
    /// 039 T026: input.role_id (i64) + endpoint_ids (Vec<i64>) 先 lookup ULID 再走 service。
    pub async fn assign_role_endpoints_for_systemmanage(
        Extension(user): Extension<User>,
        Extension(service): Extension<Arc<SysAuthorizationService>>,
        Extension(role_svc): Extension<Arc<SysRoleService>>,
        Extension(endpoint_svc): Extension<Arc<SysEndpointService>>,
        Extension(mut cache_enforcer): Extension<CasbinAxumLayer>,
        Json(input): Json<SystemManageAssignRoleEndpointsInput>,
    ) -> Result<Res<bool>, AppError> {
        let actor = Actor::from(&user);
        let domain = user.domain().to_string();
        let enforcer = cache_enforcer.get_enforcer();

        let role_ulid = role_svc.lookup_ulid_by_display_id(input.role_id).await?;
        let mut endpoint_ulids: Vec<String> = Vec::with_capacity(input.endpoint_ids.len());
        for ep_id in &input.endpoint_ids {
            endpoint_ulids.push(endpoint_svc.lookup_ulid_by_display_id(*ep_id).await?);
        }

        service
            .assign_permission(domain, role_ulid, endpoint_ulids, enforcer, &actor)
            .await
            .map(|_| Res::new_data(true))
    }
}

/// W-FW5 T010: 密碼空值正規化 — `None` 與 `Some("")`（空字串）皆視為「未提供」。
fn normalize_password(password: Option<String>) -> Option<String> {
    password.filter(|p| !p.is_empty())
}

fn map_status(s: &str) -> Result<Status, AppError> {
    match s {
        "1" => Ok(Status::Enabled),
        "2" => Ok(Status::Disabled),
        _ => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid status value: {}", s),
        }),
    }
}

fn map_gender(s: Option<&str>) -> Result<Option<Gender>, AppError> {
    match s {
        None => Ok(None),
        Some("1") => Ok(Some(Gender::Male)),
        Some("2") => Ok(Some(Gender::Female)),
        Some(other) => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid gender value: {}", other),
        }),
    }
}

fn map_menu_type(s: &str) -> Result<MenuType, AppError> {
    match s {
        "1" => Ok(MenuType::Directory),
        "2" => Ok(MenuType::Menu),
        _ => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid menu_type value: {}", s),
        }),
    }
}

fn map_icon_type(s: Option<&str>) -> Result<Option<i32>, AppError> {
    match s {
        None => Ok(None),
        Some("1") => Ok(Some(1)),
        Some("2") => Ok(Some(2)),
        Some(other) => Err(AppError {
            code: code::CODE_VALIDATION_FORMAT_INVALID,
            message: format!("Invalid icon_type value: {}", other),
        }),
    }
}
