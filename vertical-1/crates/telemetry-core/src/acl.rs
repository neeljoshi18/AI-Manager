//! Dynamic ACL mirroring engine (Spec §3.3).
//!
//! - Identity mapping: tenant_id ∥ provider_user_id → global_user_id
//! - GroupMap: global_user_id → {group_id...}
//! - Push-based invalidation on membership change (<200ms target)
//! - Query-time bitwise / array filtering in the analytical store

use crate::error::{CoreError, CoreResult};
use crate::model::{AclRevocationRecord, QueryContext};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// ACL store trait — CockroachDB in production, in-memory for embedded.
#[async_trait]
pub trait AclStore: Send + Sync {
    async fn ensure_user(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
        email: &str,
        display_name: &str,
    ) -> CoreResult<String>;

    async fn resolve_global_user_id(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
    ) -> CoreResult<Option<String>>;

    async fn set_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_ids: &[String],
    ) -> CoreResult<u64>;

    async fn add_user_to_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64>;

    async fn remove_user_from_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64>;

    async fn get_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> CoreResult<Vec<String>>;

    async fn apply_revocation(&self, rev: &AclRevocationRecord) -> CoreResult<u64>;

    /// Subscribe to ACL invalidation notifications (Redis Pub/Sub analogue).
    fn subscribe_invalidations(&self) -> broadcast::Receiver<AclInvalidation>;

    async fn current_acl_version(&self, tenant_id: &str) -> u64;

    async fn build_query_context(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> CoreResult<QueryContext> {
        let group_ids = self.get_user_groups(tenant_id, global_user_id).await?;
        Ok(QueryContext {
            tenant_id: tenant_id.to_string(),
            global_user_id: global_user_id.to_string(),
            group_ids,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AclInvalidation {
    pub tenant_id: String,
    pub global_user_id: String,
    pub acl_version: u64,
}

struct UserRecord {
    #[allow(dead_code)]
    global_user_id: String,
    #[allow(dead_code)]
    email: String,
    #[allow(dead_code)]
    display_name: String,
    groups: HashSet<String>,
}

/// In-memory ACL engine with pub/sub invalidation.
pub struct InMemoryAclStore {
    /// (tenant_id, provider_user_id) → global_user_id
    identity_map: DashMap<(String, String), String>,
    /// (tenant_id, global_user_id) → UserRecord
    users: DashMap<(String, String), RwLock<UserRecord>>,
    /// tenant_id → monotonic acl_version
    versions: DashMap<String, AtomicU64>,
    invalidation_tx: broadcast::Sender<AclInvalidation>,
}

impl InMemoryAclStore {
    pub fn new() -> Arc<Self> {
        let (tx, _) = broadcast::channel(4096);
        Arc::new(Self {
            identity_map: DashMap::new(),
            users: DashMap::new(),
            versions: DashMap::new(),
            invalidation_tx: tx,
        })
    }

    fn bump_version(&self, tenant_id: &str) -> u64 {
        let entry = self
            .versions
            .entry(tenant_id.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn publish(&self, tenant_id: &str, global_user_id: &str, version: u64) {
        let _ = self.invalidation_tx.send(AclInvalidation {
            tenant_id: tenant_id.to_string(),
            global_user_id: global_user_id.to_string(),
            acl_version: version,
        });
    }
}

impl Default for InMemoryAclStore {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self {
            identity_map: DashMap::new(),
            users: DashMap::new(),
            versions: DashMap::new(),
            invalidation_tx: tx,
        }
    }
}

#[async_trait]
impl AclStore for InMemoryAclStore {
    async fn ensure_user(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
        email: &str,
        display_name: &str,
    ) -> CoreResult<String> {
        let key = (tenant_id.to_string(), provider_user_id.to_string());
        if let Some(existing) = self.identity_map.get(&key) {
            return Ok(existing.clone());
        }
        let global_user_id = format!("gu_{}", Uuid::new_v4());
        self.identity_map
            .insert(key, global_user_id.clone());
        self.users.insert(
            (tenant_id.to_string(), global_user_id.clone()),
            RwLock::new(UserRecord {
                global_user_id: global_user_id.clone(),
                email: email.to_string(),
                display_name: display_name.to_string(),
                groups: HashSet::new(),
            }),
        );
        Ok(global_user_id)
    }

    async fn resolve_global_user_id(
        &self,
        tenant_id: &str,
        provider_user_id: &str,
    ) -> CoreResult<Option<String>> {
        Ok(self
            .identity_map
            .get(&(tenant_id.to_string(), provider_user_id.to_string()))
            .map(|v| v.clone()))
    }

    async fn set_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_ids: &[String],
    ) -> CoreResult<u64> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        let entry = self
            .users
            .get(&key)
            .ok_or_else(|| CoreError::NotFound(format!("user {global_user_id}")))?;
        {
            let mut user = entry.write();
            user.groups = group_ids.iter().cloned().collect();
        }
        let version = self.bump_version(tenant_id);
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn add_user_to_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        let entry = self
            .users
            .get(&key)
            .ok_or_else(|| CoreError::NotFound(format!("user {global_user_id}")))?;
        entry.write().groups.insert(group_id.to_string());
        let version = self.bump_version(tenant_id);
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn remove_user_from_group(
        &self,
        tenant_id: &str,
        global_user_id: &str,
        group_id: &str,
    ) -> CoreResult<u64> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        let entry = self
            .users
            .get(&key)
            .ok_or_else(|| CoreError::NotFound(format!("user {global_user_id}")))?;
        entry.write().groups.remove(group_id);
        let version = self.bump_version(tenant_id);
        self.publish(tenant_id, global_user_id, version);
        Ok(version)
    }

    async fn get_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> CoreResult<Vec<String>> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        match self.users.get(&key) {
            Some(entry) => {
                let user = entry.read();
                Ok(user.groups.iter().cloned().collect())
            }
            None => Ok(Vec::new()),
        }
    }

    async fn apply_revocation(&self, rev: &AclRevocationRecord) -> CoreResult<u64> {
        match rev.change_type.as_str() {
            "removed_from_group" => {
                self.remove_user_from_group(&rev.tenant_id, &rev.global_user_id, &rev.group_id)
                    .await
            }
            "added_to_group" => {
                self.add_user_to_group(&rev.tenant_id, &rev.global_user_id, &rev.group_id)
                    .await
            }
            other => Err(CoreError::Validation(format!(
                "unknown ACL change_type: {other}"
            ))),
        }
    }

    fn subscribe_invalidations(&self) -> broadcast::Receiver<AclInvalidation> {
        self.invalidation_tx.subscribe()
    }

    async fn current_acl_version(&self, tenant_id: &str) -> u64 {
        self.versions
            .get(tenant_id)
            .map(|v| v.load(Ordering::Acquire))
            .unwrap_or(0)
    }
}

/// Evaluate whether a query context may see an event's ACL snapshot.
///
/// Spec §3.3.2:
/// ```sql
/// is_private = false OR hasAny(allowed_group_ids, user_groups)
/// ```
pub fn acl_allows(ctx: &QueryContext, is_private: bool, allowed_group_ids: &[String]) -> bool {
    if ctx.tenant_id.is_empty() {
        return false;
    }
    if !is_private {
        return true;
    }
    if allowed_group_ids.is_empty() {
        // Private with empty allow-list: only visible if we treat empty as deny-all.
        return false;
    }
    let user_groups: HashSet<&str> = ctx.group_ids.iter().map(|s| s.as_str()).collect();
    allowed_group_ids
        .iter()
        .any(|g| user_groups.contains(g.as_str()))
}

/// Resolve groups for a resource from provider payload conventions.
pub fn groups_from_payload(
    explicit: Option<Vec<String>>,
    defaults: &[String],
) -> Vec<String> {
    match explicit {
        Some(g) if !g.is_empty() => g,
        _ => defaults.to_vec(),
    }
}

/// Seed helper used by onboarding / tests.
pub async fn seed_membership(
    store: &dyn AclStore,
    tenant_id: &str,
    provider_user_id: &str,
    email: &str,
    display_name: &str,
    groups: &[&str],
) -> CoreResult<String> {
    let gid = store
        .ensure_user(tenant_id, provider_user_id, email, display_name)
        .await?;
    let group_ids: Vec<String> = groups.iter().map(|s| s.to_string()).collect();
    store
        .set_user_groups(tenant_id, &gid, &group_ids)
        .await?;
    Ok(gid)
}

/// Cache layer: read-through group membership with invalidation.
pub struct AclCache {
    inner: Arc<dyn AclStore>,
    cache: DashMap<(String, String), Vec<String>>,
}

impl AclCache {
    pub fn new(inner: Arc<dyn AclStore>) -> Arc<Self> {
        let cache = DashMap::new();
        let this = Arc::new(Self {
            inner: inner.clone(),
            cache,
        });
        // Background invalidation listener
        let this_bg = Arc::clone(&this);
        tokio::spawn(async move {
            let mut rx = this_bg.inner.subscribe_invalidations();
            while let Ok(inv) = rx.recv().await {
                this_bg
                    .cache
                    .remove(&(inv.tenant_id, inv.global_user_id));
            }
        });
        this
    }

    pub async fn get_user_groups(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> CoreResult<Vec<String>> {
        let key = (tenant_id.to_string(), global_user_id.to_string());
        if let Some(v) = self.cache.get(&key) {
            return Ok(v.clone());
        }
        let groups = self.inner.get_user_groups(tenant_id, global_user_id).await?;
        self.cache.insert(key, groups.clone());
        Ok(groups)
    }

    pub fn store(&self) -> &Arc<dyn AclStore> {
        &self.inner
    }
}

/// Export identity map for admin / debug.
pub fn dump_identity_count(store: &InMemoryAclStore) -> usize {
    store.identity_map.len()
}

pub fn dump_group_map(store: &InMemoryAclStore, tenant_id: &str) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for entry in store.users.iter() {
        let (t, uid) = entry.key();
        if t == tenant_id {
            let user = entry.value().read();
            out.insert(uid.clone(), user.groups.iter().cloned().collect());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn revocation_removes_access() {
        let store = InMemoryAclStore::new();
        let uid = seed_membership(
            store.as_ref(),
            "ten_1",
            "gh_1",
            "a@x.com",
            "Alice",
            &["grp_eng", "grp_sec"],
        )
        .await
        .unwrap();

        let groups = store.get_user_groups("ten_1", &uid).await.unwrap();
        assert!(groups.contains(&"grp_eng".to_string()));

        store
            .remove_user_from_group("ten_1", &uid, "grp_eng")
            .await
            .unwrap();
        let groups = store.get_user_groups("ten_1", &uid).await.unwrap();
        assert!(!groups.contains(&"grp_eng".to_string()));
        assert!(groups.contains(&"grp_sec".to_string()));
    }

    #[test]
    fn acl_filter_private() {
        let ctx = QueryContext {
            tenant_id: "t".into(),
            global_user_id: "u".into(),
            group_ids: vec!["eng".into()],
        };
        assert!(acl_allows(&ctx, false, &[]));
        assert!(acl_allows(&ctx, true, &["eng".into()]));
        assert!(!acl_allows(&ctx, true, &["sec".into()]));
        assert!(!acl_allows(&ctx, true, &[]));
    }
}
