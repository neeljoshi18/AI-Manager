use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub tenant_id: String,
    pub node_id: String,
    pub node_type: String,
    pub display_name: String,
    pub resource_id: String,
    pub properties: JsonValue,
    pub is_private: bool,
    pub allowed_group_ids: Vec<String>,
    pub acl_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdge {
    pub tenant_id: String,
    pub edge_id: String,
    pub edge_type: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub valid_from: DateTime<Utc>,
    pub valid_to: Option<DateTime<Utc>>,
    pub event_id: String,
    pub properties: JsonValue,
    pub is_private: bool,
    pub allowed_group_ids: Vec<String>,
    pub acl_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityState {
    pub tenant_id: String,
    pub node_id: String,
    pub state_key: String,
    pub state_value: String,
    pub as_of: DateTime<Utc>,
    pub event_id: String,
    pub is_private: bool,
    pub allowed_group_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryContext {
    pub tenant_id: String,
    pub global_user_id: String,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neighborhood {
    pub root: GraphNode,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub hops: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPath {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOutcome {
    pub event_id: String,
    pub tenant_id: String,
    pub status: ProjectStatus,
    pub nodes_upserted: usize,
    pub edges_upserted: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Applied,
    Duplicate,
    Skipped,
}

/// Mutations produced by a single event mapping.
#[derive(Debug, Clone, Default)]
pub struct GraphMutation {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub states: Vec<EntityState>,
}
