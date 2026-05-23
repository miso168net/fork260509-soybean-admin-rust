use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EndpointTree {
    pub id: String,
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
