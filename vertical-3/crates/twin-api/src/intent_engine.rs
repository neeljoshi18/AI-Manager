//! # In-house Intent Engine
//!
//! Accumulates the principles from `plans/intent-research.md`:
//! 1. Closed ontology (not free-text as truth)
//! 2. Evidence or it didn't happen
//! 3. Ambient capture only on *chosen* surfaces (not private 1:1)
//! 4. Trajectory ≠ claim (facts separate from purpose)
//! 5. Conflicts first (exec-legible collisions)
//! 6. Rules before LLM (no outsourced mind-read brain)
//! 7. Human gate (propose; humans ratify / supersede)
//!
//! Intent = typed claim about purpose + owner + optional work + evidence + confidence.
//! Not: mood, LOC rankings, chat archive, buyer-intent scores.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Product law — returned by GET /intent/engine so the UI and partners see the contract.
pub const PRINCIPLES: &[(&str, &str)] = &[
    (
        "closed_ontology",
        "Only SHIP|BLOCKED|FIX|EXPLORE|REVIEW|FREEZE|OTHER — not free-text goals as truth",
    ),
    (
        "evidence_required",
        "Every claim carries evidence[] and source; no inventing work items",
    ),
    (
        "chosen_surfaces",
        "GitHub + tickets + team channels (bot member) + bot DM — never silent 1:1 wiretap",
    ),
    (
        "trajectory_vs_claim",
        "Commits/PRs are facts; intent is a purpose claim with confidence",
    ),
    (
        "conflicts_first",
        "Claim–claim and claim–fact collisions are the primary exec surface",
    ),
    (
        "rules_before_llm",
        "In-house rules classifiers first; no outsourced 'what does Alice want' LLM brain",
    ),
    (
        "human_gate",
        "Engine proposes; digests Approve/Edit/Don't send; claims can be superseded",
    ),
];

pub const EXPLICIT_CLAIMS_KV: &str = "intent_explicit_claims";
pub const EXPLICIT_CLAIMS_MAX: usize = 200;
pub const TEXT_PREVIEW_MAX: usize = 280;

/// Closed ontology (Oliv-style fixed question vocabulary for eng).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntentType {
    Ship,
    Blocked,
    Fix,
    Explore,
    Review,
    Freeze,
    Other,
}

impl IntentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ship => "SHIP",
            Self::Blocked => "BLOCKED",
            Self::Fix => "FIX",
            Self::Explore => "EXPLORE",
            Self::Review => "REVIEW",
            Self::Freeze => "FREEZE",
            Self::Other => "OTHER",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "SHIP" | "SHIPPING" | "RELEASE" => Some(Self::Ship),
            "BLOCKED" | "BLOCKER" | "BLOCKING" => Some(Self::Blocked),
            "FIX" | "BUGFIX" | "HOTFIX" => Some(Self::Fix),
            "EXPLORE" | "SPIKE" | "POC" | "RESEARCH" => Some(Self::Explore),
            "REVIEW" | "RFC" => Some(Self::Review),
            "FREEZE" | "HOLD" | "PAUSE" => Some(Self::Freeze),
            "OTHER" => Some(Self::Other),
            _ => None,
        }
    }
}

/// Where a claim came from (provenance — Glean-style source discipline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSource {
    GithubPr,
    GithubIssue,
    SlackChannel,
    SlackDm,
    Explicit,
    Seed,
    DigestEdit,
    Unknown,
}

impl ClaimSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GithubPr => "github_pr",
            Self::GithubIssue => "github_issue",
            Self::SlackChannel => "slack_channel",
            Self::SlackDm => "slack_dm",
            Self::Explicit => "explicit",
            Self::Seed => "seed",
            Self::DigestEdit => "digest_edit",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "github_pr" | "github" => Self::GithubPr,
            "github_issue" => Self::GithubIssue,
            "slack_channel" | "channel" => Self::SlackChannel,
            "slack_dm" | "dm" | "im" => Self::SlackDm,
            "explicit" | "bot" | "slash" | "champion" => Self::Explicit,
            "seed" | "intent_demo" | "graph_story" => Self::Seed,
            "digest_edit" => Self::DigestEdit,
            _ => Self::Unknown,
        }
    }
}

/// Lifecycle of a claim (SpecStory durability without chat-as-product).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimLifecycle {
    Open,
    Superseded,
    Resolved,
}

/// Unified claim row for the ledger (sparse high-precision graph, not activity dump).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentClaimRecord {
    pub claim_id: String,
    pub tenant_id: String,
    pub intent_type: String,
    pub summary: String,
    pub owner_subject: Option<String>,
    pub about_node_id: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub source: String,
    pub is_demo: bool,
    pub lifecycle: String,
    pub at: String,
    pub text_preview: Option<String>,
    pub channel: Option<String>,
}

impl IntentClaimRecord {
    pub fn to_json(&self) -> Value {
        json!({
            "claim_id": self.claim_id,
            "tenant_id": self.tenant_id,
            "intent_type": self.intent_type,
            "summary": self.summary,
            "owner_subject": self.owner_subject,
            "about_node_id": self.about_node_id,
            "confidence": self.confidence,
            "evidence": self.evidence,
            "source": self.source,
            "is_demo": self.is_demo,
            "lifecycle": self.lifecycle,
            "at": self.at,
            "text_preview": self.text_preview,
            "channel": self.channel,
        })
    }
}

/// Rules-first classifier (in-house; mirrors graph-core patterns — no LLM invent).
/// Returns (type string, confidence, evidence tag).
pub fn classify_text(text: &str) -> (String, f32, String) {
    let hay = text.to_ascii_lowercase();
    let patterns: &[(&str, IntentType, f32)] = &[
        ("blocked by", IntentType::Blocked, 0.9),
        ("blocked on", IntentType::Blocked, 0.9),
        ("waiting on", IntentType::Blocked, 0.8),
        ("blocked:", IntentType::Blocked, 0.85),
        ("[blocked]", IntentType::Blocked, 0.9),
        ("i'm blocked", IntentType::Blocked, 0.9),
        ("im blocked", IntentType::Blocked, 0.9),
        ("code freeze", IntentType::Freeze, 0.9),
        ("do not merge", IntentType::Freeze, 0.9),
        ("don't merge", IntentType::Freeze, 0.9),
        ("donotmerge", IntentType::Freeze, 0.9),
        ("hold merge", IntentType::Freeze, 0.8),
        ("freeze ", IntentType::Freeze, 0.75),
        ("freeze:", IntentType::Freeze, 0.8),
        ("hotfix", IntentType::Fix, 0.85),
        ("bugfix", IntentType::Fix, 0.85),
        ("fix:", IntentType::Fix, 0.8),
        ("fix ", IntentType::Fix, 0.7),
        ("working on", IntentType::Ship, 0.72),
        ("ready to ship", IntentType::Ship, 0.9),
        ("ship ", IntentType::Ship, 0.75),
        ("shipping ", IntentType::Ship, 0.75),
        ("release ", IntentType::Ship, 0.75),
        ("deploy ", IntentType::Ship, 0.7),
        ("launch ", IntentType::Ship, 0.7),
        ("merge to main", IntentType::Ship, 0.75),
        ("feat:", IntentType::Ship, 0.65),
        ("feature:", IntentType::Ship, 0.65),
        ("spike:", IntentType::Explore, 0.85),
        ("spike ", IntentType::Explore, 0.8),
        ("poc:", IntentType::Explore, 0.8),
        ("explore ", IntentType::Explore, 0.7),
        ("research ", IntentType::Explore, 0.7),
        ("rfc:", IntentType::Review, 0.85),
        ("wip:", IntentType::Explore, 0.6),
        ("[wip]", IntentType::Explore, 0.65),
    ];
    for (pat, ty, conf) in patterns {
        if hay.contains(pat) {
            return (ty.as_str().to_string(), *conf, format!("text:{pat}"));
        }
    }
    (IntentType::Other.as_str().to_string(), 0.25, "default:other".into())
}

pub fn truncate_preview(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max {
        return t.to_string();
    }
    t.chars().take(max).collect::<String>() + "…"
}

pub fn looks_like_demo(blob: &str) -> bool {
    let b = blob.to_ascii_lowercase();
    b.contains("gu_demo_")
        || b.contains("intent_demo")
        || b.contains("graph_story")
        || b.contains("/pr/story-1")
        || b.contains("\"is_demo\":true")
        || b.contains("\"seed\":\"")
}

/// Build engine status JSON (principles + doctrine).
pub fn engine_manifest() -> Value {
    let principles: Vec<Value> = PRINCIPLES
        .iter()
        .map(|(id, text)| json!({ "id": id, "text": text }))
        .collect();
    json!({
        "name": "ai_manager_intent_engine",
        "version": "v0",
        "in_house": true,
        "outsourced_brain": false,
        "ontology": ["SHIP", "BLOCKED", "FIX", "EXPLORE", "REVIEW", "FREEZE", "OTHER"],
        "principles": principles,
        "layers": {
            "L0_facts": "graph commits/PRs/CI trajectory",
            "L1_claims": "rules extractors + explicit capture",
            "L2_conflicts": "claim–claim and claim–fact (pulse / V2)",
            "L3_follow_through": "claim vs later facts",
            "L4_surfaces": "ledger · profile · cockpit · digests"
        },
        "surfaces": {
            "ambient": ["github", "tickets", "slack_channels_bot_member", "bot_dm"],
            "not_ambient": ["private_1_1_dm_wiretap", "full_doc_crawl", "buyer_intent_vendors"]
        },
        "doctrine": {
            "chat": "delivery",
            "github": "work",
            "not": ["LOC rankings", "silent 1:1 wiretap", "Glean-class archive as product"]
        },
        "research": "plans/intent-research.md",
        "design": "plans/2026-08-07_intent-engine-design.md",
    })
}

/// Normalize a graph intent node / API blob into a ledger claim.
pub fn claim_from_graph_intent(tenant_id: &str, intent: &Value) -> IntentClaimRecord {
    let blob = intent.to_string();
    let is_demo = intent
        .get("is_demo")
        .and_then(|x| x.as_bool())
        .unwrap_or_else(|| looks_like_demo(&blob));
    let props = intent.get("properties").cloned().unwrap_or(json!({}));
    let intent_type = intent
        .get("intent_type")
        .or_else(|| props.get("intent_type"))
        .and_then(|x| x.as_str())
        .unwrap_or("OTHER")
        .to_string();
    let summary = intent
        .get("display_name")
        .or_else(|| intent.get("label"))
        .or_else(|| intent.get("summary"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let claim_id = intent
        .get("id")
        .or_else(|| intent.get("node_id"))
        .or_else(|| intent.get("claim_id"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let source = intent
        .get("source")
        .or_else(|| props.get("source"))
        .and_then(|x| x.as_str())
        .map(|s| ClaimSource::parse(s).as_str().to_string())
        .unwrap_or_else(|| {
            if is_demo {
                ClaimSource::Seed.as_str().into()
            } else {
                ClaimSource::Unknown.as_str().into()
            }
        });
    let confidence = intent
        .get("confidence")
        .or_else(|| props.get("confidence"))
        .and_then(|x| x.as_f64())
        .unwrap_or(0.5) as f32;
    let evidence: Vec<String> = intent
        .get("evidence")
        .or_else(|| props.get("evidence"))
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let owner = intent
        .get("owner_node_id")
        .or_else(|| props.get("owner_node_id"))
        .or_else(|| intent.get("owner_subject"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let about = intent
        .get("about_node_id")
        .or_else(|| props.get("about_node_id"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let at = intent
        .get("at")
        .or_else(|| intent.get("updated_at"))
        .or_else(|| intent.get("created_at"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    IntentClaimRecord {
        claim_id: if claim_id.is_empty() {
            format!("graph:{}", Uuid::new_v4())
        } else {
            claim_id
        },
        tenant_id: tenant_id.to_string(),
        intent_type,
        summary,
        owner_subject: owner,
        about_node_id: about,
        confidence,
        evidence,
        source,
        is_demo,
        lifecycle: "open".into(),
        at: if at.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            at
        },
        text_preview: None,
        channel: None,
    }
}

/// Normalize slack_intent_claims KV object into ledger claim.
pub fn claim_from_slack_kv(tenant_id: &str, c: &Value) -> IntentClaimRecord {
    let blob = c.to_string();
    let is_demo = c
        .get("is_demo")
        .and_then(|x| x.as_bool())
        .unwrap_or_else(|| looks_like_demo(&blob));
    let channel = c.get("channel").and_then(|x| x.as_str()).unwrap_or("");
    let source = if channel == "dm" || channel.starts_with('D') {
        ClaimSource::SlackDm
    } else {
        ClaimSource::SlackChannel
    };
    let preview = c
        .get("text_preview")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let itype = c
        .get("intent_type")
        .and_then(|x| x.as_str())
        .unwrap_or("OTHER")
        .to_string();
    let conf = c
        .get("confidence")
        .and_then(|x| x.as_f64())
        .unwrap_or(0.5) as f32;
    let at = c
        .get("at")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let claim_id = c
        .get("claim_id")
        .or_else(|| c.get("ts"))
        .and_then(|x| x.as_str())
        .map(|s| format!("slack:{s}"))
        .unwrap_or_else(|| format!("slack:{}", Uuid::new_v4()));
    IntentClaimRecord {
        claim_id,
        tenant_id: tenant_id.to_string(),
        intent_type: itype.clone(),
        summary: if preview.is_empty() {
            format!("{itype} (slack)")
        } else {
            format!("{itype}: {preview}")
        },
        owner_subject: c
            .get("subject")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string()),
        about_node_id: None,
        confidence: conf,
        evidence: vec![
            format!("slack_user:{}", c.get("slack_user").and_then(|x| x.as_str()).unwrap_or("")),
            format!("channel:{channel}"),
            format!("ts:{}", c.get("ts").and_then(|x| x.as_str()).unwrap_or("")),
        ]
        .into_iter()
        .filter(|e| !e.ends_with(':'))
        .collect(),
        source: source.as_str().into(),
        is_demo,
        lifecycle: c
            .get("lifecycle")
            .and_then(|x| x.as_str())
            .unwrap_or("open")
            .to_string(),
        at: if at.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            at
        },
        text_preview: Some(truncate_preview(&preview, TEXT_PREVIEW_MAX)),
        channel: Some(channel.to_string()),
    }
}

/// Build explicit claim from POST body (human / bot / slash — highest trust).
pub fn build_explicit_claim(
    tenant_id: &str,
    intent_type: &str,
    summary: &str,
    owner_subject: Option<&str>,
    about_node_id: Option<&str>,
    evidence: Vec<String>,
    channel: Option<&str>,
) -> IntentClaimRecord {
    let itype = IntentType::parse(intent_type)
        .unwrap_or(IntentType::Other)
        .as_str()
        .to_string();
    let mut ev = evidence;
    if ev.is_empty() {
        ev.push("source:explicit".into());
    }
    let claim_id = format!("explicit:{}", Uuid::new_v4());
    let sum = summary.trim();
    IntentClaimRecord {
        claim_id,
        tenant_id: tenant_id.to_string(),
        intent_type: itype.clone(),
        summary: if sum.is_empty() {
            format!("{itype} (explicit)")
        } else {
            truncate_preview(sum, TEXT_PREVIEW_MAX)
        },
        owner_subject: owner_subject.map(|s| s.to_string()),
        about_node_id: about_node_id.map(|s| s.to_string()),
        confidence: 0.95, // explicit human/bot statement — high trust
        evidence: ev,
        source: ClaimSource::Explicit.as_str().into(),
        is_demo: false,
        lifecycle: ClaimLifecycle::Open.as_api().into(),
        at: Utc::now().to_rfc3339(),
        text_preview: Some(truncate_preview(sum, TEXT_PREVIEW_MAX)),
        channel: channel.map(|s| s.to_string()),
    }
}

impl ClaimLifecycle {
    fn as_api(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Superseded => "superseded",
            Self::Resolved => "resolved",
        }
    }
}

/// Merge graph + slack + explicit into one ledger; optional filters.
pub fn merge_ledger(
    graph: Vec<IntentClaimRecord>,
    slack: Vec<IntentClaimRecord>,
    explicit: Vec<IntentClaimRecord>,
    include_demo: bool,
    open_only: bool,
) -> Vec<IntentClaimRecord> {
    let mut all = Vec::new();
    all.extend(graph);
    all.extend(slack);
    all.extend(explicit);
    all.retain(|c| {
        if !include_demo && c.is_demo {
            return false;
        }
        if open_only && c.lifecycle != "open" {
            return false;
        }
        true
    });
    // Newest first
    all.sort_by(|a, b| b.at.cmp(&a.at));
    all
}

/// Counts for engine status / cockpit.
pub fn ledger_stats(claims: &[IntentClaimRecord]) -> Value {
    let mut by_type: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut by_source: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut demo = 0u64;
    let mut live = 0u64;
    let mut open = 0u64;
    for c in claims {
        *by_type.entry(c.intent_type.clone()).or_insert(0) += 1;
        *by_source.entry(c.source.clone()).or_insert(0) += 1;
        if c.is_demo {
            demo += 1;
        } else {
            live += 1;
        }
        if c.lifecycle == "open" {
            open += 1;
        }
    }
    json!({
        "total": claims.len(),
        "live": live,
        "demo": demo,
        "open": open,
        "by_type": by_type,
        "by_source": by_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_blocked() {
        let (t, c, _) = classify_text("I'm blocked on security review");
        assert_eq!(t, "BLOCKED");
        assert!(c >= 0.8);
    }

    #[test]
    fn classify_freeze() {
        let (t, _, _) = classify_text("do not merge until demo");
        assert_eq!(t, "FREEZE");
    }

    #[test]
    fn explicit_high_confidence() {
        let c = build_explicit_claim(
            "ten_github",
            "BLOCKED",
            "blocked on partner review",
            Some("neeljoshi18"),
            None,
            vec![],
            Some("dm"),
        );
        assert!(!c.is_demo);
        assert!((c.confidence - 0.95).abs() < 0.01);
        assert_eq!(c.source, "explicit");
    }

    #[test]
    fn merge_filters_demo() {
        let mut demo = build_explicit_claim("t", "SHIP", "x", None, None, vec![], None);
        demo.is_demo = true;
        demo.source = "seed".into();
        let live = build_explicit_claim("t", "FIX", "hotfix auth", None, None, vec![], None);
        let m = merge_ledger(vec![demo], vec![], vec![live], false, true);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].intent_type, "FIX");
    }
}
