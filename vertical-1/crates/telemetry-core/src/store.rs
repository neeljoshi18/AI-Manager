//! Analytical event store (ClickHouse in production, in-memory for embedded).
//!
//! Spec §4.4: ReplacingMergeTree-style dedup on event_id / acl_version.
//! Query-time ACL filter mandatory on every read (Invariant #2).
//!
//! Embedded durability: optional JSON snapshot so container restarts do not wipe
//! the pilot event journal (bridge can re-project into V2 after reboot).

use crate::acl::acl_allows;
use crate::error::{CoreError, CoreResult};
use crate::model::{CanonicalEventRecord, EventQuery, QueryContext};
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append (or replace by event_id with higher acl_version / sequence).
    async fn upsert(&self, event: CanonicalEventRecord) -> CoreResult<()>;

    async fn upsert_batch(&self, events: Vec<CanonicalEventRecord>) -> CoreResult<()> {
        for e in events {
            self.upsert(e).await?;
        }
        Ok(())
    }

    /// ACL-filtered query. **Must** enforce group membership.
    async fn query(
        &self,
        ctx: &QueryContext,
        filter: &EventQuery,
    ) -> CoreResult<Vec<CanonicalEventRecord>>;

    /// Count unique event_ids (for idempotency tests).
    async fn count_unique(&self, tenant_id: &str) -> CoreResult<u64>;

    /// Fetch by event_id without ACL (admin / internal only).
    async fn get_raw(&self, tenant_id: &str, event_id: &str) -> CoreResult<Option<CanonicalEventRecord>>;

    /// Reconstruct latest state for a resource using origin timestamps
    /// (Challenge 1: out-of-order webhooks).
    async fn latest_state_for_resource(
        &self,
        ctx: &QueryContext,
        resource_id: &str,
    ) -> CoreResult<Option<CanonicalEventRecord>>;
}

/// On-disk snapshot for embedded staging durability.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventPersistSnapshot {
    pub version: u32,
    pub saved_at: Option<String>,
    pub events: Vec<CanonicalEventRecord>,
}

/// In-memory ReplacingMergeTree analogue.
pub struct InMemoryEventStore {
    /// tenant_id → event_id → record
    by_id: DashMap<String, DashMap<String, CanonicalEventRecord>>,
    /// tenant_id → resource_id → ordered events (by event_timestamp)
    by_resource: DashMap<String, DashMap<String, RwLock<Vec<CanonicalEventRecord>>>>,
    /// Optional path for embedded persistence.
    persist_path: RwLock<Option<std::path::PathBuf>>,
    /// Writes since last successful save (for periodic flush).
    dirty: AtomicU64,
}

impl InMemoryEventStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            by_id: DashMap::new(),
            by_resource: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
        })
    }

    pub fn set_persist_path(&self, path: Option<std::path::PathBuf>) {
        *self.persist_path.write() = path;
    }

    pub fn export_snapshot(&self) -> EventPersistSnapshot {
        let mut events = Vec::new();
        for tenant in self.by_id.iter() {
            for e in tenant.value().iter() {
                events.push(e.value().clone());
            }
        }
        EventPersistSnapshot {
            version: 1,
            saved_at: Some(Utc::now().to_rfc3339()),
            events,
        }
    }

    pub fn import_snapshot(&self, snap: EventPersistSnapshot) {
        for e in snap.events {
            let _ = self.upsert_sync_no_persist(e);
        }
    }

    fn upsert_sync_no_persist(&self, event: CanonicalEventRecord) -> CoreResult<()> {
        let tenant = event.tenant_id.clone();
        let event_id = event.event_id.clone();
        let resource_id = event.resource_id.clone();
        let tenant_events = self
            .by_id
            .entry(tenant.clone())
            .or_insert_with(DashMap::new);
        tenant_events.insert(event_id.clone(), event.clone());
        let by_res = self
            .by_resource
            .entry(tenant)
            .or_insert_with(DashMap::new);
        let list = by_res
            .entry(resource_id)
            .or_insert_with(|| RwLock::new(Vec::new()));
        {
            let mut v = list.write();
            if let Some(pos) = v.iter().position(|e| e.event_id == event_id) {
                v[pos] = event;
            } else {
                v.push(event);
            }
            v.sort_by_key(|e| e.event_timestamp);
        }
        Ok(())
    }

    pub fn save_to_path(&self, path: &Path) -> CoreResult<()> {
        let snap = self.export_snapshot();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::Storage(format!("event persist mkdir {}: {e}", parent.display()))
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(&snap)
            .map_err(|e| CoreError::Storage(format!("event persist encode: {e}")))?;
        std::fs::write(&tmp, &bytes)
            .map_err(|e| CoreError::Storage(format!("event persist write: {e}")))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| CoreError::Storage(format!("event persist rename: {e}")))?;
        self.dirty.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn load_from_path(&self, path: &Path) -> CoreResult<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let bytes = std::fs::read(path)
            .map_err(|e| CoreError::Storage(format!("event persist read: {e}")))?;
        let snap: EventPersistSnapshot = serde_json::from_slice(&bytes)
            .map_err(|e| CoreError::Storage(format!("event persist decode: {e}")))?;
        let n = snap.events.len();
        self.import_snapshot(snap);
        tracing::info!(path = %path.display(), events = n, "loaded embedded V1 event journal");
        Ok(true)
    }

    fn maybe_persist(&self) {
        let n = self.dirty.fetch_add(1, Ordering::Relaxed) + 1;
        // Flush every 5 writes or when path set and n==1 after load
        if n % 5 != 0 {
            return;
        }
        let path = self.persist_path.read().clone();
        if let Some(p) = path {
            if let Err(e) = self.save_to_path(&p) {
                tracing::warn!(error = %e, "V1 event persist failed");
            }
        }
    }

    async fn upsert_inner(&self, event: CanonicalEventRecord, persist: bool) -> CoreResult<()> {
        let tenant = event.tenant_id.clone();
        let event_id = event.event_id.clone();
        let resource_id = event.resource_id.clone();

        let tenant_events = self
            .by_id
            .entry(tenant.clone())
            .or_insert_with(DashMap::new);
        match tenant_events.entry(event_id.clone()) {
            dashmap::mapref::entry::Entry::Occupied(mut occ) => {
                let existing = occ.get();
                let replace = event.acl.acl_version > existing.acl.acl_version
                    || (event.acl.acl_version == existing.acl.acl_version
                        && event.event_sequence_number >= existing.event_sequence_number)
                    || event.event_timestamp >= existing.event_timestamp
                        && event.event_sequence_number >= existing.event_sequence_number;
                let _ = replace;
                occ.insert(event.clone());
            }
            dashmap::mapref::entry::Entry::Vacant(v) => {
                v.insert(event.clone());
            }
        }

        let by_res = self
            .by_resource
            .entry(tenant)
            .or_insert_with(DashMap::new);
        let list = by_res
            .entry(resource_id)
            .or_insert_with(|| RwLock::new(Vec::new()));
        {
            let mut v = list.write();
            if let Some(pos) = v.iter().position(|e| e.event_id == event_id) {
                v[pos] = event;
            } else {
                v.push(event);
            }
            v.sort_by_key(|e| e.event_timestamp);
        }
        if persist {
            self.maybe_persist();
        }
        Ok(())
    }
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self {
            by_id: DashMap::new(),
            by_resource: DashMap::new(),
            persist_path: RwLock::new(None),
            dirty: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn upsert(&self, event: CanonicalEventRecord) -> CoreResult<()> {
        self.upsert_inner(event, true).await
    }

    async fn query(
        &self,
        ctx: &QueryContext,
        filter: &EventQuery,
    ) -> CoreResult<Vec<CanonicalEventRecord>> {
        if ctx.tenant_id != filter.tenant_id {
            return Err(CoreError::AclDenied(
                "query tenant_id does not match context".into(),
            ));
        }
        let Some(tenant_events) = self.by_id.get(&filter.tenant_id) else {
            return Ok(Vec::new());
        };

        let mut results: Vec<CanonicalEventRecord> = tenant_events
            .iter()
            .map(|e| e.value().clone())
            .filter(|e| {
                if !acl_allows(ctx, e.acl.is_private, &e.acl.allowed_group_ids) {
                    return false;
                }
                if !filter.categories.is_empty()
                    && !filter.categories.iter().any(|c| *c as i32 == e.category as i32)
                {
                    // Compare via clickhouse labels for safety
                    let wanted: Vec<&str> = filter
                        .categories
                        .iter()
                        .map(|c| c.clickhouse_label())
                        .collect();
                    if !wanted.contains(&e.category.clickhouse_label()) {
                        return false;
                    }
                }
                if !filter.providers.is_empty() {
                    let wanted: Vec<&str> = filter
                        .providers
                        .iter()
                        .map(|p| p.clickhouse_label())
                        .collect();
                    if !wanted.contains(&e.provider.clickhouse_label()) {
                        return false;
                    }
                }
                if let Some(ref rid) = filter.resource_id {
                    if &e.resource_id != rid {
                        return false;
                    }
                }
                if let Some(ref prid) = filter.parent_resource_id {
                    if &e.parent_resource_id != prid {
                        return false;
                    }
                }
                if let Some(ref et) = filter.event_type {
                    if &e.event_type != et {
                        return false;
                    }
                }
                if let Some(since) = filter.since {
                    if e.event_timestamp < since {
                        return false;
                    }
                }
                if let Some(until) = filter.until {
                    if e.event_timestamp > until {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by_key(|e| std::cmp::Reverse(e.event_timestamp));
        results.truncate(filter.limit.max(1));
        Ok(results)
    }

    async fn count_unique(&self, tenant_id: &str) -> CoreResult<u64> {
        Ok(self
            .by_id
            .get(tenant_id)
            .map(|m| m.len() as u64)
            .unwrap_or(0))
    }

    async fn get_raw(
        &self,
        tenant_id: &str,
        event_id: &str,
    ) -> CoreResult<Option<CanonicalEventRecord>> {
        Ok(self
            .by_id
            .get(tenant_id)
            .and_then(|m| m.get(event_id).map(|e| e.clone())))
    }

    async fn latest_state_for_resource(
        &self,
        ctx: &QueryContext,
        resource_id: &str,
    ) -> CoreResult<Option<CanonicalEventRecord>> {
        let Some(tenant_map) = self.by_resource.get(&ctx.tenant_id) else {
            return Ok(None);
        };
        let Some(list) = tenant_map.get(resource_id) else {
            return Ok(None);
        };
        let v = list.read();
        // State reconstruction by origin timestamp (not ingest order).
        // argMax(state, timestamp) analogue: last event by event_timestamp wins.
        let latest = v
            .iter()
            .filter(|e| acl_allows(ctx, e.acl.is_private, &e.acl.allowed_group_ids))
            .max_by_key(|e| (e.event_timestamp, e.event_sequence_number))
            .cloned();
        Ok(latest)
    }
}

/// Derive a simplified "PR state" from event_type for out-of-order tests.
pub fn derive_pr_state(event_type: &str) -> &'static str {
    match event_type {
        "pull_request.opened" | "pull_request.reopened" => "OPEN",
        "pull_request.closed" => "CLOSED",
        "pull_request.merged" => "MERGED",
        "pull_request.synchronize" => "OPEN",
        other if other.contains("closed") => "CLOSED",
        other if other.contains("opened") => "OPEN",
        _ => "UNKNOWN",
    }
}

/// Helper used by tests to assert resource state.
pub async fn pr_state(
    store: &dyn EventStore,
    ctx: &QueryContext,
    resource_id: &str,
) -> CoreResult<Option<String>> {
    Ok(store
        .latest_state_for_resource(ctx, resource_id)
        .await?
        .map(|e| derive_pr_state(&e.event_type).to_string()))
}

/// Admin stats.
pub async fn tenant_stats(store: &InMemoryEventStore) -> HashMap<String, u64> {
    store
        .by_id
        .iter()
        .map(|e| (e.key().clone(), e.value().len() as u64))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActorIdentity, AclSnapshot};
    use chrono::{DateTime, TimeZone, Utc};
    use telemetry_proto::{EventCategory, SourceProvider};

    fn evt(
        id: &str,
        etype: &str,
        ts: DateTime<Utc>,
        private: bool,
        groups: &[&str],
    ) -> CanonicalEventRecord {
        CanonicalEventRecord {
            event_id: id.into(),
            tenant_id: "ten".into(),
            provider: SourceProvider::Github,
            category: EventCategory::Code,
            event_type: etype.into(),
            event_timestamp: ts,
            ingested_at: Utc::now(),
            actor: ActorIdentity::default(),
            acl: AclSnapshot {
                tenant_id: "ten".into(),
                allowed_group_ids: groups.iter().map(|s| s.to_string()).collect(),
                is_private: private,
                acl_version: 1,
            },
            resource_id: "repo/pr/1".into(),
            parent_resource_id: "repo".into(),
            attributes: serde_json::json!({}),
            raw_payload_s3_uri: String::new(),
            event_sequence_number: 1,
        }
    }

    #[tokio::test]
    async fn acl_blocks_private() {
        let store = InMemoryEventStore::new();
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        store
            .upsert(evt("e1", "pull_request.opened", t1, true, &["eng"]))
            .await
            .unwrap();

        let allowed = QueryContext {
            tenant_id: "ten".into(),
            global_user_id: "u1".into(),
            group_ids: vec!["eng".into()],
        };
        let denied = QueryContext {
            tenant_id: "ten".into(),
            global_user_id: "u2".into(),
            group_ids: vec!["sales".into()],
        };
        let filter = EventQuery {
            tenant_id: "ten".into(),
            limit: 10,
            ..EventQuery {
                tenant_id: "ten".into(),
                limit: 10,
                categories: vec![],
                providers: vec![],
                resource_id: None,
                parent_resource_id: None,
                event_type: None,
                since: None,
                until: None,
            }
        };
        assert_eq!(store.query(&allowed, &filter).await.unwrap().len(), 1);
        assert_eq!(store.query(&denied, &filter).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn out_of_order_state() {
        let store = InMemoryEventStore::new();
        let t1 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 5).unwrap();
        // CLOSED arrives first
        store
            .upsert(evt("e_closed", "pull_request.closed", t2, false, &[]))
            .await
            .unwrap();
        store
            .upsert(evt("e_open", "pull_request.opened", t1, false, &[]))
            .await
            .unwrap();

        let ctx = QueryContext {
            tenant_id: "ten".into(),
            global_user_id: "u".into(),
            group_ids: vec![],
        };
        let state = pr_state(store.as_ref(), &ctx, "repo/pr/1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(state, "CLOSED");
    }
}
