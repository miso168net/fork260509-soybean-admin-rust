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
