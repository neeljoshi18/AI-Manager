//! Live membership reader: prefers Vertical 1 Cockroach `user_group_membership`
//! so ACL revocation in V1 immediately applies to V2 graph reads (no dual-write lag).

use crate::error::{GraphError, GraphResult};
use crate::membership::MembershipStore;
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;

/// Hybrid membership: writes go to local store; reads prefer V1 identity tables when configured.
pub struct HybridMembership {
    local: Arc<dyn MembershipStore>,
    v1_pool: Option<PgPool>,
}

impl HybridMembership {
    pub fn local_only(local: Arc<dyn MembershipStore>) -> Arc<Self> {
        Arc::new(Self {
            local,
            v1_pool: None,
        })
    }

    pub async fn with_v1_identity(
        local: Arc<dyn MembershipStore>,
        v1_database_url: &str,
    ) -> GraphResult<Arc<Self>> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(v1_database_url)
            .await
            .map_err(|e| GraphError::Storage(format!("v1 identity connect: {e}")))?;
        Ok(Arc::new(Self {
            local,
            v1_pool: Some(pool),
        }))
    }
}

#[async_trait]
impl MembershipStore for HybridMembership {
    async fn set_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        groups: &[String],
    ) -> GraphResult<()> {
        self.local
            .set_groups(tenant_id, global_user_id, groups)
            .await
    }

    async fn add_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        self.local
            .add_group(tenant_id, global_user_id, group_id)
            .await
    }

    async fn remove_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        self.local
            .remove_group(tenant_id, global_user_id, group_id)
            .await
    }

    async fn get_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> GraphResult<Vec<String>> {
        if let Some(pool) = &self.v1_pool {
            let rows: Result<Vec<(String,)>, _> = sqlx::query_as(
                r#"
                SELECT group_id FROM user_group_membership
                WHERE tenant_id = $1 AND global_user_id = $2
                "#,
            )
            .bind(tenant_id)
            .bind(global_user_id)
            .fetch_all(pool)
            .await;
            match rows {
                Ok(r) => return Ok(r.into_iter().map(|x| x.0).collect()),
                Err(e) => {
                    // Fall back to local if V1 schema not reachable
                    tracing::warn!(error = %e, "v1 membership read failed; falling back to local");
                }
            }
        }
        self.local.get_groups(tenant_id, global_user_id).await
    }
}
