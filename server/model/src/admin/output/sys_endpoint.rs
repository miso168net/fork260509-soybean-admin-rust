use chrono::NaiveDateTime;
use serde::Serialize;

use crate::admin::entities::sys_endpoint;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EndpointTree {
    pub id: i64,
    pub path: String,
    pub method: String,
    pub action: String,
    pub resource: String,
    pub controller: String,
    pub summary: Option<String>,
    pub children: Option<Vec<EndpointTree>>,
}

/// W-FW8 US1: NTree-friendly shape for base-web button-auth-modal.
/// `key` = resource group id (`resource:<name>`) or endpoint id (leaf).
/// `label` = group resource name (group) or `"{summary}（{method}）"` / fallback `"{method} {path}"` (leaf).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointTreeNode {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<EndpointTreeNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub is_leaf: bool,
}

/// 052 wire-shape-leak-fix D-pattern extension: raw endpoint output wire DTO for sys_endpoint (paginated).
/// 隔離 Sea-ORM Model 與 wire 表示；wire 上 `id: i64`、無 `displayId` 重複欄、無 `deletedAt`。
/// Note: sys_endpoint::Model 本身就無 `created_by`/`updated_by`（endpoint catalog 為 system seed、不追作者）。
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointDetail {
    pub id: i64,
    pub path: String,
    pub method: String,
    pub action: String,
    pub resource: String,
    pub controller: String,
    pub summary: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

impl From<sys_endpoint::Model> for EndpointDetail {
    fn from(m: sys_endpoint::Model) -> Self {
        Self {
            id: m.display_id,
            path: m.path,
            method: m.method,
            action: m.action,
            resource: m.resource,
            controller: m.controller,
            summary: m.summary,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}
