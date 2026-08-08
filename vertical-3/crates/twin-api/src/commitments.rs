//! Commitment tracker — plain-English accountability from chat promises.
//!
//! Inspired by Miten's Commit (promises you/they made, quiet until done) and
//! Minimi (open loops that close when activity shows resolution).
//!
//! **Not** a Jira/Linear clone. Linear/Jira create *issues* when someone
//! explicitly files them (slash, emoji, automation). They do **not** ambiently
//! detect "I'll send the deck by Friday" and close it when the channel says done.
//!
//! We store **commitments** (human promises), not tickets:
//! - who promised
//! - who is owed (optional)
//! - what in plain English
//! - open → done when chat or human says so

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub const COMMITMENTS_KV: &str = "commitment_ledger";
pub const COMMITMENTS_MAX: usize = 300;
pub const PREVIEW_MAX: usize = 280;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub id: String,
    pub tenant_id: String,
    /// Who made the promise (subject / display / slack map key)
    pub promiser: String,
    /// Display name for promiser when known (non-technical UI)
    #[serde(default)]
    pub promiser_label: Option<String>,
    /// Who they promised (optional)
    pub promisee: Option<String>,
    #[serde(default)]
    pub promisee_label: Option<String>,
    /// Plain English description
    pub text: String,
    pub status: String, // open | done | dismissed
    pub source: String, // slack_channel | slack_dm | explicit | digest
    pub channel: Option<String>,
    pub evidence: Vec<String>,
    pub confidence: f32,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolve_evidence: Option<String>,
    /// Optional one-way Linear export (not required)
    #[serde(default)]
    pub linear_issue_id: Option<String>,
    #[serde(default)]
    pub linear_url: Option<String>,
}

impl Commitment {
    pub fn promiser_display(&self) -> &str {
        self.promiser_label
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(self.promiser.as_str())
    }

    pub fn promisee_display(&self) -> Option<&str> {
        self.promisee_label
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.promisee.as_deref())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "tenant_id": self.tenant_id,
            "promiser": self.promiser,
            "promiser_label": self.promiser_label,
            "promisee": self.promisee,
            "promisee_label": self.promisee_label,
            "text": self.text,
            "status": self.status,
            "source": self.source,
            "channel": self.channel,
            "evidence": self.evidence,
            "confidence": self.confidence,
            "created_at": self.created_at,
            "resolved_at": self.resolved_at,
            "resolve_evidence": self.resolve_evidence,
            "linear_issue_id": self.linear_issue_id,
            "linear_url": self.linear_url,
            // Plain English for UI
            "headline": commitment_headline(self),
            "for_promiser": format!("You said you'd: {}", self.text),
            "for_promisee": self.promisee_display().map(|_p| {
                format!("{} said they'd: {}", self.promiser_display(), self.text)
            }),
        })
    }

    /// Lean storage shape (round-trip safe).
    pub fn to_storage_json(&self) -> Value {
        json!({
            "id": self.id,
            "tenant_id": self.tenant_id,
            "promiser": self.promiser,
            "promiser_label": self.promiser_label,
            "promisee": self.promisee,
            "promisee_label": self.promisee_label,
            "text": self.text,
            "status": self.status,
            "source": self.source,
            "channel": self.channel,
            "evidence": self.evidence,
            "confidence": self.confidence,
            "created_at": self.created_at,
            "resolved_at": self.resolved_at,
            "resolve_evidence": self.resolve_evidence,
            "linear_issue_id": self.linear_issue_id,
            "linear_url": self.linear_url,
        })
    }
}

pub fn commitment_headline(c: &Commitment) -> String {
    let who = c.promiser_display();
    match c.status.as_str() {
        "done" => format!("Done: {}", c.text),
        "dismissed" => format!("Dropped: {}", c.text),
        _ => {
            if let Some(to) = c.promisee_display() {
                format!("{who} owes {to} — {}", c.text)
            } else {
                format!("{who} committed — {}", c.text)
            }
        }
    }
}

/// Morning / champion digest text (plain English, Commit-inspired).
pub fn format_morning_digest(tenant_id: &str, open: &[Commitment]) -> String {
    let n = open.len();
    if n == 0 {
        return format!(
            "Good morning — no open commitments for {tenant_id}. Quiet board. Have a focused day."
        );
    }
    let mut lines = vec![format!(
        "Good morning — {n} open commitment{}:",
        if n == 1 { "" } else { "s" }
    )];
    // Cap like Commit "act on today"
    for (i, c) in open.iter().take(8).enumerate() {
        lines.push(format!("{}. {}", i + 1, commitment_headline(c)));
    }
    if n > 8 {
        lines.push(format!("…and {} more. Open Cockpit → Commitments.", n - 8));
    } else {
        lines.push("Mark done in Cockpit when finished, or say done/shipped in the channel.".into());
    }
    lines.join("\n")
}

/// Detect a commitment in free text. Returns (plain text summary, confidence, evidence tag).
pub fn extract_commitment(text: &str) -> Option<(String, f32, String)> {
    let t = text.trim();
    if t.len() < 8 {
        return None;
    }
    let hay = t.to_ascii_lowercase();

    // Resolution phrases — not new commitments
    if is_resolution_phrase(&hay) {
        return None;
    }

    // Strong commitment patterns (English)
    let patterns: &[(&str, f32)] = &[
        ("i'll ", 0.88),
        ("i will ", 0.9),
        ("i’ll ", 0.88),
        ("i can send", 0.85),
        ("i can get", 0.8),
        ("i'll send", 0.9),
        ("i will send", 0.92),
        ("i'll share", 0.88),
        ("i'll update", 0.85),
        ("i'll fix", 0.88),
        ("i'll push", 0.85),
        ("i'll merge", 0.85),
        ("i'll review", 0.85),
        ("i'll follow up", 0.88),
        ("i'll get back", 0.88),
        ("let me send", 0.8),
        ("let me get", 0.78),
        ("going to ship", 0.82),
        ("will ship", 0.82),
        ("will have it", 0.85),
        ("by eod", 0.75),
        ("by end of day", 0.78),
        ("by tomorrow", 0.78),
        ("by friday", 0.78),
        ("by monday", 0.78),
        ("tomorrow i'll", 0.88),
        ("i owe you", 0.9),
        ("i promise", 0.9),
        ("you have my word", 0.85),
        ("consider it done", 0.7),
        ("on it — will", 0.8),
        ("on it, will", 0.8),
    ];

    for (pat, conf) in patterns {
        if hay.contains(pat) {
            let summary = plain_commitment_summary(t);
            return Some((summary, *conf, format!("phrase:{pat}")));
        }
    }

    // "can you … by …" is a request, not a self-commitment — skip unless "I will"
    None
}

/// Someone saying work is finished.
pub fn is_resolution_phrase(hay: &str) -> bool {
    let done_patterns = [
        "done —",
        "done-",
        "done.",
        "it's done",
        "its done",
        "all done",
        "just shipped",
        "shipped it",
        "i shipped",
        "i sent",
        "sent it",
        "sent the",
        "merged",
        "pr is merged",
        "closed the",
        "resolved",
        "finished the",
        "completed the",
        "already done",
        "took care of",
        "handled it",
        "✓",
        "✅",
        "marked done",
    ];
    // Only treat as resolution if it looks like status, not "I'll get it done"
    if hay.contains("i'll") || hay.contains("i will") || hay.contains("i’ll") {
        return false;
    }
    done_patterns.iter().any(|p| hay.contains(p))
}

/// Soft match: open commitment text appears in later message.
pub fn message_resolves_commitment(message: &str, commitment_text: &str) -> bool {
    let hay = message.to_ascii_lowercase();
    if is_resolution_phrase(&hay) {
        // If they mention a distinctive word from the commitment, stronger
        let words: Vec<&str> = commitment_text
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .take(6)
            .collect();
        if words.is_empty() {
            return true; // generic "done" in same thread context — caller scopes by person
        }
        let hits = words
            .iter()
            .filter(|w| hay.contains(&w.to_ascii_lowercase()))
            .count();
        return hits >= 1 || hay.contains("done") || hay.contains("shipped") || hay.contains("sent");
    }
    false
}

fn plain_commitment_summary(raw: &str) -> String {
    let t = raw.trim();
    let mut s = t.to_string();
    // Strip leading @mentions noise lightly
    if s.starts_with('<') {
        if let Some(i) = s.find('>') {
            s = s[i + 1..].trim().to_string();
        }
    }
    if s.chars().count() > PREVIEW_MAX {
        s = s.chars().take(PREVIEW_MAX).collect::<String>() + "…";
    }
    // Capitalize first letter for readability
    let mut c = s.chars();
    match c.next() {
        None => s,
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Try to pull "@name" / "you" style promisee from text (best-effort).
pub fn guess_promisee(text: &str) -> Option<String> {
    // <@U123> slack mention
    if let Some(start) = text.find("<@") {
        if let Some(end) = text[start..].find('>') {
            let id = &text[start + 2..start + end];
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    // "for neel" / "to alice"
    let hay = text.to_ascii_lowercase();
    for prefix in [" for ", " to ", " with "] {
        if let Some(i) = hay.find(prefix) {
            let rest = text[i + prefix.len()..].trim();
            let word = rest
                .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
                .next()
                .unwrap_or("")
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-');
            if word.len() >= 2 && word.len() < 40 {
                return Some(word.to_string());
            }
        }
    }
    None
}

pub fn build_commitment(
    tenant_id: &str,
    promiser: &str,
    promiser_label: Option<String>,
    promisee: Option<String>,
    promisee_label: Option<String>,
    text: &str,
    source: &str,
    channel: Option<&str>,
    evidence: Vec<String>,
    confidence: f32,
) -> Commitment {
    Commitment {
        id: format!("cmt:{}", Uuid::new_v4()),
        tenant_id: tenant_id.to_string(),
        promiser: promiser.to_string(),
        promiser_label,
        promisee,
        promisee_label,
        text: plain_commitment_summary(text),
        status: "open".into(),
        source: source.into(),
        channel: channel.map(|s| s.to_string()),
        evidence,
        confidence,
        created_at: Utc::now().to_rfc3339(),
        resolved_at: None,
        resolve_evidence: None,
        linear_issue_id: None,
        linear_url: None,
    }
}

/// Directory entry for @mention / UI pickers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonDirEntry {
    pub subject_id: String,
    pub display_name: String,
    pub slack_user_id: Option<String>,
    pub aliases: Vec<String>,
}

/// Resolve free text / Slack id / github login against directory.
pub fn resolve_person_ref(raw: &str, dir: &[PersonDirEntry]) -> Option<(String, String)> {
    let r = raw.trim().trim_start_matches('@');
    if r.is_empty() {
        return None;
    }
    let rl = r.to_ascii_lowercase();
    // Exact slack user id
    if let Some(p) = dir.iter().find(|p| {
        p.slack_user_id
            .as_ref()
            .map(|s| s.eq_ignore_ascii_case(r))
            .unwrap_or(false)
    }) {
        return Some((p.subject_id.clone(), p.display_name.clone()));
    }
    // Exact subject / display / alias
    if let Some(p) = dir.iter().find(|p| {
        p.subject_id.eq_ignore_ascii_case(r)
            || p.display_name.eq_ignore_ascii_case(r)
            || p.aliases.iter().any(|a| a.eq_ignore_ascii_case(r))
    }) {
        return Some((p.subject_id.clone(), p.display_name.clone()));
    }
    // Substring display (careful: len >= 3)
    if rl.len() >= 3 {
        if let Some(p) = dir.iter().find(|p| {
            p.display_name.to_ascii_lowercase().contains(&rl)
                || p.aliases
                    .iter()
                    .any(|a| a.to_ascii_lowercase().contains(&rl))
        }) {
            return Some((p.subject_id.clone(), p.display_name.clone()));
        }
    }
    None
}

/// Extract Slack mention user ids from message text (<@U123> or <@U123|name>).
pub fn extract_slack_mention_ids(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<@") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find('>') {
            let inner = &after[..end];
            let uid = inner.split('|').next().unwrap_or(inner).trim();
            if !uid.is_empty() && !out.iter().any(|x| x == uid) {
                out.push(uid.to_string());
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out
}

// ─── Plain English insights (exec-facing) ───────────────────────────────────

/// Turn technical claim types into something a manager can act on.
pub fn claim_type_plain(intent_type: &str) -> &'static str {
    match intent_type.to_ascii_uppercase().as_str() {
        "SHIP" => "trying to ship",
        "BLOCKED" => "stuck / waiting",
        "FIX" => "fixing something",
        "EXPLORE" => "exploring / spiking",
        "REVIEW" => "in review",
        "FREEZE" => "holding back (don't merge yet)",
        _ => "working on something",
    }
}

/// One-line insight from a typed claim.
pub fn claim_insight_line(intent_type: &str, summary: &str, owner: &str, is_demo: bool) -> String {
    if is_demo {
        return format!(
            "(Demo seed — ignore for real decisions) {} — {}",
            owner,
            claim_type_plain(intent_type)
        );
    }
    let body = if summary.is_empty() {
        claim_type_plain(intent_type).to_string()
    } else {
        // Prefer human text over type codes
        let s = summary
            .trim_start_matches("SHIP:")
            .trim_start_matches("BLOCKED:")
            .trim_start_matches("FREEZE:")
            .trim_start_matches("FIX:")
            .trim();
        s.to_string()
    };
    match intent_type.to_ascii_uppercase().as_str() {
        "BLOCKED" => format!("{owner} is blocked: {body}"),
        "FREEZE" => format!("{owner} is holding merge: {body}"),
        "SHIP" => format!("{owner} is aiming to ship: {body}"),
        "FIX" => format!("{owner} is fixing: {body}"),
        _ => format!("{owner}: {body}"),
    }
}

/// Conflict → plain English for champions/execs.
pub fn conflict_insight_line(kind: &str, summary: &str, is_demo: bool) -> String {
    if is_demo {
        return format!("(Demo only) {summary}");
    }
    let k = kind.to_ascii_lowercase();
    if k.contains("ship") && k.contains("freeze") {
        return format!(
            "Mixed signals on the same work: someone wants to ship while someone else wants to hold. Details: {summary}"
        );
    }
    if k.contains("block") {
        return format!("A blocker is sitting open and needs an owner or unblock path. Details: {summary}");
    }
    if k.contains("owner") || k.contains("dual") {
        return format!("More than one person claims ownership — align on who drives it. Details: {summary}");
    }
    if k.contains("merge") || k.contains("friction") {
        return format!("Merge friction on a PR — code or process is stuck. Details: {summary}");
    }
    if k.contains("stale") {
        return format!("Review has gone quiet — someone needs a nudge. Details: {summary}");
    }
    if k.contains("ci") {
        return format!("Ship intent meets failing checks — fix CI or stop claiming ready. Details: {summary}");
    }
    format!("Team friction: {summary}")
}

/// Build an "act on today" style pack (Commit-inspired: few items, not 600).
pub fn build_plain_insights(
    tenant_id: &str,
    claims: &[Value],
    conflicts: &[Value],
    commitments: &[Commitment],
    commit_count: Option<u64>,
    people_with_digests: Option<usize>,
) -> Value {
    let mut act_now: Vec<Value> = Vec::new();
    let mut watch: Vec<Value> = Vec::new();
    let mut wins: Vec<Value> = Vec::new();

    // Open commitments first (highest accountability signal)
    let mut open_cmts: Vec<&Commitment> = commitments
        .iter()
        .filter(|c| c.status == "open")
        .collect();
    open_cmts.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    for c in open_cmts.iter().take(8) {
        act_now.push(json!({
            "kind": "commitment",
            "priority": "high",
            "text": commitment_headline(c),
            "action": format!("Ask {} if this is still open, or mark done when finished.", c.promiser),
            "id": c.id,
        }));
    }

    // Live (non-demo) blocked/freeze claims
    for cl in claims.iter().filter(|c| {
        c.get("is_demo").and_then(|x| x.as_bool()) != Some(true)
            && c.get("lifecycle").and_then(|x| x.as_str()).unwrap_or("open") == "open"
    }) {
        let ty = cl
            .get("intent_type")
            .and_then(|x| x.as_str())
            .unwrap_or("OTHER");
        let owner = cl
            .get("owner_subject")
            .and_then(|x| x.as_str())
            .unwrap_or("Someone");
        let summary = cl
            .get("summary")
            .or_else(|| cl.get("text_preview"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let line = claim_insight_line(ty, summary, owner, false);
        if ty == "BLOCKED" || ty == "FREEZE" {
            act_now.push(json!({
                "kind": "claim",
                "priority": "high",
                "text": line,
                "action": if ty == "BLOCKED" {
                    "Unblock: name who can help, or drop the claim if it's stale."
                } else {
                    "Confirm whether the hold still applies; tell the team."
                },
            }));
        } else if ty == "SHIP" || ty == "FIX" {
            watch.push(json!({
                "kind": "claim",
                "priority": "medium",
                "text": line,
                "action": "Keep an eye on follow-through in the next day or two.",
            }));
        }
    }

    for cf in conflicts.iter().filter(|c| {
        c.get("is_demo").and_then(|x| x.as_bool()) != Some(true)
    }) {
        let kind = cf
            .get("kind")
            .and_then(|x| x.as_str())
            .unwrap_or("conflict");
        let summary = cf
            .get("summary")
            .and_then(|x| x.as_str())
            .unwrap_or("team conflict");
        act_now.push(json!({
            "kind": "conflict",
            "priority": "high",
            "text": conflict_insight_line(kind, summary, false),
            "action": "Get the owners in one thread and pick ship vs hold (or split ownership).",
        }));
    }

    // Trajectory wins (plain English)
    if let Some(n) = commit_count {
        if n > 0 {
            wins.push(json!({
                "kind": "trajectory",
                "priority": "info",
                "text": format!("The team has logged about {n} commits on the work graph recently — motion is real even when status is quiet."),
                "action": "Use digests to turn that motion into a short story for stakeholders.",
            }));
        }
    }
    if let Some(p) = people_with_digests {
        if p >= 2 {
            wins.push(json!({
                "kind": "status",
                "priority": "info",
                "text": format!("{p} people already have real status digests — multi-person status is working."),
                "action": "Champion: scan digests once, not a standup round-robin.",
            }));
        }
    }

    // Cap "act on today" like Commit (3–8 items)
    act_now.truncate(8);
    watch.truncate(6);
    wins.truncate(4);

    let headline = if !act_now.is_empty() {
        format!(
            "{} thing{} need attention today.",
            act_now.len(),
            if act_now.len() == 1 { "" } else { "s" }
        )
    } else if !watch.is_empty() {
        "Nothing urgent — a few items worth a light check.".to_string()
    } else {
        "Quiet board: no open commitments or live blockers in the ledger yet. Capture promises from Slack or the Capture button.".to_string()
    };

    json!({
        "tenant_id": tenant_id,
        "as_of": Utc::now().to_rfc3339(),
        "headline": headline,
        "act_on_today": act_now,
        "worth_watching": watch,
        "good_news": wins,
        "how_we_read_signals": {
            "simple": "We watch work (GitHub) and promises (Slack/bot). Technical labels stay under the hood; you see plain English.",
            "github": "Commits and PRs show motion — not who is 'best'.",
            "claims": "Words like blocked/ship/freeze become 'stuck' / 'trying to ship' / 'holding merge'.",
            "commitments": "Phrases like 'I'll send…' become open loops: who owes what. We mark them done when someone says shipped/sent/done (or you mark done).",
            "not": "We do not rank people by lines of code. We do not read private 1:1 DMs unless the bot is there and you chose that surface.",
        },
        "vs_tickets": {
            "jira_linear": "Jira/Linear create tickets when you file them. They do not ambiently track 'I'll do X by Friday' from chat and close it when chat says done.",
            "us": "Commitments are lightweight promises for teams that live in Slack — optional, not a full ticketing suite.",
        },
        "inspired_by": ["Commit (promises in chat)", "Minimi (open loops)"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_ill() {
        let r = extract_commitment("I'll send the deck by Friday");
        assert!(r.is_some());
        let (t, c, _) = r.unwrap();
        assert!(c > 0.8);
        assert!(t.to_ascii_lowercase().contains("deck") || t.to_ascii_lowercase().contains("send"));
    }

    #[test]
    fn slack_mention_ids() {
        let ids = extract_slack_mention_ids("hey <@U0APK7W1X99|neel> I'll review this");
        assert_eq!(ids, vec!["U0APK7W1X99".to_string()]);
    }

    #[test]
    fn resolve_person_dir() {
        let dir = vec![PersonDirEntry {
            subject_id: "gu_1".into(),
            display_name: "neeljoshi18".into(),
            slack_user_id: Some("U123".into()),
            aliases: vec!["neel".into()],
        }];
        let r = resolve_person_ref("U123", &dir).unwrap();
        assert_eq!(r.0, "gu_1");
        assert_eq!(r.1, "neeljoshi18");
    }

    #[test]
    fn morning_digest_empty() {
        let t = format_morning_digest("ten_github", &[]);
        assert!(t.contains("no open"));
    }

    #[test]
    fn skips_resolution_as_commitment() {
        assert!(extract_commitment("Done — shipped the deck").is_none());
    }

    #[test]
    fn resolution_detect() {
        assert!(is_resolution_phrase("just shipped the export"));
        assert!(!is_resolution_phrase("i'll ship tomorrow"));
    }

    #[test]
    fn plain_blocked() {
        let s = claim_insight_line("BLOCKED", "waiting on security", "Neel", false);
        assert!(s.contains("blocked"));
        assert!(s.contains("Neel"));
        assert!(!s.contains("BLOCKED:"));
    }
}
