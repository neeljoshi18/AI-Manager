//! Twin state store: embedded in-memory + trait for production CRDB.

use crate::error::{TwinError, TwinResult};
use crate::model::*;
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
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

    /// Insert publish record. Unique (tenant_id, ledger_id) — returns Ok(false) if already exists.
    async fn put_publish_if_absent(&self, rec: PublishRecord) -> TwinResult<bool>;
    async fn get_publish_by_ledger(
        &self,
        tenant_id: &str,
        ledger_id: &str,
    ) -> TwinResult<Option<PublishRecord>>;

    async fn put_compile_run(&self, run: CompileRun) -> TwinResult<()>;
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
            lock: RwLock::new(()),
        })
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
