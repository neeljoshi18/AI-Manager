//! Deterministic confidence scoring (TAS §4.3). Never invents work items.

use crate::model::{BlockerItem, ConfidenceTier, LedgerItem, StatusLedger};

/// Score a single work item from lifecycle + activity signals.
///
/// | Signal | Tier |
/// | Merged/closed PR or closed ticket with evidence | High |
/// | Open PR with commits/activity | Medium |
/// | Open BLOCKS / BLOCKED_BY | Blocker |
/// | No activity | Medium empty (caller) |
pub fn score_item_confidence(lifecycle: &str, has_activity: bool) -> ConfidenceTier {
    let lc = lifecycle.to_ascii_uppercase();
    match lc.as_str() {
        "MERGED" | "CLOSED" | "DONE" | "RESOLVED" => ConfidenceTier::High,
        "OPEN" | "IN_PROGRESS" | "DRAFT" if has_activity => ConfidenceTier::Medium,
        "OPEN" | "IN_PROGRESS" | "DRAFT" => ConfidenceTier::Medium,
        _ if has_activity => ConfidenceTier::Medium,
        _ => ConfidenceTier::Medium,
    }
}

/// Rollup rules (TAS §4.3):
/// - Any Blocker item or open blocker → Blocker
/// - Else if any High and no Medium-only conflict → High when all substantive are High
/// - Else → Medium
/// - Empty activity → Medium with empty items
pub fn roll_up_confidence(items: &[LedgerItem], open_blockers: &[BlockerItem]) -> ConfidenceTier {
    if !open_blockers.is_empty()
        || items
            .iter()
            .any(|i| i.confidence == ConfidenceTier::Blocker)
        || open_blockers
            .iter()
            .any(|b| b.confidence == ConfidenceTier::Blocker)
    {
        return ConfidenceTier::Blocker;
    }

    if items.is_empty() {
        return ConfidenceTier::Medium;
    }

    if items.iter().any(|i| i.confidence == ConfidenceTier::Blocker) {
        return ConfidenceTier::Blocker;
    }

    let all_high = items
        .iter()
        .all(|i| i.confidence == ConfidenceTier::High);
    if all_high {
        return ConfidenceTier::High;
    }

    if items.iter().any(|i| i.confidence == ConfidenceTier::High)
        && items
            .iter()
            .all(|i| matches!(i.confidence, ConfidenceTier::High | ConfidenceTier::Medium))
    {
        // Prefer Medium when mixed High+Medium (not "all substantive High")
        let only_high_and_empty_medium = items
            .iter()
            .filter(|i| i.confidence == ConfidenceTier::Medium)
            .all(|i| i.summary.is_empty());
        if only_high_and_empty_medium {
            return ConfidenceTier::High;
        }
        return ConfidenceTier::Medium;
    }

    ConfidenceTier::Medium
}

/// Apply rollup onto a ledger (mutates confidence_rollup).
pub fn apply_rollup(ledger: &mut StatusLedger) {
    ledger.confidence_rollup = roll_up_confidence(&ledger.items, &ledger.open_blockers);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn item(conf: ConfidenceTier) -> LedgerItem {
        LedgerItem {
            kind: "pr".into(),
            resource_id: "r".into(),
            node_id: "n".into(),
            summary: "s".into(),
            confidence: conf,
            evidence_refs: vec!["event:e1".into()],
        }
    }

    #[test]
    fn empty_is_medium() {
        assert_eq!(roll_up_confidence(&[], &[]), ConfidenceTier::Medium);
    }

    #[test]
    fn all_high() {
        assert_eq!(
            roll_up_confidence(&[item(ConfidenceTier::High), item(ConfidenceTier::High)], &[]),
            ConfidenceTier::High
        );
    }

    #[test]
    fn mixed_is_medium() {
        assert_eq!(
            roll_up_confidence(
                &[item(ConfidenceTier::High), item(ConfidenceTier::Medium)],
                &[]
            ),
            ConfidenceTier::Medium
        );
    }

    #[test]
    fn blocker_wins() {
        let blockers = vec![BlockerItem {
            node_id: "issue:1".into(),
            summary: "blocked".into(),
            confidence: ConfidenceTier::Blocker,
            evidence_refs: vec!["edge:blocks:1".into()],
        }];
        assert_eq!(
            roll_up_confidence(&[item(ConfidenceTier::High)], &blockers),
            ConfidenceTier::Blocker
        );
    }

    #[test]
    fn score_merged_high() {
        assert_eq!(
            score_item_confidence("MERGED", true),
            ConfidenceTier::High
        );
        assert_eq!(score_item_confidence("OPEN", true), ConfidenceTier::Medium);
    }

    #[test]
    fn apply_rollup_mutates() {
        let mut ledger = StatusLedger {
            tenant_id: "t".into(),
            person_id: "p".into(),
            period: crate::model::LedgerPeriod {
                start: Utc::now(),
                end: Utc::now(),
            },
            confidence_rollup: ConfidenceTier::Medium,
            items: vec![item(ConfidenceTier::High)],
            open_blockers: vec![],
            graph_as_of: Utc::now(),
            compiled_at: Utc::now(),
        };
        apply_rollup(&mut ledger);
        assert_eq!(ledger.confidence_rollup, ConfidenceTier::High);
    }
}
