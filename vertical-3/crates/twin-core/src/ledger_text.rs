//! Structure-first draft text from ledger JSON (no LLM inventing items).
//! Human-facing prose for scheduled status windows; evidence stays on ledger JSON.

use crate::model::{ConfidenceTier, LedgerItem, StatusLedger};

/// Render friendly status for a **batched window** — emphasize outcomes, not every micro-event.
pub fn render_draft_text(ledger: &StatusLedger) -> String {
    let mut lines = Vec::new();
    let who = human_name(&ledger.person_id);
    let day = ledger.period.end.date_naive();
    let conf = match ledger.confidence_rollup {
        ConfidenceTier::High => "shipped / closed work",
        ConfidenceTier::Medium => "work in progress",
        ConfidenceTier::Blocker => "blocked — needs attention",
    };
    let win = format_window(ledger.period.start, ledger.period.end);

    lines.push(format!("Status update for {who} · {day}"));
    lines.push(format!("Lookback: {win} · {conf}."));
    lines.push(String::new());

    if ledger.items.is_empty() && ledger.open_blockers.is_empty() {
        lines.push(
            "• No code or ticket signals in this window (nothing invented)."
                .into(),
        );
        return lines.join("\n");
    }

    // Prefer High (merged/closed) first — those are the actionable outcomes
    let mut highs: Vec<&LedgerItem> = ledger
        .items
        .iter()
        .filter(|i| i.confidence == ConfidenceTier::High)
        .collect();
    let mut mediums: Vec<&LedgerItem> = ledger
        .items
        .iter()
        .filter(|i| i.confidence == ConfidenceTier::Medium)
        .collect();
    highs.truncate(5);
    // Cap medium noise — show up to 3 open items after outcomes
    mediums.truncate(3);

    if !highs.is_empty() {
        lines.push("*Completed / closed*".into());
        for item in &highs {
            lines.push(format!("• {}", clean_summary(&item.summary)));
        }
        lines.push(String::new());
    }

    if !mediums.is_empty() {
        lines.push("*Still open*".into());
        for item in &mediums {
            lines.push(format!("• {}", clean_summary(&item.summary)));
        }
        lines.push(String::new());
    }

    for b in ledger.open_blockers.iter().take(3) {
        let s = if b.summary.is_empty() {
            b.node_id.clone()
        } else {
            b.summary.clone()
        };
        lines.push(format!("• Blocker: {s}"));
    }

    if matches!(ledger.confidence_rollup, ConfidenceTier::Blocker) {
        lines.push("Please unblock or reply before this posts to the team channel.".into());
    } else {
        lines.push(
            "Scheduled summary (only when something changes). Approve · Edit · Don't send."
                .into(),
        );
    }

    lines.join("\n")
}

fn clean_summary(s: &str) -> String {
    s.trim().to_string()
}

fn format_window(start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>) -> String {
    let secs = (end - start).num_seconds().max(0);
    let label = if secs >= 86_400 {
        format!("{}d", (secs + 43_200) / 86_400)
    } else if secs >= 3600 {
        format!("{}h", (secs + 1800) / 3600)
    } else {
        format!("{}m", (secs + 30) / 60)
    };
    format!(
        "{label} ({} → {} UTC)",
        start.format("%m-%d %H:%M"),
        end.format("%m-%d %H:%M")
    )
}

fn human_name(person_id: &str) -> String {
    let s = person_id
        .strip_prefix("gu_")
        .or_else(|| person_id.strip_prefix("person:"))
        .unwrap_or(person_id);
    if s.is_empty() {
        return "you".to_string();
    }
    if s.contains('-') && s.len() > 20 {
        return "you".to_string();
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LedgerPeriod;
    use chrono::Utc;

    #[test]
    fn never_invents_items() {
        let ledger = StatusLedger {
            tenant_id: "t".into(),
            person_id: "gu_a".into(),
            period: LedgerPeriod {
                start: Utc::now(),
                end: Utc::now(),
            },
            confidence_rollup: ConfidenceTier::Medium,
            items: vec![],
            open_blockers: vec![],
            graph_as_of: Utc::now(),
            compiled_at: Utc::now(),
        };
        let t = render_draft_text(&ledger);
        assert!(t.contains("nothing invented"));
    }

    #[test]
    fn prioritizes_high() {
        let ledger = StatusLedger {
            tenant_id: "t".into(),
            person_id: "gu_alice".into(),
            period: LedgerPeriod {
                start: Utc::now(),
                end: Utc::now(),
            },
            confidence_rollup: ConfidenceTier::High,
            items: vec![
                LedgerItem {
                    kind: "pr".into(),
                    resource_id: "r1".into(),
                    node_id: "n1".into(),
                    summary: "Open pr: WIP".into(),
                    confidence: ConfidenceTier::Medium,
                    evidence_refs: vec![],
                },
                LedgerItem {
                    kind: "pr".into(),
                    resource_id: "r2".into(),
                    node_id: "n2".into(),
                    summary: "Merged PR #7: fix auth".into(),
                    confidence: ConfidenceTier::High,
                    evidence_refs: vec!["event:e1".into()],
                },
            ],
            open_blockers: vec![],
            graph_as_of: Utc::now(),
            compiled_at: Utc::now(),
        };
        let t = render_draft_text(&ledger);
        assert!(t.find("Merged PR #7").unwrap() < t.find("WIP").unwrap());
        assert!(t.contains("Completed"));
    }
}
