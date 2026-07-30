//! HTTP client for Vertical 2 graph-api (ACL QueryContext via user_id).

use crate::graph_source::GraphSource;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use twin_core::model::*;
use twin_core::{TwinError, TwinResult};

pub struct HttpV2GraphSource {
    base_url: String,
    http: Client,
}

impl HttpV2GraphSource {
    pub fn new(base_url: impl Into<String>) -> Arc<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest");
        Arc::new(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
        })
    }
}

#[derive(Debug, Deserialize)]
struct V2Neighborhood {
    #[serde(default)]
    root: Option<V2Node>,
    #[serde(default)]
    nodes: Vec<V2Node>,
    #[serde(default)]
    edges: Vec<V2Edge>,
    #[allow(dead_code)]
    #[serde(default)]
    hops: usize,
}

#[derive(Debug, Deserialize)]
struct V2Node {
    node_id: String,
    #[serde(default)]
    node_type: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    resource_id: String,
    #[serde(default, alias = "properties_json")]
    properties: serde_json::Value,
    #[serde(default)]
    is_private: bool,
}

#[derive(Debug, Deserialize)]
struct V2Edge {
    edge_id: String,
    #[serde(default)]
    edge_type: String,
    from_node_id: String,
    to_node_id: String,
    #[serde(default)]
    event_id: String,
    #[serde(default, alias = "properties_json")]
    properties: serde_json::Value,
    #[serde(default)]
    is_private: bool,
}

#[derive(Debug, Deserialize)]
struct BlockersResp {
    #[serde(default)]
    blockers: Vec<V2Edge>,
}

#[derive(Debug, Deserialize)]
struct StateResp {
    state: Option<V2State>,
}

#[derive(Debug, Deserialize)]
struct V2State {
    node_id: String,
    state_key: String,
    state_value: String,
    #[serde(default)]
    event_id: String,
    #[serde(default)]
    as_of: Option<chrono::DateTime<Utc>>,
}

#[async_trait]
impl GraphSource for HttpV2GraphSource {
    async fn fetch_person_view(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        person_node_id: &str,
        hops: usize,
    ) -> TwinResult<GraphView> {
        // Neighborhood as the person (ACL via user_id)
        let nb_url = format!(
            "{}/v2/tenants/{}/neighborhood?user_id={}&node_id={}&hops={}",
            self.base_url,
            urlencoding_lite(tenant_id),
            urlencoding_lite(global_user_id),
            urlencoding_lite(person_node_id),
            hops
        );
        // 404 (unknown person / empty ACL) → empty view, not hard compile failure.
        // Digests stay empty/non-spam; multi-person team compile continues for others.
        let nb_resp = self
            .http
            .get(&nb_url)
            .send()
            .await
            .map_err(|e| TwinError::Upstream(format!("v2 neighborhood: {e}")))?;
        if nb_resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(GraphView {
                nodes: vec![],
                edges: vec![],
                states: vec![],
                blockers: vec![],
                graph_as_of: Some(Utc::now()),
            });
        }
        let nb: V2Neighborhood = nb_resp
            .error_for_status()
            .map_err(|e| TwinError::Upstream(format!("v2 neighborhood status: {e}")))?
            .json()
            .await
            .map_err(|e| TwinError::Upstream(format!("v2 neighborhood json: {e}")))?;

        let mut nodes: Vec<GraphNodeView> = nb
            .nodes
            .into_iter()
            .map(|n| GraphNodeView {
                node_id: n.node_id,
                node_type: n.node_type,
                display_name: n.display_name,
                resource_id: n.resource_id,
                properties: n.properties,
                is_private: n.is_private,
            })
            .collect();
        if let Some(root) = nb.root {
            if !nodes.iter().any(|n| n.node_id == root.node_id) {
                nodes.push(GraphNodeView {
                    node_id: root.node_id,
                    node_type: root.node_type,
                    display_name: root.display_name,
                    resource_id: root.resource_id,
                    properties: root.properties,
                    is_private: root.is_private,
                });
            }
        }

        let edges: Vec<GraphEdgeView> = nb
            .edges
            .into_iter()
            .map(|e| GraphEdgeView {
                edge_id: e.edge_id,
                edge_type: e.edge_type,
                from_node_id: e.from_node_id,
                to_node_id: e.to_node_id,
                event_id: e.event_id,
                properties: e.properties,
                is_private: e.is_private,
            })
            .collect();

        // Blockers endpoint
        let bl_url = format!(
            "{}/v2/tenants/{}/blockers?user_id={}&node_id={}",
            self.base_url,
            urlencoding_lite(tenant_id),
            urlencoding_lite(global_user_id),
            urlencoding_lite(person_node_id),
        );
        let blockers = match self.http.get(&bl_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: BlockersResp = resp
                    .json()
                    .await
                    .unwrap_or(BlockersResp { blockers: vec![] });
                body.blockers
                    .into_iter()
                    .map(|e| GraphEdgeView {
                        edge_id: e.edge_id,
                        edge_type: e.edge_type,
                        from_node_id: e.from_node_id,
                        to_node_id: e.to_node_id,
                        event_id: e.event_id,
                        properties: e.properties,
                        is_private: e.is_private,
                    })
                    .collect()
            }
            _ => edges
                .iter()
                .filter(|e| {
                    e.edge_type.eq_ignore_ascii_case("BLOCKS")
                        || e.edge_type.eq_ignore_ascii_case("BLOCKED_BY")
                })
                .cloned()
                .collect(),
        };

        // Fetch lifecycle state for PR nodes
        let mut states = Vec::new();
        for n in &nodes {
            if n.node_id.starts_with("pr:")
                || n.node_type.eq_ignore_ascii_case("PullRequest")
                || n.node_id.starts_with("issue:")
            {
                let st_url = format!(
                    "{}/v2/tenants/{}/state?user_id={}&node_id={}&state_key=lifecycle",
                    self.base_url,
                    urlencoding_lite(tenant_id),
                    urlencoding_lite(global_user_id),
                    urlencoding_lite(&n.node_id),
                );
                if let Ok(resp) = self.http.get(&st_url).send().await {
                    if resp.status().is_success() {
                        if let Ok(body) = resp.json::<StateResp>().await {
                            if let Some(s) = body.state {
                                states.push(EntityStateView {
                                    node_id: s.node_id,
                                    state_key: s.state_key,
                                    state_value: s.state_value,
                                    event_id: s.event_id,
                                    as_of: s.as_of.unwrap_or_else(Utc::now),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(GraphView {
            nodes,
            edges,
            states,
            blockers,
            graph_as_of: Some(Utc::now()),
        })
    }
}

fn urlencoding_lite(s: &str) -> String {
    // Minimal encoding for path/query segments
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}
