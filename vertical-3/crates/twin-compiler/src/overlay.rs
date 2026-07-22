//! Prefer explicit fixtures (demo/tests); otherwise call live V2 HTTP.

use crate::fixtures::FixtureGraphSource;
use crate::graph_source::GraphSource;
use crate::http_v2::HttpV2GraphSource;
use async_trait::async_trait;
use std::sync::Arc;
use twin_core::model::GraphView;
use twin_core::TwinResult;

/// Fixture wins when a view was injected for (tenant, user); else V2 ACL APIs.
pub struct OverlayGraphSource {
    fixture: Arc<FixtureGraphSource>,
    http: Arc<HttpV2GraphSource>,
}

impl OverlayGraphSource {
    pub fn new(fixture: Arc<FixtureGraphSource>, v2_base_url: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            fixture: fixture.clone(),
            http: HttpV2GraphSource::new(v2_base_url),
        })
    }

    pub fn fixture(&self) -> Arc<FixtureGraphSource> {
        self.fixture.clone()
    }
}

#[async_trait]
impl GraphSource for OverlayGraphSource {
    async fn fetch_person_view(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        person_node_id: &str,
        hops: usize,
    ) -> TwinResult<GraphView> {
        let view = self
            .fixture
            .fetch_person_view(tenant_id, global_user_id, person_node_id, hops)
            .await?;
        // Empty fixture (default) → live V2
        if view.nodes.is_empty() && view.edges.is_empty() && view.blockers.is_empty() {
            return self
                .http
                .fetch_person_view(tenant_id, global_user_id, person_node_id, hops)
                .await;
        }
        Ok(view)
    }
}
