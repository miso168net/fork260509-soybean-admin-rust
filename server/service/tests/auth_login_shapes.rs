//! T011 — Unit tests: serde camelCase serialization of output structs + tree_builder behavior.
//!
//! Covers:
//!   - AuthOutput, UserInfoOutput, UserRoute  (server_model::admin::output::sys_authentication)
//!   - MenuRoute, RouteMeta                   (server_model::admin::output::sys_menu)
//!   - TreeBuilder::build behavior via MenuRoute (server_utils::TreeBuilder)
//!     └── TreeBuilder found at: rust-api/server/utils/src/tree_util.rs:47

use server_model::admin::output::{AuthOutput, MenuRoute, RouteMeta, UserInfoOutput, UserRoute};
use server_utils::TreeBuilder;

// ── helpers ─────────────────────────────────────────────────────────────────

fn make_route_meta(order: i32) -> RouteMeta {
    RouteMeta {
        title: "Test".to_string(),
        i18n_key: None,
        keep_alive: None,
        constant: false,
        icon: None,
        order,
        href: None,
        hide_in_menu: None,
        active_menu: None,
        multi_tab: None,
        query: None,
        fixed_index_in_tab: None,
    }
}

fn make_menu_route(name: &str, id: i32, pid: &str, order: i32) -> MenuRoute {
    MenuRoute {
        name: name.to_string(),
        path: format!("/{}", name),
        component: "layout.base".to_string(),
        meta: make_route_meta(order),
        children: None,
        id,
        pid: pid.to_string(),
    }
}

// ── serde camelCase tests ────────────────────────────────────────────────────

/// T011-1: AuthOutput camelCase — refresh_token must appear as refreshToken.
#[test]
fn auth_output_serde_camelcase() {
    let output = AuthOutput {
        token: "x".to_string(),
        refresh_token: "y".to_string(),
    };
    let value = serde_json::to_value(&output).expect("serialize");

    // positive: camelCase keys present with correct values
    assert_eq!(value["token"], "x");
    assert_eq!(value["refreshToken"], "y");

    // negative: snake_case key must NOT appear
    assert!(
        value.get("refresh_token").is_none(),
        "snake_case 'refresh_token' must not appear in JSON output"
    );
}

/// T011-2: UserInfoOutput camelCase — user_id → userId, user_name → userName.
#[test]
fn user_info_output_serde_camelcase() {
    let output = UserInfoOutput {
        user_id: "uid-1".to_string(),
        user_name: "alice".to_string(),
        roles: vec!["R_SUPER".to_string()],
        buttons: vec![],
    };
    let value = serde_json::to_value(&output).expect("serialize");

    // positive
    assert_eq!(value["userId"], "uid-1");
    assert_eq!(value["userName"], "alice");
    assert_eq!(value["roles"][0], "R_SUPER");
    assert_eq!(value["buttons"], serde_json::json!([]));

    // negative
    assert!(value.get("user_id").is_none(), "snake_case 'user_id' must not appear");
    assert!(value.get("user_name").is_none(), "snake_case 'user_name' must not appear");
}

/// T011-3: UserRoute camelCase — routes and home are already lowercase, confirm
/// they serialize with the correct key names and types.
#[test]
fn user_route_serde_camelcase() {
    let output = UserRoute {
        routes: vec![],
        home: "home".to_string(),
    };
    let value = serde_json::to_value(&output).expect("serialize");

    // positive
    assert_eq!(value["home"], "home");
    assert!(value["routes"].is_array());
    assert_eq!(value["routes"].as_array().unwrap().len(), 0);
}

/// T011-4: MenuRoute camelCase — all fields present; children omitted when None
/// (skip_serializing_if = "Option::is_none").
#[test]
fn menu_route_serde_camelcase() {
    let route = MenuRoute {
        name: "dashboard".to_string(),
        path: "/dashboard".to_string(),
        component: "layout.base".to_string(),
        meta: make_route_meta(1),
        children: None, // should be absent in JSON (skip_serializing_if)
        id: 10,
        pid: "0".to_string(),
    };
    let value = serde_json::to_value(&route).expect("serialize");

    // positive: all present fields serialize with the right keys
    assert_eq!(value["name"], "dashboard");
    assert_eq!(value["path"], "/dashboard");
    assert_eq!(value["component"], "layout.base");
    assert!(value["meta"].is_object());
    assert_eq!(value["id"], 10);
    assert_eq!(value["pid"], "0");

    // children is None → must be absent (skip_serializing_if)
    assert!(
        value.get("children").is_none(),
        "'children' must be absent from JSON when None"
    );
}

/// T011-5: RouteMeta camelCase — snake_case fields must appear as camelCase.
/// i18n_key→i18nKey, keep_alive→keepAlive, hide_in_menu→hideInMenu,
/// active_menu→activeMenu, multi_tab→multiTab.
#[test]
fn route_meta_serde_camelcase() {
    let meta = RouteMeta {
        title: "Dashboard".to_string(),
        i18n_key: Some("route.dashboard".to_string()),
        keep_alive: Some(true),
        constant: false,
        icon: Some("mdi:home".to_string()),
        order: 1,
        href: Some("https://example.com".to_string()),
        hide_in_menu: Some(false),
        active_menu: Some("dashboard".to_string()),
        multi_tab: Some(true),
        query: None,
        fixed_index_in_tab: None,
    };
    let value = serde_json::to_value(&meta).expect("serialize");

    // positive: camelCase keys with correct values
    assert_eq!(value["title"], "Dashboard");
    assert_eq!(value["i18nKey"], "route.dashboard");
    assert_eq!(value["keepAlive"], true);
    assert_eq!(value["constant"], false);
    assert_eq!(value["icon"], "mdi:home");
    assert_eq!(value["order"], 1);
    assert_eq!(value["href"], "https://example.com");
    assert_eq!(value["hideInMenu"], false);
    assert_eq!(value["activeMenu"], "dashboard");
    assert_eq!(value["multiTab"], true);

    // negative: snake_case keys must NOT appear
    assert!(value.get("i18n_key").is_none(), "snake_case 'i18n_key' must not appear");
    assert!(value.get("keep_alive").is_none(), "snake_case 'keep_alive' must not appear");
    assert!(value.get("hide_in_menu").is_none(), "snake_case 'hide_in_menu' must not appear");
    assert!(value.get("active_menu").is_none(), "snake_case 'active_menu' must not appear");
    assert!(value.get("multi_tab").is_none(), "snake_case 'multi_tab' must not appear");
}

// ── TreeBuilder behavior tests (via MenuRoute) ───────────────────────────────
//
// TreeBuilder is public in server_utils (rust-api/server/utils/src/tree_util.rs:47).
// server_utils is a direct dependency of server-service, so it is in scope here.
// We use MenuRoute as the node type, mirroring the actual usage in sys_auth_service.rs:196.
// The id_fn uses route.name.clone() (String-based IDs) and pid "0" means root.

fn build_menu_tree(routes: Vec<MenuRoute>) -> Vec<MenuRoute> {
    // Mirror the pid resolution used in sys_auth_service.rs:196-213:
    // id = route.name (String), parent = resolved by pid "0" → None, else look up by id.
    let routes_ref = routes.clone();
    TreeBuilder::build(
        routes,
        |r| r.name.clone(),
        |r| {
            if r.pid == "0" {
                None
            } else {
                routes_ref
                    .iter()
                    .find(|m| m.id.to_string() == r.pid)
                    .map(|m| m.name.clone())
            }
        },
        |r| r.meta.order,
        |r, children| {
            r.children = Some(children);
        },
    )
}

/// T011-6: TreeBuilder — empty input returns empty output.
#[test]
fn tree_builder_empty() {
    let result = build_menu_tree(vec![]);
    assert!(result.is_empty(), "empty input must yield empty tree");
}

/// T011-7: TreeBuilder — single root node (pid="0") yields 1-element Vec with no children.
#[test]
fn tree_builder_single_root() {
    let nodes = vec![make_menu_route("home", 1, "0", 1)];
    let tree = build_menu_tree(nodes);

    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].name, "home");
    // TreeBuilder only calls set_children_fn when there are children to attach,
    // so children remains None for a leaf root.
    assert!(
        tree[0].children.as_ref().map_or(true, |c| c.is_empty()),
        "root with no children must have None or empty children"
    );
}

/// T011-8: TreeBuilder — three-level hierarchy: root → child → grandchild.
///   root (id=1, pid="0")
///   └── child (id=2, pid="1")
///       └── grandchild (id=3, pid="2")
#[test]
fn tree_builder_multi_level() {
    let nodes = vec![
        make_menu_route("root", 1, "0", 1),
        make_menu_route("child", 2, "1", 1),
        make_menu_route("grandchild", 3, "2", 1),
    ];
    let tree = build_menu_tree(nodes);

    // One root
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].name, "root");

    // Root has one child
    let children = tree[0].children.as_ref().expect("root must have children");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "child");

    // Child has one grandchild
    let grandchildren = children[0].children.as_ref().expect("child must have grandchild");
    assert_eq!(grandchildren.len(), 1);
    assert_eq!(grandchildren[0].name, "grandchild");
}
