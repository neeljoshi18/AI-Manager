//! In-process graph fixtures for verify battery (no V2 process required).

use crate::graph_source::GraphSource;
use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use twin_core::model::*;
use twin_core::TwinResult;

pub struct FixtureGraphSource {
    /// key: (tenant_id, global_user_id)
    views: RwLock<HashMap<(String, String), GraphView>>,
    default: GraphView,
}

impl FixtureGraphSource {
    pub fn new(default: GraphView) -> Arc<Self> {
        Arc::new(Self {
            views: RwLock::new(HashMap::new()),
            default,
        })
    }

    pub fn empty() -> Arc<Self> {
        Self::new(GraphView::default())
    }

    pub fn set_view(&self, tenant_id: &str, global_user_id: &str, view: GraphView) {
        self.views
            .write()
            .insert((tenant_id.into(), global_user_id.into()), view);
    }
}

#[async_trait]
impl GraphSource for FixtureGraphSource {
    async fn fetch_person_view(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        _person_node_id: &str,
        _hops: usize,
    ) -> TwinResult<GraphView> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        if let Some(v) = self.views.read().get(&key) {
            return Ok(v.clone());
        }
        Ok(self.default.clone())
    }
}

/// TC-T01 fixture: open PR with evidence for alice; private PR for eng group only
/// is simply not present when view is "as alice without eng" (compiler never sees it).
pub fn alice_open_pr_fixture(tenant: &str) -> GraphView {
    let _ = tenant;
    let as_of = Utc::now();
    GraphView {
        nodes: vec![
            GraphNodeView {
                node_id: "person:gu_alice".into(),
                node_type: "Person".into(),
                display_name: "Alice".into(),
                resource_id: "gu_alice".into(),
                properties: serde_json::json!({}),
                is_private: false,
            },
            GraphNodeView {
                node_id: "pr:acme/app/pr/7".into(),
                node_type: "PullRequest".into(),
                display_name: "fix auth race".into(),
                resource_id: "acme/app/pr/7".into(),
                properties: serde_json::json!({"title": "fix auth race"}),
                is_private: false,
            },
            GraphNodeView {
                node_id: "repo:acme/app".into(),
                node_type: "Repo".into(),
                display_name: "acme/app".into(),
                resource_id: "acme/app".into(),
                properties: serde_json::json!({}),
                is_private: false,
            },
        ],
        edges: vec![
            GraphEdgeView {
                edge_id: "authored:alice:pr7".into(),
                edge_type: "AUTHORED".into(),
                from_node_id: "person:gu_alice".into(),
                to_node_id: "pr:acme/app/pr/7".into(),
                event_id: "evt_123".into(),
                properties: serde_json::json!({}),
                is_private: false,
            },
            GraphEdgeView {
                edge_id: "belongs:pr7:repo".into(),
                edge_type: "BELONGS_TO".into(),
                from_node_id: "pr:acme/app/pr/7".into(),
                to_node_id: "repo:acme/app".into(),
                event_id: "evt_123".into(),
                properties: serde_json::json!({}),
                is_private: false,
            },
        ],
        states: vec![EntityStateView {
            node_id: "pr:acme/app/pr/7".into(),
            state_key: "lifecycle".into(),
            state_value: "OPEN".into(),
            event_id: "evt_123".into(),
            as_of,
        }],
        blockers: vec![],
        graph_as_of: Some(as_of),
    }
}

/// High confidence: merged PR.
pub fn alice_merged_pr_fixture() -> GraphView {
    let mut v = alice_open_pr_fixture("ten");
    if let Some(s) = v.states.get_mut(0) {
        s.state_value = "MERGED".into();
        s.event_id = "evt_merged".into();
    }
    v
}

/// Blocker: open BLOCKS edge.
pub fn alice_blocker_fixture() -> GraphView {
    let mut v = alice_open_pr_fixture("ten");
    v.nodes.push(GraphNodeView {
        node_id: "issue:acme/app/issues/9".into(),
        node_type: "Issue".into(),
        display_name: "API key rotation".into(),
        resource_id: "acme/app/issues/9".into(),
        properties: serde_json::json!({"title": "API key rotation"}),
        is_private: false,
    });
    v.blockers.push(GraphEdgeView {
        edge_id: "blocks:9:pr7".into(),
        edge_type: "BLOCKS".into(),
        from_node_id: "issue:acme/app/issues/9".into(),
        to_node_id: "pr:acme/app/pr/7".into(),
        event_id: "evt_block".into(),
        properties: serde_json::json!({"summary": "Blocked on API key rotation"}),
        is_private: false,
    });
    v.edges.push(v.blockers[0].clone());
    v
}

/// ACL: view for user without eng group — private PR omitted (V2 would filter).
pub fn bob_no_private_pr_fixture() -> GraphView {
    GraphView {
        nodes: vec![GraphNodeView {
            node_id: "person:gu_bob".into(),
            node_type: "Person".into(),
            display_name: "Bob".into(),
            resource_id: "gu_bob".into(),
            properties: serde_json::json!({}),
            is_private: false,
        }],
        edges: vec![],
        states: vec![],
        blockers: vec![],
        graph_as_of: Some(Utc::now()),
    }
}

/// Private PR present only in eng-visible view (for contrast in TC-T06).
pub fn alice_with_private_pr_fixture() -> GraphView {
    let mut v = alice_open_pr_fixture("ten");
    v.nodes.push(GraphNodeView {
        node_id: "pr:acme/secret/pr/1".into(),
        node_type: "PullRequest".into(),
        display_name: "secret".into(),
        resource_id: "acme/secret/pr/1".into(),
        properties: serde_json::json!({"title": "secret work"}),
        is_private: true,
    });
    v.edges.push(GraphEdgeView {
        edge_id: "authored:alice:secret".into(),
        edge_type: "AUTHORED".into(),
        from_node_id: "person:gu_alice".into(),
        to_node_id: "pr:acme/secret/pr/1".into(),
        event_id: "evt_secret".into(),
        properties: serde_json::json!({}),
        is_private: true,
    });
    v.states.push(EntityStateView {
        node_id: "pr:acme/secret/pr/1".into(),
        state_key: "lifecycle".into(),
        state_value: "OPEN".into(),
        event_id: "evt_secret".into(),
        as_of: Utc::now(),
    });
    v
}
