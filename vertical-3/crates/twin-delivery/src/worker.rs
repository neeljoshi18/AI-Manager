use crate::policy::DeliveryPolicy;
use crate::slack::{SlackClient, SlackPostResult};
use chrono::{Duration, Utc};
use std::sync::Arc;
use twin_core::ids::body_hash;
use twin_core::model::*;
use twin_core::state_machine::{
    apply_delivery_event, initial_draft_status, silence_may_publish, DeliveryEvent,
};
use twin_core::store::TwinStore;
use twin_core::{TwinError, TwinResult};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct StartDeliveryOpts {
    pub force_now: Option<chrono::DateTime<Utc>>,
}

pub struct DeliveryService {
    store: Arc<dyn TwinStore>,
    slack: Arc<dyn SlackClient>,
    policy: DeliveryPolicy,
}

impl DeliveryService {
    pub fn new(
        store: Arc<dyn TwinStore>,
        slack: Arc<dyn SlackClient>,
        policy: DeliveryPolicy,
    ) -> Self {
        Self {
            store,
            slack,
            policy,
        }
    }

    pub fn slack(&self) -> Arc<dyn SlackClient> {
        self.slack.clone()
    }

    /// Create draft after compile and optionally DM / auto-queue publish.
    ///
    /// `allow_notify`: when false, store draft/state only — no Slack DM (used for
    /// continuous recompiles between scheduled status windows).
    pub async fn start_after_compile(
        &self,
        twin: &Twin,
        snap: &LedgerSnapshot,
        draft_text: &str,
        now: chrono::DateTime<Utc>,
    ) -> TwinResult<DraftDelivery> {
        self.start_after_compile_opts(twin, snap, draft_text, now, true)
            .await
    }

    pub async fn start_after_compile_opts(
        &self,
        twin: &Twin,
        snap: &LedgerSnapshot,
        draft_text: &str,
        now: chrono::DateTime<Utc>,
        allow_notify: bool,
    ) -> TwinResult<DraftDelivery> {
        // Idempotent: one draft per ledger — refresh text if still open
        if let Some(mut existing) = self
            .store
            .get_draft_by_ledger(&twin.tenant_id, &snap.ledger_id)
            .await?
        {
            if matches!(
                existing.status,
                DraftStatus::Pending
                    | DraftStatus::ForceHuman
                    | DraftStatus::Edited
                    | DraftStatus::Shadow
            ) {
                existing.draft_text = draft_text.to_string();
                existing.updated_at = now;
                self.store.update_draft(existing.clone()).await?;
            }
            return Ok(existing);
        }

        let in_shadow = twin.is_in_shadow(now);
        // Quiet recompiles stay shadow-like for notify purposes when !allow_notify
        let mut status =
            initial_draft_status(in_shadow, snap.confidence_rollup, twin.high_auto_publish);
        if !allow_notify && !in_shadow {
            // Still track draft as pending for later scheduled notify, but skip Slack below
            if status == DraftStatus::PublishQueued {
                status = DraftStatus::Pending;
            }
        }

        let veto_deadline = match (status, snap.confidence_rollup) {
            (DraftStatus::Pending | DraftStatus::ForceHuman, ConfidenceTier::Medium) => {
                Some(now + Duration::seconds(self.policy.medium_veto_window_secs))
            }
            (DraftStatus::Pending | DraftStatus::ForceHuman, ConfidenceTier::Blocker) => {
                Some(now + Duration::seconds(self.policy.blocker_veto_window_secs))
            }
            (DraftStatus::Pending, ConfidenceTier::High) => {
                Some(now + Duration::seconds(self.policy.medium_veto_window_secs))
            }
            _ => None,
        };

        let draft_id = format!("dft_{}", Uuid::new_v4());
        let mut draft = DraftDelivery {
            tenant_id: twin.tenant_id.clone(),
            draft_id: draft_id.clone(),
            ledger_id: snap.ledger_id.clone(),
            twin_id: twin.twin_id.clone(),
            status,
            slack_dm_channel: String::new(),
            slack_dm_ts: String::new(),
            draft_text: draft_text.to_string(),
            edited_text: None,
            veto_deadline,
            created_at: now,
            updated_at: now,
        };

        // Shadow or quiet compile: no Slack calls
        if status == DraftStatus::Shadow || !allow_notify {
            self.store.put_draft(draft.clone()).await?;
            return Ok(draft);
        }

        // High auto-publish: optional short DM then queue
        if status == DraftStatus::PublishQueued
            && snap.confidence_rollup == ConfidenceTier::High
            && twin.high_auto_publish
        {
            if let Some(map) = self
                .store
                .get_slack_map(&twin.tenant_id, &twin.subject_id)
                .await?
            {
                if let Ok(dm) = self
                    .slack
                    .post_dm(
                        &map.slack_user_id,
                        &format!("Auto-publishing high confidence status:\n{draft_text}"),
                    )
                    .await
                {
                    draft.slack_dm_channel = dm.channel;
                    draft.slack_dm_ts = dm.ts;
                }
            }
            self.store.put_draft(draft.clone()).await?;
            let _ = self.try_publish_channel(twin, &mut draft).await?;
            return Ok(draft);
        }

        // Medium / High(no auto) / Blocker: required DM (except we still DM for high when not auto)
        if matches!(
            status,
            DraftStatus::Pending | DraftStatus::ForceHuman
        ) {
            let slack_user = self
                .store
                .get_slack_map(&twin.tenant_id, &twin.subject_id)
                .await?
                .map(|m| m.slack_user_id)
                .unwrap_or_else(|| format!("U_{}", twin.subject_id));

            let deadline_s = veto_deadline
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "none".into());
            let dm_text = format!(
                "{draft_text}\n\n[Publish as-is] · [Edit] · [Veto]\nReply window until {deadline_s}."
            );
            match self.slack.post_dm(&slack_user, &dm_text).await {
                Ok(dm) => {
                    draft.slack_dm_channel = dm.channel;
                    draft.slack_dm_ts = dm.ts;
                    let _ = apply_delivery_event(
                        draft.status,
                        &DeliveryEvent::DmSent,
                        snap.confidence_rollup,
                        twin.high_auto_publish,
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "dm failed; draft still stored");
                }
            }
        }

        // Re-check status after potential transitions (none for dm)
        draft.status = status;
        self.store.put_draft(draft.clone()).await?;
        Ok(draft)
    }

    pub async fn silence_timeout(
        &self,
        twin: &Twin,
        tenant_id: &str,
        draft_id: &str,
    ) -> TwinResult<(DraftDelivery, Option<PublishRecord>)> {
        let mut draft = self
            .store
            .get_draft(tenant_id, draft_id)
            .await?
            .ok_or_else(|| TwinError::NotFound(format!("draft {draft_id}")))?;
        let snap = self
            .store
            .get_ledger(tenant_id, &draft.ledger_id)
            .await?
            .ok_or_else(|| TwinError::NotFound("ledger".into()))?;

        if !silence_may_publish(snap.confidence_rollup) {
            return Err(TwinError::Conflict(
                "silence timeout does not auto-publish for this confidence tier".into(),
            ));
        }

        let next = apply_delivery_event(
            draft.status,
            &DeliveryEvent::MediumSilenceTimeout,
            snap.confidence_rollup,
            twin.high_auto_publish,
        )
        .ok_or_else(|| {
            TwinError::Conflict(format!(
                "silence not allowed from status {:?}",
                draft.status
            ))
        })?;
        draft.status = next;
        draft.updated_at = Utc::now();
        self.store.update_draft(draft.clone()).await?;

        if draft.status == DraftStatus::PublishQueued {
            let pub_rec = self.try_publish_channel(twin, &mut draft).await?;
            return Ok((draft, pub_rec));
        }
        Ok((draft, None))
    }

    pub async fn explicit_publish(
        &self,
        twin: &Twin,
        tenant_id: &str,
        draft_id: &str,
    ) -> TwinResult<(DraftDelivery, Option<PublishRecord>)> {
        let mut draft = self
            .store
            .get_draft(tenant_id, draft_id)
            .await?
            .ok_or_else(|| TwinError::NotFound(format!("draft {draft_id}")))?;
        let snap = self
            .store
            .get_ledger(tenant_id, &draft.ledger_id)
            .await?
            .ok_or_else(|| TwinError::NotFound("ledger".into()))?;

        // Idempotent: already published → return existing record (TC-T08)
        if draft.status == DraftStatus::Published {
            let existing = self
                .store
                .get_publish_by_ledger(tenant_id, &draft.ledger_id)
                .await?;
            return Ok((draft, existing));
        }
        if draft.status == DraftStatus::Vetoed {
            return Err(TwinError::Conflict("vetoed — never channel post".into()));
        }

        if draft.status != DraftStatus::PublishQueued {
            let next = apply_delivery_event(
                draft.status,
                &DeliveryEvent::ExplicitPublish,
                snap.confidence_rollup,
                twin.high_auto_publish,
            )
            .ok_or_else(|| {
                TwinError::Conflict(format!(
                    "cannot publish from status {:?}",
                    draft.status
                ))
            })?;
            draft.status = next;
            draft.updated_at = Utc::now();
            self.store.update_draft(draft.clone()).await?;
        }

        let pub_rec = self.try_publish_channel(twin, &mut draft).await?;
        Ok((draft, pub_rec))
    }

    /// Exactly-once channel publish intent (TC-T08).
    async fn try_publish_channel(
        &self,
        twin: &Twin,
        draft: &mut DraftDelivery,
    ) -> TwinResult<Option<PublishRecord>> {
        if draft.status == DraftStatus::Vetoed {
            return Err(TwinError::Conflict("vetoed — never channel post".into()));
        }
        if draft.status == DraftStatus::Shadow {
            return Err(TwinError::Conflict("shadow — no publish".into()));
        }
        if draft.status == DraftStatus::Published {
            return self
                .store
                .get_publish_by_ledger(&draft.tenant_id, &draft.ledger_id)
                .await;
        }
        if draft.status != DraftStatus::PublishQueued {
            return Ok(None);
        }

        // Already published?
        if let Some(existing) = self
            .store
            .get_publish_by_ledger(&draft.tenant_id, &draft.ledger_id)
            .await?
        {
            draft.status = DraftStatus::Published;
            draft.updated_at = Utc::now();
            let _ = self.store.update_draft(draft.clone()).await;
            return Ok(Some(existing));
        }

        let body = draft.publish_body().to_string();
        let hash = body_hash(&body);
        let channel = if twin.channel_id.is_empty() {
            "C_TEAM".to_string()
        } else {
            twin.channel_id.clone()
        };

        let post: SlackPostResult = match self.slack.post_channel(&channel, &body).await {
            Ok(p) => p,
            Err(e) => {
                draft.status = DraftStatus::PublishFailed;
                draft.updated_at = Utc::now();
                self.store.update_draft(draft.clone()).await?;
                return Err(e);
            }
        };

        let rec = PublishRecord {
            tenant_id: draft.tenant_id.clone(),
            publish_id: format!("pub_{}", Uuid::new_v4()),
            ledger_id: draft.ledger_id.clone(),
            draft_id: draft.draft_id.clone(),
            channel_id: channel,
            slack_ts: post.ts,
            body_hash: hash,
            published_at: Utc::now(),
        };

        let inserted = self.store.put_publish_if_absent(rec.clone()).await?;
        if !inserted {
            // Race: another worker won — return existing; do not double-count as new
            let existing = self
                .store
                .get_publish_by_ledger(&draft.tenant_id, &draft.ledger_id)
                .await?;
            draft.status = DraftStatus::Published;
            draft.updated_at = Utc::now();
            self.store.update_draft(draft.clone()).await?;
            return Ok(existing);
        }

        draft.status = DraftStatus::Published;
        draft.updated_at = Utc::now();
        self.store.update_draft(draft.clone()).await?;
        Ok(Some(rec))
    }
}
