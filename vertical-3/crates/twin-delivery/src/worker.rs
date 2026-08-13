use crate::delivery::{DeliveryClient, DeliveryPostResult};
use crate::policy::DeliveryPolicy;
use crate::resolve_chat_user_id;
use chrono::{Duration, Utc};
use std::sync::Arc;
use twin_core::ids::body_hash;
use twin_core::model::*;
use twin_core::notify_policy::{
    decide_notify, load_notify_state, record_dm_sent, write_notify_state, SuppressReason,
};
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

/// Result of draft + optional DM (Notify Policy v1).
#[derive(Debug, Clone)]
pub struct DeliveryStartResult {
    pub draft: DraftDelivery,
    pub dm_sent: bool,
    /// Why Slack was skipped (if any).
    pub suppressed: Option<&'static str>,
}

pub struct DeliveryService {
    store: Arc<dyn TwinStore>,
    delivery: Arc<dyn DeliveryClient>,
    policy: DeliveryPolicy,
}

impl DeliveryService {
    pub fn new(
        store: Arc<dyn TwinStore>,
        delivery: Arc<dyn DeliveryClient>,
        policy: DeliveryPolicy,
    ) -> Self {
        Self {
            store,
            delivery,
            policy,
        }
    }

    pub fn delivery(&self) -> Arc<dyn DeliveryClient> {
        self.delivery.clone()
    }

    /// Backward-compatible alias for the active delivery adapter.
    pub fn slack(&self) -> Arc<dyn DeliveryClient> {
        self.delivery.clone()
    }

    /// Create draft after compile and optionally DM / auto-queue publish.
    ///
    /// `allow_notify`: when false, store draft/state only — no chat DM (used for
    /// continuous recompiles between scheduled status windows).
    pub async fn start_after_compile(
        &self,
        twin: &Twin,
        snap: &LedgerSnapshot,
        draft_text: &str,
        now: chrono::DateTime<Utc>,
    ) -> TwinResult<DraftDelivery> {
        Ok(self
            .start_after_compile_opts(twin, snap, draft_text, now, true, false)
            .await?
            .draft)
    }

    pub async fn start_after_compile_opts(
        &self,
        twin: &Twin,
        snap: &LedgerSnapshot,
        draft_text: &str,
        now: chrono::DateTime<Utc>,
        allow_notify: bool,
        force_notify: bool,
    ) -> TwinResult<DeliveryStartResult> {
        // Idempotent: one draft per ledger — refresh text if still open.
        // Empty placeholder → real items: reuse same draft + run Notify Policy (no second put).
        if let Some(mut existing) = self
            .store
            .get_draft_by_ledger(&twin.tenant_id, &snap.ledger_id)
            .await?
        {
            let was_emptyish = existing.draft_text.contains("No code or ticket signals")
                || existing.draft_text.contains("nothing invented")
                || existing.draft_text.trim().is_empty();
            let now_has_items =
                !snap.ledger.items.is_empty() || !snap.ledger.open_blockers.is_empty();
            let openish = matches!(
                existing.status,
                DraftStatus::Pending
                    | DraftStatus::ForceHuman
                    | DraftStatus::Edited
                    | DraftStatus::Shadow
            );
            if openish {
                existing.draft_text = draft_text.to_string();
                existing.updated_at = now;
            }
            let already_dm = !existing.slack_dm_ts.is_empty();
            let upgrade_empty_to_items = was_emptyish && now_has_items && !already_dm && openish;

            if !upgrade_empty_to_items {
                if openish {
                    self.store.update_draft(existing.clone()).await?;
                }
                return Ok(DeliveryStartResult {
                    draft: existing,
                    dm_sent: already_dm,
                    suppressed: if already_dm {
                        None
                    } else {
                        Some("existing_draft")
                    },
                });
            }

            // --- Upgrade path: same draft_id/ledger, notify if policy allows ---
            let in_shadow = twin.is_in_shadow(now);
            let mut notify_state = load_notify_state(twin);
            let decision = decide_notify(
                &snap.ledger,
                &notify_state,
                now,
                self.policy.max_dms_per_day,
                allow_notify,
                force_notify,
                in_shadow,
            );
            let mut status =
                initial_draft_status(in_shadow, snap.confidence_rollup, twin.high_auto_publish);
            if (!allow_notify && !force_notify) && !in_shadow {
                if status == DraftStatus::PublishQueued {
                    status = DraftStatus::Pending;
                }
            }
            let mut will_dm = decision.allow_dm
                && matches!(
                    status,
                    DraftStatus::Pending | DraftStatus::ForceHuman | DraftStatus::PublishQueued
                );
            if matches!(
                decision.suppress,
                Some(SuppressReason::Unchanged)
                    | Some(SuppressReason::DailyCap)
                    | Some(SuppressReason::Empty)
                    | Some(SuppressReason::Quiet)
                    | Some(SuppressReason::Shadow)
            ) {
                will_dm = false;
            }
            if in_shadow {
                will_dm = false;
                status = DraftStatus::Shadow;
            }
            existing.status = status;
            existing.veto_deadline = match (status, snap.confidence_rollup) {
                (DraftStatus::Pending | DraftStatus::ForceHuman, ConfidenceTier::Medium) => {
                    Some(now + Duration::seconds(self.policy.medium_veto_window_secs))
                }
                (DraftStatus::Pending | DraftStatus::ForceHuman, ConfidenceTier::Blocker) => {
                    Some(now + Duration::seconds(self.policy.blocker_veto_window_secs))
                }
                (DraftStatus::Pending, ConfidenceTier::High) => {
                    Some(now + Duration::seconds(self.policy.medium_veto_window_secs))
                }
                _ => existing.veto_deadline,
            };

            let mut dm_sent = false;
            if will_dm && matches!(status, DraftStatus::Pending | DraftStatus::ForceHuman) {
                let chat_user = resolve_chat_user_id(
                    self.store.as_ref(),
                    twin,
                    self.delivery.adapter_kind(),
                )
                .await?;
                let deadline_s = existing
                    .veto_deadline
                    .map(|d| twin_core::format_ist_list(d))
                    .unwrap_or_else(|| "none".into());
                match self
                    .delivery
                    .post_draft_dm(&chat_user, &existing.draft_id, draft_text, &deadline_s)
                    .await
                {
                    Ok(dm) => {
                        existing.slack_dm_channel = dm.channel;
                        existing.slack_dm_ts = dm.ts;
                        dm_sent = true;
                        record_dm_sent(&mut notify_state, &decision.fingerprint, now);
                        let mut t = twin.clone();
                        write_notify_state(&mut t, &notify_state);
                        let _ = self.store.upsert_twin(t).await;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "upgrade dm failed; draft text still upgraded");
                    }
                }
            } else if decision.suppress == Some(SuppressReason::Unchanged) {
                notify_state.last_fingerprint = decision.fingerprint.clone();
                let mut t = twin.clone();
                write_notify_state(&mut t, &notify_state);
                let _ = self.store.upsert_twin(t).await;
            }
            existing.updated_at = now;
            self.store.update_draft(existing.clone()).await?;
            return Ok(DeliveryStartResult {
                draft: existing,
                dm_sent,
                suppressed: if dm_sent {
                    None
                } else {
                    decision.suppress.map(|s| s.as_str()).or(Some("no_dm"))
                },
            });
        }

        let in_shadow = twin.is_in_shadow(now);
        let mut notify_state = load_notify_state(twin);
        let decision = decide_notify(
            &snap.ledger,
            &notify_state,
            now,
            self.policy.max_dms_per_day,
            allow_notify,
            force_notify,
            in_shadow,
        );

        // Quiet recompiles stay shadow-like for notify purposes when !allow_notify
        let mut status =
            initial_draft_status(in_shadow, snap.confidence_rollup, twin.high_auto_publish);
        if (!allow_notify && !force_notify) && !in_shadow {
            if status == DraftStatus::PublishQueued {
                status = DraftStatus::Pending;
            }
        }
        // Notify Policy v1: do not DM — still keep draft for UI
        let mut will_dm = decision.allow_dm
            && matches!(
                status,
                DraftStatus::Pending | DraftStatus::ForceHuman | DraftStatus::PublishQueued
            );
        if decision.suppress == Some(SuppressReason::Unchanged)
            || decision.suppress == Some(SuppressReason::DailyCap)
            || decision.suppress == Some(SuppressReason::Empty)
            || decision.suppress == Some(SuppressReason::Quiet)
        {
            will_dm = false;
        }
        if in_shadow || decision.suppress == Some(SuppressReason::Shadow) {
            will_dm = false;
            status = DraftStatus::Shadow;
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

        // Shadow or quiet / suppressed: no chat delivery calls
        if status == DraftStatus::Shadow || !will_dm {
            self.store.put_draft(draft.clone()).await?;
            // Remember fingerprint when we suppress unchanged so we stay quiet
            if decision.suppress == Some(SuppressReason::Unchanged) {
                notify_state.last_fingerprint = decision.fingerprint.clone();
                let mut t = twin.clone();
                write_notify_state(&mut t, &notify_state);
                let _ = self.store.upsert_twin(t).await;
            }
            let suppressed = decision.suppress.map(|s| s.as_str()).or(Some("no_dm"));
            return Ok(DeliveryStartResult {
                draft,
                dm_sent: false,
                suppressed,
            });
        }

        // High auto-publish: optional short DM then queue
        if status == DraftStatus::PublishQueued
            && snap.confidence_rollup == ConfidenceTier::High
            && twin.high_auto_publish
        {
            if let Ok(chat_user) = resolve_chat_user_id(
                self.store.as_ref(),
                twin,
                self.delivery.adapter_kind(),
            )
            .await
            {
                if let Ok(dm) = self
                    .delivery
                    .post_dm(
                        &chat_user,
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
            record_dm_sent(&mut notify_state, &decision.fingerprint, now);
            let mut t = twin.clone();
            write_notify_state(&mut t, &notify_state);
            let _ = self.store.upsert_twin(t).await;
            let dm_sent = !draft.slack_dm_ts.is_empty();
            return Ok(DeliveryStartResult {
                draft,
                dm_sent,
                suppressed: None,
            });
        }

        // Medium / High(no auto) / Blocker: DM when policy allows
        let mut dm_sent = false;
        if matches!(status, DraftStatus::Pending | DraftStatus::ForceHuman) {
            let chat_user = resolve_chat_user_id(
                self.store.as_ref(),
                twin,
                self.delivery.adapter_kind(),
            )
            .await?;

            let deadline_s = veto_deadline
                .map(|d| twin_core::format_ist_list(d))
                .unwrap_or_else(|| "none".into());
            match self
                .delivery
                .post_draft_dm(&chat_user, &draft_id, draft_text, &deadline_s)
                .await
            {
                Ok(dm) => {
                    draft.slack_dm_channel = dm.channel;
                    draft.slack_dm_ts = dm.ts;
                    dm_sent = true;
                    let _ = apply_delivery_event(
                        draft.status,
                        &DeliveryEvent::DmSent,
                        snap.confidence_rollup,
                        twin.high_auto_publish,
                    );
                    record_dm_sent(&mut notify_state, &decision.fingerprint, now);
                    let mut t = twin.clone();
                    write_notify_state(&mut t, &notify_state);
                    let _ = self.store.upsert_twin(t).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "dm failed; draft still stored");
                }
            }
        }

        draft.status = status;
        self.store.put_draft(draft.clone()).await?;
        Ok(DeliveryStartResult {
            draft,
            dm_sent,
            suppressed: if dm_sent {
                None
            } else {
                decision.suppress.map(|s| s.as_str())
            },
        })
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

        // Channel post; if bot not in channel, fall back to IC DM so Approve still works in pilot.
        let post: DeliveryPostResult = match self.delivery.post_channel(&channel, &body).await {
            Ok(p) => p,
            Err(e) => {
                let msg = e.to_string();
                let not_in_channel = msg.contains("not_in_channel")
                    || msg.contains("channel_not_found")
                    || msg.contains("is_archived")
                    || channel == "C_TEAM"
                    || channel.is_empty();
                if not_in_channel {
                    tracing::warn!(
                        %channel,
                        error = %msg,
                        "channel publish failed; approving via DM fallback"
                    );
                    let chat_user = resolve_chat_user_id(
                        self.store.as_ref(),
                        twin,
                        self.delivery.adapter_kind(),
                    )
                    .await
                    .unwrap_or_else(|_| format!("U_{}", twin.subject_id));
                    let dm_body = format!(
                        "✅ *Approved* (channel share skipped — invite the bot to `{channel}` for team posts).\n\n{body}"
                    );
                    match self.delivery.post_dm(&chat_user, &dm_body).await {
                        Ok(dm) => {
                            // Record as published with dm channel id so Approve succeeds.
                            let rec = PublishRecord {
                                tenant_id: draft.tenant_id.clone(),
                                publish_id: format!("pub_{}", Uuid::new_v4()),
                                ledger_id: draft.ledger_id.clone(),
                                draft_id: draft.draft_id.clone(),
                                channel_id: format!("dm_fallback:{}", dm.channel),
                                slack_ts: dm.ts,
                                body_hash: hash.clone(),
                                published_at: Utc::now(),
                            };
                            let _ = self.store.put_publish_if_absent(rec.clone()).await?;
                            draft.status = DraftStatus::Published;
                            draft.updated_at = Utc::now();
                            self.store.update_draft(draft.clone()).await?;
                            return Ok(Some(rec));
                        }
                        Err(e2) => {
                            draft.status = DraftStatus::PublishFailed;
                            draft.updated_at = Utc::now();
                            self.store.update_draft(draft.clone()).await?;
                            return Err(TwinError::Egress(format!(
                                "channel post failed ({msg}); DM fallback also failed ({e2}). Invite bot to channel {channel} or check vault token."
                            )));
                        }
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_slack::MockSlackClient;
    use twin_core::ids::person_twin_id;
    use twin_core::model::{
        ConfidenceTier, LedgerItem, LedgerPeriod, LedgerSnapshot, SlackUserMap, StatusLedger,
        TwinKind,
    };
    use twin_core::store::InMemoryTwinStore;

    fn twin_now() -> (Twin, chrono::DateTime<Utc>) {
        let now = Utc::now();
        let twin = Twin {
            tenant_id: "ten_t".into(),
            twin_id: person_twin_id("gu_a"),
            twin_kind: TwinKind::Person,
            subject_id: "gu_a".into(),
            display_name: "A".into(),
            timezone: twin_core::DISPLAY_TIMEZONE.into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        };
        (twin, now)
    }

    fn snap_with_items(
        twin: &Twin,
        now: chrono::DateTime<Utc>,
        items: Vec<LedgerItem>,
        ledger_id: &str,
    ) -> (LedgerSnapshot, String) {
        let ledger = StatusLedger {
            tenant_id: twin.tenant_id.clone(),
            person_id: twin.subject_id.clone(),
            period: LedgerPeriod {
                start: now - chrono::Duration::hours(24),
                end: now,
            },
            confidence_rollup: ConfidenceTier::Medium,
            items,
            open_blockers: vec![],
            graph_as_of: now,
            compiled_at: now,
        };
        let draft_text = if ledger.items.is_empty() {
            "Status update for you · 2026-08-01\nWindow: work in progress.\n\n• No code or ticket signals in this window (nothing invented).".into()
        } else {
            format!(
                "Status update\n\n• {}\n\nApprove · Edit · Don't send",
                ledger.items[0].summary
            )
        };
        let snap = LedgerSnapshot {
            tenant_id: twin.tenant_id.clone(),
            ledger_id: ledger_id.into(),
            twin_id: twin.twin_id.clone(),
            period_start: now - chrono::Duration::hours(24),
            period_end: now,
            confidence_rollup: ConfidenceTier::Medium,
            ledger,
            graph_as_of: now,
            compiled_at: now,
        };
        (snap, draft_text)
    }

    #[tokio::test]
    async fn empty_placeholder_upgrades_to_items() {
        let store = InMemoryTwinStore::new();
        let slack = MockSlackClient::new();
        let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
        let (twin, now) = twin_now();
        store.upsert_twin(twin.clone()).await.unwrap();
        store
            .put_slack_map(SlackUserMap {
                tenant_id: twin.tenant_id.clone(),
                global_user_id: twin.subject_id.clone(),
                slack_user_id: "U_TEST".into(),
                slack_team_id: String::new(),
            })
            .await
            .unwrap();

        let ledger_id = "led_upgrade_test";
        let (empty_snap, empty_text) = snap_with_items(&twin, now, vec![], ledger_id);
        store.put_ledger(empty_snap.clone()).await.unwrap();
        let r1 = service
            .start_after_compile_opts(&twin, &empty_snap, &empty_text, now, true, false)
            .await
            .unwrap();
        assert!(!r1.dm_sent);
        assert!(r1.draft.draft_text.contains("nothing invented"));

        let items = vec![LedgerItem {
            kind: "pr".into(),
            resource_id: "org/r/pr/1".into(),
            node_id: "pr:org/r/pr/1".into(),
            summary: "Open pr: real work".into(),
            confidence: ConfidenceTier::Medium,
            evidence_refs: vec!["edge:e1".into()],
        }];
        let (full_snap, full_text) = snap_with_items(&twin, now, items, ledger_id);
        store.put_ledger(full_snap.clone()).await.unwrap();
        let r2 = service
            .start_after_compile_opts(&twin, &full_snap, &full_text, now, true, false)
            .await
            .unwrap();
        assert!(
            r2.dm_sent || r2.draft.draft_text.contains("real work"),
            "empty→items should upgrade draft and allow notify path: {:?}",
            r2
        );
        assert!(!r2.draft.draft_text.contains("nothing invented"));
    }
}
