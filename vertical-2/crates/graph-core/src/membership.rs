use crate::error::{GraphError, GraphResult};
use async_trait::async_trait;
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;

#[async_trait]
pub trait MembershipStore: Send + Sync {
    async fn set_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        groups: &[String],
    ) -> GraphResult<()>;

    async fn add_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()>;

    async fn remove_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()>;

    async fn get_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> GraphResult<Vec<String>>;
}

pub struct InMemoryMembership {
    /// (tenant, user) → groups
    map: DashMap<(String, String), HashSet<String>>,
}

impl InMemoryMembership {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            map: DashMap::new(),
        })
    }
}

impl Default for InMemoryMembership {
    fn default() -> Self {
        Self {
            map: DashMap::new(),
        }
    }
}

#[async_trait]
impl MembershipStore for InMemoryMembership {
    async fn set_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        groups: &[String],
    ) -> GraphResult<()> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        self.map
            .insert(key, groups.iter().cloned().collect());
        Ok(())
    }

    async fn add_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        self.map
            .entry(key)
            .or_insert_with(HashSet::new)
            .insert(group_id.to_string());
        Ok(())
    }

    async fn remove_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> GraphResult<()> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        if let Some(mut g) = self.map.get_mut(&key) {
            g.remove(group_id);
        }
        Ok(())
    }

    async fn get_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> GraphResult<Vec<String>> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        Ok(self
            .map
            .get(&key)
            .map(|g| g.iter().cloned().collect())
            .unwrap_or_default())
    }
}

/// Apply ACL-style membership change from V1 identity/ACL events.
pub async fn apply_membership_change(
    store: &dyn MembershipStore,
    tenant_id: &str,
    global_user_id: &str,
    group_id: &str,
    change_type: &str,
) -> GraphResult<()> {
    if global_user_id.is_empty() || group_id.is_empty() {
        return Err(GraphError::Validation(
            "membership change requires user and group".into(),
        ));
    }
    match change_type {
        "removed_from_group" => store.remove_group(tenant_id, global_user_id, group_id).await,
        "added_to_group" => store.add_group(tenant_id, global_user_id, group_id).await,
        other => Err(GraphError::Validation(format!(
            "unknown membership change_type: {other}"
        ))),
    }
}
