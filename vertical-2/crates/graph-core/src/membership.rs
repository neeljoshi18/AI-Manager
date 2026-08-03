use crate::error::{GraphError, GraphResult};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::info;

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MembershipPersistSnapshot {
    pub version: u32,
    pub saved_at: Option<String>,
    /// entries: { "tenant|user": ["grp_a", ...] }
    pub users: std::collections::BTreeMap<String, Vec<String>>,
}

pub struct InMemoryMembership {
    /// (tenant, user) → groups
    map: DashMap<(String, String), HashSet<String>>,
    persist_path: RwLock<Option<PathBuf>>,
    dirty: AtomicU64,
}

impl InMemoryMembership {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            map: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
        })
    }

    pub fn set_persist_path(&self, path: Option<PathBuf>) {
        *self.persist_path.write() = path;
    }

    pub fn export_snapshot(&self) -> MembershipPersistSnapshot {
        let mut users = std::collections::BTreeMap::new();
        for e in self.map.iter() {
            let (tenant, user) = e.key();
            let key = format!("{tenant}|{user}");
            let mut groups: Vec<String> = e.value().iter().cloned().collect();
            groups.sort();
            users.insert(key, groups);
        }
        MembershipPersistSnapshot {
            version: 1,
            saved_at: Some(chrono::Utc::now().to_rfc3339()),
            users,
        }
    }

    pub fn import_snapshot(&self, snap: MembershipPersistSnapshot) {
        self.map.clear();
        for (k, groups) in snap.users {
            let mut parts = k.splitn(2, '|');
            let tenant = parts.next().unwrap_or("").to_string();
            let user = parts.next().unwrap_or("").to_string();
            if tenant.is_empty() || user.is_empty() {
                continue;
            }
            self.map
                .insert((tenant, user), groups.into_iter().collect());
        }
    }

    pub fn save_to_path(&self, path: &Path) -> GraphResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GraphError::Storage(format!("membership persist mkdir {}: {e}", parent.display()))
            })?;
        }
        let snap = self.export_snapshot();
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&snap)
            .map_err(|e| GraphError::Storage(format!("membership persist encode: {e}")))?;
        std::fs::write(&tmp, &bytes)
            .map_err(|e| GraphError::Storage(format!("membership persist write: {e}")))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| GraphError::Storage(format!("membership persist rename: {e}")))?;
        self.dirty.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn load_from_path(&self, path: &Path) -> GraphResult<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let bytes = std::fs::read(path)
            .map_err(|e| GraphError::Storage(format!("membership persist read: {e}")))?;
        let snap: MembershipPersistSnapshot = serde_json::from_slice(&bytes)
            .map_err(|e| GraphError::Storage(format!("membership persist decode: {e}")))?;
        let n = snap.users.len();
        self.import_snapshot(snap);
        info!(path = %path.display(), users = n, "loaded embedded V2 membership snapshot");
        Ok(true)
    }

    pub fn force_persist(&self) {
        let path = self.persist_path.read().clone();
        if let Some(p) = path {
            if let Err(e) = self.save_to_path(&p) {
                tracing::warn!(error = %e, "V2 membership persist failed");
            }
        }
    }

    fn maybe_persist(&self) {
        let _ = self.dirty.fetch_add(1, Ordering::Relaxed);
        self.force_persist();
    }

    pub fn user_count(&self) -> usize {
        self.map.len()
    }
}

impl Default for InMemoryMembership {
    fn default() -> Self {
        Self {
            map: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
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
        self.maybe_persist();
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
        self.maybe_persist();
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
        self.maybe_persist();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[tokio::test]
    async fn membership_roundtrip_disk() {
        let m = InMemoryMembership::new();
        m.set_groups("ten_t", "gu_a", &["grp_eng".into(), "grp_default".into()])
            .await
            .unwrap();
        let path = temp_dir().join(format!("v2_memb_{}.json", std::process::id()));
        m.set_persist_path(Some(path.clone()));
        m.force_persist();
        let m2 = InMemoryMembership::new();
        assert!(m2.load_from_path(&path).unwrap());
        let g = m2.get_groups("ten_t", "gu_a").await.unwrap();
        assert!(g.contains(&"grp_eng".to_string()));
        let _ = std::fs::remove_file(path);
    }
}
