//! Twin state store: embedded in-memory + trait for production CRDB.

use crate::error::{TwinError, TwinResult};
use crate::model::*;
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

#[async_trait]
pub trait TwinStore: Send + Sync {
    async fn upsert_twin(&self, twin: Twin) -> TwinResult<()>;
    async fn get_twin(&self, tenant_id: &str, twin_id: &str) -> TwinResult<Option<Twin>>;
    async fn list_twins(&self, tenant_id: &str) -> TwinResult<Vec<Twin>>;

    async fn put_slack_map(&self, map: SlackUserMap) -> TwinResult<()>;
    async fn get_slack_map(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> TwinResult<Option<SlackUserMap>>;
    /// All Slack maps for a tenant (multi-person team admin).
    async fn list_slack_maps(&self, tenant_id: &str) -> TwinResult<Vec<SlackUserMap>>;

    /// Insert or replace ledger snapshot. Idempotent on unique period key.
    /// Returns false if an existing published draft blocks replace (ledger already published).
    async fn put_ledger(&self, snap: LedgerSnapshot) -> TwinResult<()>;
    async fn get_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<LedgerSnapshot>>;
    async fn get_ledger_by_period(
        &self,
        tenant_id: &str,
        twin_id: &str,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> TwinResult<Option<LedgerSnapshot>>;

    async fn put_draft(&self, draft: DraftDelivery) -> TwinResult<()>;
    async fn get_draft(&self, tenant_id: &str, draft_id: &str) -> TwinResult<Option<DraftDelivery>>;
    async fn get_draft_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<DraftDelivery>>;
    async fn update_draft(&self, draft: DraftDelivery) -> TwinResult<()>;
    /// All drafts for a twin (newest first). Used by multi-person Team digests board.
    async fn list_drafts_for_twin(
        &self,
        tenant_id: &str,
        twin_id: &str,
    ) -> TwinResult<Vec<DraftDelivery>>;

    /// Insert publish record. Unique (tenant_id, ledger_id) — returns Ok(false) if already exists.
    async fn put_publish_if_absent(&self, rec: PublishRecord) -> TwinResult<bool>;
    async fn get_publish_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<PublishRecord>>;

    async fn put_compile_run(&self, run: CompileRun) -> TwinResult<()>;
}

/// On-disk snapshot for embedded staging (twins + slack maps + drafts survive restart).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TwinPersistSnapshot {
    pub version: u32,
    pub saved_at: Option<String>,
    pub twins: Vec<Twin>,
    pub slack_maps: Vec<SlackUserMap>,
    pub drafts: Vec<DraftDelivery>,
    pub ledgers: Vec<LedgerSnapshot>,
    /// Tenant-scoped JSON blobs (roles, tomorrow focus, SSO scaffold, …).
    #[serde(default)]
    pub tenant_kv: Vec<TenantKvEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TenantKvEntry {
    pub tenant_id: String,
    pub key: String,
    pub value: serde_json::Value,
}

pub struct InMemoryTwinStore {
    twins: DashMap<(String, String), Twin>,
    slack_map: DashMap<(String, String), SlackUserMap>,
    ledgers: DashMap<(String, String), LedgerSnapshot>,
    /// (tenant, twin, period_start_rfc, period_end_rfc) → ledger_id
    ledger_period: DashMap<(String, String, String, String), String>,
    drafts: DashMap<(String, String), DraftDelivery>,
    draft_by_ledger: DashMap<(String, String), String>,
    publishes: DashMap<(String, String), PublishRecord>,
    /// ledger_id uniqueness for publish
    publish_by_ledger: DashMap<(String, String), String>,
    compile_runs: DashMap<(String, String), CompileRun>,
    /// (tenant_id, key) → JSON value (roles, tomorrow_focus, …)
    tenant_kv: DashMap<(String, String), serde_json::Value>,
    lock: RwLock<()>,
}

impl InMemoryTwinStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            twins: DashMap::new(),
            slack_map: DashMap::new(),
            ledgers: DashMap::new(),
            ledger_period: DashMap::new(),
            drafts: DashMap::new(),
            draft_by_ledger: DashMap::new(),
            publishes: DashMap::new(),
            publish_by_ledger: DashMap::new(),
            compile_runs: DashMap::new(),
            tenant_kv: DashMap::new(),
            lock: RwLock::new(()),
        })
    }

    /// Get tenant-scoped JSON blob (embedded).
    pub fn get_tenant_kv(&self, tenant_id: &str, key: &str) -> Option<serde_json::Value> {
        self.tenant_kv
            .get(&(tenant_id.to_string(), key.to_string()))
            .map(|v| v.clone())
    }

    /// Put tenant-scoped JSON blob (embedded).
    pub fn put_tenant_kv(&self, tenant_id: &str, key: &str, value: serde_json::Value) {
        self.tenant_kv
            .insert((tenant_id.to_string(), key.to_string()), value);
    }

    /// Export durable pilot state (team map + digests) for embedded restarts.
    pub fn export_snapshot(&self) -> TwinPersistSnapshot {
        let twins: Vec<Twin> = self.twins.iter().map(|e| e.value().clone()).collect();
        let slack_maps: Vec<SlackUserMap> =
            self.slack_map.iter().map(|e| e.value().clone()).collect();
        let drafts: Vec<DraftDelivery> = self.drafts.iter().map(|e| e.value().clone()).collect();
        let ledgers: Vec<LedgerSnapshot> =
            self.ledgers.iter().map(|e| e.value().clone()).collect();
        let tenant_kv: Vec<TenantKvEntry> = self
            .tenant_kv
            .iter()
            .map(|e| TenantKvEntry {
                tenant_id: e.key().0.clone(),
                key: e.key().1.clone(),
                value: e.value().clone(),
            })
            .collect();
        TwinPersistSnapshot {
            version: 2,
            saved_at: Some(Utc::now().to_rfc3339()),
            twins,
            slack_maps,
            drafts,
            ledgers,
            tenant_kv,
        }
    }

    /// Restore from disk. Merges (upserts) into current maps — does not wipe unknown keys first.
    pub fn import_snapshot(&self, snap: TwinPersistSnapshot) {
        for t in snap.twins {
            self.twins
                .insert((t.tenant_id.clone(), t.twin_id.clone()), t);
        }
        for m in snap.slack_maps {
            self.slack_map
                .insert((m.tenant_id.clone(), m.global_user_id.clone()), m);
        }
        for d in snap.drafts {
            let ledger_key = (d.tenant_id.clone(), d.ledger_id.clone());
            self.draft_by_ledger
                .insert(ledger_key, d.draft_id.clone());
            self.drafts
                .insert((d.tenant_id.clone(), d.draft_id.clone()), d);
        }
        for snap_l in snap.ledgers {
            let pk = (
                snap_l.tenant_id.clone(),
                snap_l.twin_id.clone(),
                snap_l.period_start.to_rfc3339(),
                snap_l.period_end.to_rfc3339(),
            );
            self.ledger_period
                .insert(pk, snap_l.ledger_id.clone());
            self.ledgers.insert(
                (snap_l.tenant_id.clone(), snap_l.ledger_id.clone()),
                snap_l,
            );
        }
        for kv in snap.tenant_kv {
            self.tenant_kv
                .insert((kv.tenant_id, kv.key), kv.value);
        }
    }

    pub fn save_to_path(&self, path: &Path) -> TwinResult<()> {
        let snap = self.export_snapshot();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                TwinError::Storage(format!("persist mkdir {}: {e}", parent.display()))
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&snap)
            .map_err(|e| TwinError::Storage(format!("persist encode: {e}")))?;
        std::fs::write(&tmp, bytes)
            .map_err(|e| TwinError::Storage(format!("persist write {}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| TwinError::Storage(format!("persist rename {}: {e}", path.display())))?;
        Ok(())
    }

    pub fn load_from_path(&self, path: &Path) -> TwinResult<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let bytes = std::fs::read(path)
            .map_err(|e| TwinError::Storage(format!("persist read {}: {e}", path.display())))?;
        let snap: TwinPersistSnapshot = serde_json::from_slice(&bytes)
            .map_err(|e| TwinError::Storage(format!("persist decode: {e}")))?;
        let n_twins = snap.twins.len();
        let n_maps = snap.slack_maps.len();
        self.import_snapshot(snap);
        tracing::info!(
            path = %path.display(),
            twins = n_twins,
            slack_maps = n_maps,
            "loaded embedded twin persist snapshot"
        );
        Ok(true)
    }

    pub fn latest_draft_for_twin(&self, tenant_id: &str, twin_id: &str) -> Option<DraftDelivery> {
        let mut drafts: Vec<DraftDelivery> = self
            .drafts
            .iter()
            .filter(|e| e.key().0 == tenant_id && e.value().twin_id == twin_id)
            .map(|e| e.value().clone())
            .collect();
        drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        drafts.into_iter().next()
    }
}

impl Default for InMemoryTwinStore {
    fn default() -> Self {
        Self {
            twins: DashMap::new(),
            slack_map: DashMap::new(),
            ledgers: DashMap::new(),
            ledger_period: DashMap::new(),
            drafts: DashMap::new(),
            draft_by_ledger: DashMap::new(),
            publishes: DashMap::new(),
            publish_by_ledger: DashMap::new(),
            compile_runs: DashMap::new(),
            tenant_kv: DashMap::new(),
            lock: RwLock::new(()),
        }
    }
}

#[async_trait]
impl TwinStore for InMemoryTwinStore {
    async fn upsert_twin(&self, twin: Twin) -> TwinResult<()> {
        self.twins
            .insert((twin.tenant_id.clone(), twin.twin_id.clone()), twin);
        Ok(())
    }

    async fn get_twin(&self, tenant_id: &str, twin_id: &str) -> TwinResult<Option<Twin>> {
        Ok(self
            .twins
            .get(&(tenant_id.to_string(), twin_id.to_string()))
            .map(|t| t.clone()))
    }

    async fn list_twins(&self, tenant_id: &str) -> TwinResult<Vec<Twin>> {
        Ok(self
            .twins
            .iter()
            .filter(|e| e.key().0 == tenant_id)
            .map(|e| e.value().clone())
            .collect())
    }

    async fn put_slack_map(&self, map: SlackUserMap) -> TwinResult<()> {
        self.slack_map.insert(
            (map.tenant_id.clone(), map.global_user_id.clone()),
            map,
        );
        Ok(())
    }

    async fn get_slack_map(
        &self,
        tenant_id: &str,
        global_user_id: &str,
    ) -> TwinResult<Option<SlackUserMap>> {
        Ok(self
            .slack_map
            .get(&(tenant_id.to_string(), global_user_id.to_string()))
            .map(|m| m.clone()))
    }

    async fn list_slack_maps(&self, tenant_id: &str) -> TwinResult<Vec<SlackUserMap>> {
        Ok(self
            .slack_map
            .iter()
            .filter(|e| e.key().0 == tenant_id)
            .map(|e| e.value().clone())
            .collect())
    }

    async fn put_ledger(&self, snap: LedgerSnapshot) -> TwinResult<()> {
        let _g = self.lock.write();
        // If already published for this ledger id, do not replace content used for audit
        if self
            .publish_by_ledger
            .contains_key(&(snap.tenant_id.clone(), snap.ledger_id.clone()))
        {
            return Err(TwinError::Conflict(format!(
                "ledger {} already published",
                snap.ledger_id
            )));
        }
        let pk = (
            snap.tenant_id.clone(),
            snap.twin_id.clone(),
            snap.period_start.to_rfc3339(),
            snap.period_end.to_rfc3339(),
        );
        if let Some(existing_id) = self.ledger_period.get(&pk) {
            // Replace draft-only recompile: remove old ledger if same period different id
            if existing_id.as_str() != snap.ledger_id {
                self.ledgers
                    .remove(&(snap.tenant_id.clone(), existing_id.clone()));
            }
        }
        self.ledger_period
            .insert(pk, snap.ledger_id.clone());
        self.ledgers
            .insert((snap.tenant_id.clone(), snap.ledger_id.clone()), snap);
        Ok(())
    }

    async fn get_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<LedgerSnapshot>> {
        Ok(self
            .ledgers
            .get(&(tenant_id.to_string(), ledger_id.to_string()))
            .map(|l| l.clone()))
    }

    async fn get_ledger_by_period(
        &self,
        tenant_id: &str,
        twin_id: &str,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> TwinResult<Option<LedgerSnapshot>> {
        let pk = (
            tenant_id.to_string(),
            twin_id.to_string(),
            period_start.to_rfc3339(),
            period_end.to_rfc3339(),
        );
        if let Some(id) = self.ledger_period.get(&pk) {
            return self.get_ledger(tenant_id, id.as_str()).await;
        }
        Ok(None)
    }

    async fn put_draft(&self, draft: DraftDelivery) -> TwinResult<()> {
        let _g = self.lock.write();
        let key = (draft.tenant_id.clone(), draft.draft_id.clone());
        let ledger_key = (draft.tenant_id.clone(), draft.ledger_id.clone());
        if let Some(existing_id) = self.draft_by_ledger.get(&ledger_key) {
            if existing_id.as_str() != draft.draft_id {
                return Err(TwinError::Conflict(format!(
                    "draft already exists for ledger {}",
                    draft.ledger_id
                )));
            }
        }
        self.draft_by_ledger
            .insert(ledger_key, draft.draft_id.clone());
        self.drafts.insert(key, draft);
        Ok(())
    }

    async fn get_draft(
        &self,
        tenant_id: &str,
        draft_id: &str,
    ) -> TwinResult<Option<DraftDelivery>> {
        Ok(self
            .drafts
            .get(&(tenant_id.to_string(), draft_id.to_string()))
            .map(|d| d.clone()))
    }

    async fn get_draft_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<DraftDelivery>> {
        if let Some(id) = self
            .draft_by_ledger
            .get(&(tenant_id.to_string(), ledger_id.to_string()))
        {
            return self.get_draft(tenant_id, id.as_str()).await;
        }
        Ok(None)
    }

    async fn update_draft(&self, draft: DraftDelivery) -> TwinResult<()> {
        let key = (draft.tenant_id.clone(), draft.draft_id.clone());
        if !self.drafts.contains_key(&key) {
            return Err(TwinError::NotFound(format!(
                "draft {}",
                draft.draft_id
            )));
        }
        self.drafts.insert(key, draft);
        Ok(())
    }

    async fn list_drafts_for_twin(
        &self,
        tenant_id: &str,
        twin_id: &str,
    ) -> TwinResult<Vec<DraftDelivery>> {
        let mut drafts: Vec<DraftDelivery> = self
            .drafts
            .iter()
            .filter(|e| e.key().0 == tenant_id && e.value().twin_id == twin_id)
            .map(|e| e.value().clone())
            .collect();
        drafts.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(drafts)
    }

    async fn put_publish_if_absent(&self, rec: PublishRecord) -> TwinResult<bool> {
        let _g = self.lock.write();
        let ledger_key = (rec.tenant_id.clone(), rec.ledger_id.clone());
        if self.publish_by_ledger.contains_key(&ledger_key) {
            return Ok(false);
        }
        self.publish_by_ledger
            .insert(ledger_key, rec.publish_id.clone());
        self.publishes
            .insert((rec.tenant_id.clone(), rec.publish_id.clone()), rec);
        Ok(true)
    }

    async fn get_publish_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<PublishRecord>> {
        if let Some(id) = self
            .publish_by_ledger
            .get(&(tenant_id.to_string(), ledger_id.to_string()))
        {
            return Ok(self
                .publishes
                .get(&(tenant_id.to_string(), id.clone()))
                .map(|p| p.clone()));
        }
        Ok(None)
    }

    async fn put_compile_run(&self, run: CompileRun) -> TwinResult<()> {
        self.compile_runs
            .insert((run.tenant_id.clone(), run.run_id.clone()), run);
        Ok(())
    }
}

#[cfg(test)]
mod persist_tests {
    use super::*;
    use crate::ids::person_twin_id;
    use chrono::Utc;

    #[tokio::test]
    async fn snapshot_roundtrip_keeps_team_map() {
        let store = InMemoryTwinStore::new();
        let now = Utc::now();
        let twin = Twin {
            tenant_id: "ten_github".into(),
            twin_id: person_twin_id("gu_a"),
            twin_kind: TwinKind::Person,
            subject_id: "gu_a".into(),
            display_name: "A".into(),
            timezone: "UTC".into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({"provider_aliases": ["alice"]}),
            created_at: now,
            updated_at: now,
        };
        store.upsert_twin(twin).await.unwrap();
        store
            .put_slack_map(SlackUserMap {
                tenant_id: "ten_github".into(),
                global_user_id: "gu_a".into(),
                slack_user_id: "U1".into(),
                slack_team_id: String::new(),
            })
            .await
            .unwrap();
        let dir = std::env::temp_dir().join(format!("twin_persist_test_{}", now.timestamp_nanos_opt().unwrap_or(0)));
        let path = dir.join("twin_state.json");
        store.save_to_path(&path).unwrap();

        let store2 = InMemoryTwinStore::new();
        assert!(store2.load_from_path(&path).unwrap());
        let twins = store2.list_twins("ten_github").await.unwrap();
        assert_eq!(twins.len(), 1);
        let maps = store2.list_slack_maps("ten_github").await.unwrap();
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].slack_user_id, "U1");
        let _ = std::fs::remove_dir_all(dir);
    }
}
