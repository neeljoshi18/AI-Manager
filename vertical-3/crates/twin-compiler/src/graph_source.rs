use async_trait::async_trait;
use twin_core::model::GraphView;
use twin_core::TwinResult;

/// ACL-scoped graph data for a person. Implementations must not bypass V2 ACL.
#[async_trait]
pub trait GraphSource: Send + Sync {
    async fn fetch_person_view(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        person_node_id: &str,
        hops: usize,
    ) -> TwinResult<GraphView>;
}
