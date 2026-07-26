//! Notify Policy v1 — continuous ingest, rare Slack (developer-first).
//!
//! Rules:
//! - Content fingerprint: no DM if status story unchanged
//! - Max N DMs per person per UTC day (default 1) unless new blocker or force
//! - Empty ledgers never DM (caller also skips)

use crate::ids::body_hash;
use crate::model::{ConfidenceTier, StatusLedger, Twin};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

pub const CONFIG_KEY: &str = "notify_v1";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotifyState {
    pub last_fingerprint: String,
    /// RFC3339 of last status DM sent
    pub last_dm_at: Option<String>,
    /// UTC date YYYY-MM-DD of last DM day counter
    pub last_dm_day: String,
    pub dms_today: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuppressReason {
    Unchanged,
    DailyCap,
    Empty,
    Quiet,
    Shadow,
}

impl SuppressReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::DailyCap => "daily_cap",
            Self::Empty => "empty",
            Self::Quiet => "quiet",
            Self::Shadow => "shadow",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotifyDecision {
    pub allow_dm: bool,
    pub fingerprint: String,
    pub has_new_blocker: bool,
    pub suppress: Option<SuppressReason>,
}

/// Stable fingerprint of *substantive* status content (not wall-clock window).
/// Same open PR with same lifecycle → same hash → no re-DM.
pub fn ledger_fingerprint(ledger: &StatusLedger) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("rollup:{}", ledger.confidence_rollup.as_str()));
    let mut items: Vec<_> = ledger.items.iter().collect();
    items.sort_by(|a, b| {
        a.resource_id
            .cmp(&b.resource_id)
            .then_with(|| a.node_id.cmp(&b.node_id))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    for it in items {
        parts.push(format!(
            "i:{}|{}|{}|{}",
            it.kind,
            it.resource_id,
            it.node_id,
            normalize_summary(&it.summary)
        ));
    }
    let mut blockers: Vec<_> = ledger.open_blockers.iter().collect();
    blockers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
    for b in blockers {
        parts.push(format!(
            "b:{}|{}",
            b.node_id,
            normalize_summary(&b.summary)
        ));
    }
    let joined = parts.join("\n");
    let mut h = Sha256::new();
    h.update(joined.as_bytes());
    hex::encode(h.finalize())
}

fn normalize_summary(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn ledger_is_empty(ledger: &StatusLedger) -> bool {
    ledger.items.is_empty() && ledger.open_blockers.is_empty()
}

pub fn load_notify_state(twin: &Twin) -> NotifyState {
    twin.config_json
        .get(CONFIG_KEY)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

pub fn write_notify_state(twin: &mut Twin, state: &NotifyState) {
    let mut cfg = twin.config_json.clone();
    if !cfg.is_object() {
        cfg = json!({});
    }
    if let Some(obj) = cfg.as_object_mut() {
        obj.insert(
            CONFIG_KEY.to_string(),
            serde_json::to_value(state).unwrap_or(json!({})),
        );
    }
    twin.config_json = cfg;
    twin.updated_at = Utc::now();
}

/// Decide whether to Slack-DM this draft.
///
/// `force_notify`: demo / explicit "send now" bypasses fingerprint + daily cap.
/// `allow_notify`: scheduler quiet compile — no DM.
pub fn decide_notify(
    ledger: &StatusLedger,
    state: &NotifyState,
    now: DateTime<Utc>,
    max_dms_per_day: u32,
    allow_notify: bool,
    force_notify: bool,
    in_shadow: bool,
) -> NotifyDecision {
    let fingerprint = ledger_fingerprint(ledger);
    let has_blocker = !ledger.open_blockers.is_empty()
        || ledger.confidence_rollup == ConfidenceTier::Blocker;
    let prev_had_blocker = state
        .last_fingerprint
        .contains("b:")
        || state.last_fingerprint.contains("rollup:blocker");
    let has_new_blocker = has_blocker
        && (state.last_fingerprint.is_empty()
            || !state.last_fingerprint.contains("b:")
            || fingerprint != state.last_fingerprint);

    if in_shadow {
        return NotifyDecision {
            allow_dm: false,
            fingerprint,
            has_new_blocker,
            suppress: Some(SuppressReason::Shadow),
        };
    }
    if !allow_notify && !force_notify {
        return NotifyDecision {
            allow_dm: false,
            fingerprint,
            has_new_blocker,
            suppress: Some(SuppressReason::Quiet),
        };
    }
    if ledger_is_empty(ledger) {
        return NotifyDecision {
            allow_dm: false,
            fingerprint,
            has_new_blocker: false,
            suppress: Some(SuppressReason::Empty),
        };
    }
    if force_notify {
        return NotifyDecision {
            allow_dm: true,
            fingerprint,
            has_new_blocker,
            suppress: None,
        };
    }
    // Change-only: identical story → silence
    if !state.last_fingerprint.is_empty() && state.last_fingerprint == fingerprint {
        return NotifyDecision {
            allow_dm: false,
            fingerprint,
            has_new_blocker: false,
            suppress: Some(SuppressReason::Unchanged),
        };
    }
    // Daily cap (UTC) unless new blocker appeared
    let day = now.format("%Y-%m-%d").to_string();
    let dms_today = if state.last_dm_day == day {
        state.dms_today
    } else {
        0
    };
    if max_dms_per_day > 0 && dms_today >= max_dms_per_day && !has_new_blocker {
        return NotifyDecision {
            allow_dm: false,
            fingerprint,
            has_new_blocker,
            suppress: Some(SuppressReason::DailyCap),
        };
    }
    // New blocker can break daily cap; still require content change if fingerprint same
    // (handled above). If fingerprint differs due to new blocker, allow.
    let _ = prev_had_blocker;

    NotifyDecision {
        allow_dm: true,
        fingerprint,
        has_new_blocker,
        suppress: None,
    }
}

pub fn record_dm_sent(state: &mut NotifyState, fingerprint: &str, now: DateTime<Utc>) {
    let day = now.format("%Y-%m-%d").to_string();
    if state.last_dm_day == day {
        state.dms_today = state.dms_today.saturating_add(1);
    } else {
        state.last_dm_day = day;
        state.dms_today = 1;
    }
    state.last_fingerprint = fingerprint.to_string();
    state.last_dm_at = Some(now.to_rfc3339());
}

/// When we suppress DM but still store a draft, remember fingerprint so we don't
/// re-evaluate as "new" on every quiet compile incorrectly.
/// Only update fingerprint on successful DM or explicit suppress-unchanged retention.
pub fn record_suppressed_unchanged(state: &mut NotifyState, fingerprint: &str) {
    if state.last_fingerprint.is_empty() {
        state.last_fingerprint = fingerprint.to_string();
    }
}

/// Helper for tests / diagnostics.
pub fn short_fp(ledger: &StatusLedger) -> String {
    let f = ledger_fingerprint(ledger);
    f.chars().take(12).collect()
}

#[allow(dead_code)]
pub fn content_body_hash(text: &str) -> String {
    body_hash(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BlockerItem, ConfidenceTier, LedgerItem, LedgerPeriod};

    fn sample_ledger(pr: &str, conf: ConfidenceTier) -> StatusLedger {
        StatusLedger {
            tenant_id: "t".into(),
            person_id: "gu_a".into(),
            period: LedgerPeriod {
                start: Utc::now() - chrono::Duration::hours(1),
                end: Utc::now(),
            },
            confidence_rollup: conf,
            items: vec![LedgerItem {
                kind: "pr".into(),
                resource_id: pr.into(),
                node_id: format!("pr:{pr}"),
                summary: format!("Open PR {pr}"),
                confidence: conf,
                evidence_refs: vec![],
            }],
            open_blockers: vec![],
            graph_as_of: Utc::now(),
            compiled_at: Utc::now(),
        }
    }

    #[test]
    fn same_pr_same_fingerprint() {
        let a = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        let b = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        assert_eq!(ledger_fingerprint(&a), ledger_fingerprint(&b));
    }

    #[test]
    fn lifecycle_change_new_fingerprint() {
        let mut a = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        let mut b = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        b.items[0].summary = "Merged PR org/r/pr/1".into();
        assert_ne!(ledger_fingerprint(&a), ledger_fingerprint(&b));
        a.open_blockers.push(BlockerItem {
            node_id: "pr:x".into(),
            summary: "blocked".into(),
            confidence: ConfidenceTier::Blocker,
            evidence_refs: vec![],
        });
        assert_ne!(ledger_fingerprint(&a), ledger_fingerprint(&b));
    }

    #[test]
    fn suppress_unchanged() {
        let led = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        let fp = ledger_fingerprint(&led);
        let state = NotifyState {
            last_fingerprint: fp,
            last_dm_at: Some(Utc::now().to_rfc3339()),
            last_dm_day: Utc::now().format("%Y-%m-%d").to_string(),
            dms_today: 1,
        };
        let d = decide_notify(&led, &state, Utc::now(), 1, true, false, false);
        assert!(!d.allow_dm);
        assert_eq!(d.suppress, Some(SuppressReason::Unchanged));
    }

    #[test]
    fn daily_cap() {
        let led = sample_ledger("org/r/pr/2", ConfidenceTier::Medium);
        let state = NotifyState {
            last_fingerprint: "other".into(),
            last_dm_at: Some(Utc::now().to_rfc3339()),
            last_dm_day: Utc::now().format("%Y-%m-%d").to_string(),
            dms_today: 1,
        };
        let d = decide_notify(&led, &state, Utc::now(), 1, true, false, false);
        assert!(!d.allow_dm);
        assert_eq!(d.suppress, Some(SuppressReason::DailyCap));
    }

    #[test]
    fn force_bypasses() {
        let led = sample_ledger("org/r/pr/1", ConfidenceTier::Medium);
        let fp = ledger_fingerprint(&led);
        let state = NotifyState {
            last_fingerprint: fp,
            last_dm_day: Utc::now().format("%Y-%m-%d").to_string(),
            dms_today: 5,
            last_dm_at: None,
        };
        let d = decide_notify(&led, &state, Utc::now(), 1, true, true, false);
        assert!(d.allow_dm);
    }
}
