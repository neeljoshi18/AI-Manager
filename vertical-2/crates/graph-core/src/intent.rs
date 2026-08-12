//! Intent classification v0 + conflict detection (rules only — no LLM).
//!
//! Pipeline (roadmap): extract → type → attach to graph → conflict detect → surface.
//! Facts = webhook/work items; intents = claims about purpose (SHIP / BLOCKED / …).
//!
//! Provenance on every Intent node: `classified_by`, `source`, `is_demo`, confidence, evidence.

use crate::ids::{person_node_id, stable_edge_id};
use crate::model::{GraphEdge, GraphMutation, GraphNode};
use crate::v1_event::V1CanonicalEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Deterministic intent types for v0 rules (ADR-016: rules before local model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl std::fmt::Display for IntentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured intent claim extracted from a work item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentClaim {
    pub intent_type: IntentType,
    pub summary: String,
    /// Work item node this intent is about (pr:… / issue:…).
    pub about_node_id: String,
    /// Person node who claims / owns the intent when known.
    pub owner_node_id: Option<String>,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

/// Conflict card for UI / batched Slack (human-gated).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictCard {
    pub conflict_id: String,
    pub kind: ConflictKind,
    pub summary: String,
    pub severity: ConflictSeverity,
    pub node_ids: Vec<String>,
    pub owner_node_ids: Vec<String>,
    pub intent_types: Vec<String>,
    pub evidence: Vec<String>,
    /// True iff **all** involved intent nodes are demo/seed. Mixed → false (evidence may note seed).
    #[serde(default)]
    pub is_demo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Two BLOCKS edges / mutual blockers on related work.
    DualBlocks,
    /// SHIP vs FREEZE on same resource or shared target.
    ShipVsFreeze,
    /// Same work item has multiple person owners with competing intents.
    DualOwners,
    /// Explicit BLOCKED intent without resolving owner.
    OpenBlocker,
    /// PR properties suggest conflicted / unstable merge.
    MergeFriction,
    /// Open PR with no update / review stall signals.
    StaleReview,
    /// CI / checks failure while intent is SHIP.
    CiBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSeverity {
    High,
    Medium,
    Low,
}

/// Stable intent node id: one claim per (owner, work item) so dual-owner conflicts surface.
pub fn intent_node_id(owner_node_id: &str, about_node_id: &str) -> String {
    format!("intent:{owner_node_id}:{about_node_id}")
}

/// Whether a graph node is demo/seed theater (not organic pilot signal).
pub fn node_is_demo(n: &GraphNode) -> bool {
    if n.properties
        .get("is_demo")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return true;
    }
    if let Some(s) = n.properties.get("seed").and_then(|v| v.as_str()) {
        if !s.is_empty() {
            return true;
        }
    }
    let id = n.node_id.to_ascii_lowercase();
    let resource = n.resource_id.to_ascii_lowercase();
    id.contains("gu_demo_")
        || id.contains("demo-repo")
        || resource.contains("demo-repo")
}

/// True when every involved intent is demo; empty intent set → false (not theater-only).
fn intents_all_demo(intent_nodes: &[&GraphNode]) -> bool {
    !intent_nodes.is_empty() && intent_nodes.iter().all(|n| node_is_demo(n))
}

/// Annotate evidence when a mixed organic+seed card is produced.
fn note_mixed_seed(evidence: &mut Vec<String>, intent_nodes: &[&GraphNode]) {
    let any_demo = intent_nodes.iter().any(|n| node_is_demo(n));
    let all_demo = intents_all_demo(intent_nodes);
    if any_demo && !all_demo {
        evidence.push("includes_seed_intent".into());
    }
}

/// Infer provenance `source` for organic attach (extensible string).
pub fn infer_intent_source(event: &V1CanonicalEvent, about: &GraphNode) -> String {
    let nt = about.node_type.to_ascii_lowercase();
    let et = event.event_type.to_ascii_lowercase();
    let provider = event.provider.to_ascii_lowercase();

    if nt.contains("pull")
        || nt.contains("merge_request")
        || et.contains("pull_request")
        || et.contains("merge_request")
    {
        if provider.contains("gitlab") {
            return "gitlab_mr".into();
        }
        // github_pr for surface; intent engine also accepts github_rules alias
        return "github_pr".into();
    }
    if nt.contains("issue") || et.contains("issue") {
        if provider.contains("jira") {
            return "jira_issue".into();
        }
        if provider.contains("linear") {
            return "linear_issue".into();
        }
        return "github_issue".into();
    }
    "work_item".into()
}

/// Collect free-text + labels from event attributes for classification.
pub fn extract_text_from_event(event: &V1CanonicalEvent) -> (String, Vec<String>, String) {
    let title = event
        .attributes
        .get("title")
        .or_else(|| event.attributes.get("summary"))
        .or_else(|| event.attributes.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let body = event
        .attributes
        .get("body")
        .or_else(|| event.attributes.get("description"))
        .or_else(|| event.attributes.get("body_preview"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut labels = Vec::new();
    if let Some(arr) = event.attributes.get("labels").and_then(|v| v.as_array()) {
        for l in arr {
            if let Some(s) = l.as_str() {
                labels.push(s.to_string());
            } else if let Some(name) = l.get("name").and_then(|v| v.as_str()) {
                labels.push(name.to_string());
            }
        }
    }
    if let Some(s) = event.attributes.get("label").and_then(|v| v.as_str()) {
        labels.push(s.to_string());
    }
    // GitHub draft / state hints
    if event
        .attributes
        .get("draft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        labels.push("draft".into());
    }
    (title, labels, body)
}

/// True when title is empty or non-informative noise (should not inflate SHIP confidence).
fn title_is_weak_noise(title: &str) -> bool {
    let t = title.trim();
    if t.is_empty() || t.len() <= 3 {
        return true;
    }
    // No alphanumeric content (punctuation-only / emoji noise)
    if !t.chars().any(|c| c.is_alphanumeric()) {
        return true;
    }
    // Generic placeholders
    let lower = t.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "wip" | "tmp" | "temp" | "test" | "asdf" | "foo" | "bar" | "pr" | "update" | "..."
    )
}

/// Rules-only intent classifier (structure-first; no LLM invent).
pub fn classify_intent(title: &str, labels: &[String], body: &str) -> (IntentType, f32, Vec<String>) {
    let mut evidence = Vec::new();
    let hay = format!(
        "{} {} {}",
        title.to_ascii_lowercase(),
        labels
            .iter()
            .map(|l| l.to_ascii_lowercase())
            .collect::<Vec<_>>()
            .join(" "),
        body.to_ascii_lowercase()
    );
    let label_l: Vec<String> = labels.iter().map(|l| l.to_ascii_lowercase()).collect();

    // Label wins when explicit
    for l in &label_l {
        if let Some(t) = IntentType::parse(l) {
            if t != IntentType::Other {
                evidence.push(format!("label:{l}"));
                return (t, 0.95, evidence);
            }
        }
        if l.contains("blocked") || l == "blocker" || l == "wip-blocked" {
            evidence.push(format!("label:{l}"));
            return (IntentType::Blocked, 0.95, evidence);
        }
        if l.contains("freeze") || l == "do-not-merge" || l == "hold" {
            evidence.push(format!("label:{l}"));
            return (IntentType::Freeze, 0.9, evidence);
        }
        if l == "bug" || l == "fix" || l == "hotfix" {
            evidence.push(format!("label:{l}"));
            return (IntentType::Fix, 0.9, evidence);
        }
        if l == "spike" || l == "exploration" || l == "rfc" {
            evidence.push(format!("label:{l}"));
            return (
                if l == "rfc" {
                    IntentType::Review
                } else {
                    IntentType::Explore
                },
                0.85,
                evidence,
            );
        }
    }

    // Draft PRs are not ship-ready: prefer FREEZE (or EXPLORE for pure WIP label).
    // "draft" is injected from attributes.draft by extract_text_from_event.
    if label_l.iter().any(|l| l == "draft") {
        evidence.push("label:draft".into());
        return (IntentType::Freeze, 0.8, evidence);
    }
    if label_l.iter().any(|l| l == "wip") {
        evidence.push("label:wip".into());
        return (IntentType::Explore, 0.7, evidence);
    }

    // Title / body verb patterns (order matters: blocked/freeze before ship)
    let patterns: &[(&str, IntentType, f32)] = &[
        ("blocked by", IntentType::Blocked, 0.9),
        ("blocked on", IntentType::Blocked, 0.9),
        ("waiting on", IntentType::Blocked, 0.8),
        ("blocked:", IntentType::Blocked, 0.85),
        ("[blocked]", IntentType::Blocked, 0.9),
        ("code freeze", IntentType::Freeze, 0.9),
        ("do not merge", IntentType::Freeze, 0.9),
        ("donotmerge", IntentType::Freeze, 0.9),
        ("hold merge", IntentType::Freeze, 0.8),
        ("freeze ", IntentType::Freeze, 0.75),
        ("hotfix", IntentType::Fix, 0.85),
        ("bugfix", IntentType::Fix, 0.85),
        ("fix:", IntentType::Fix, 0.8),
        ("fix(", IntentType::Fix, 0.75),
        ("fix ", IntentType::Fix, 0.7),
        ("spike:", IntentType::Explore, 0.85),
        ("spike ", IntentType::Explore, 0.8),
        ("poc:", IntentType::Explore, 0.8),
        ("explore ", IntentType::Explore, 0.7),
        ("research ", IntentType::Explore, 0.7),
        ("rfc:", IntentType::Review, 0.85),
        ("wip:", IntentType::Explore, 0.6),
        ("[wip]", IntentType::Explore, 0.65),
        ("ready to ship", IntentType::Ship, 0.9),
        ("ship ", IntentType::Ship, 0.75),
        ("release ", IntentType::Ship, 0.75),
        ("deploy ", IntentType::Ship, 0.7),
        ("launch ", IntentType::Ship, 0.7),
        ("merge to main", IntentType::Ship, 0.75),
        ("feat:", IntentType::Ship, 0.65),
        ("feature:", IntentType::Ship, 0.65),
        (" impl ", IntentType::Ship, 0.55),
    ];

    for (pat, ty, conf) in patterns {
        if hay.contains(pat) {
            evidence.push(format!("text:{pat}"));
            return (*ty, *conf, evidence);
        }
    }

    // Weak default: open work without strong signal.
    // Empty / noise titles must NOT get a confident SHIP — use ≤0.45 + default:open_pr_weak.
    if title_is_weak_noise(title) {
        if hay.contains("pull_request") || hay.contains("pullrequest") {
            evidence.push("default:open_pr_weak".into());
            return (IntentType::Ship, 0.35, evidence);
        }
        evidence.push("default:other".into());
        return (IntentType::Other, 0.25, evidence);
    }

    // Non-empty title on a work item without verb signal → mild SHIP (eng digest default)
    if hay.contains("pull_request") || title.trim().len() > 3 {
        evidence.push("default:work_item".into());
        return (IntentType::Ship, 0.4, evidence);
    }

    evidence.push("default:other".into());
    (IntentType::Other, 0.3, evidence)
}

/// Build Intent node + CLAIMS (person→intent) + ABOUT (intent→work) edges.
///
/// Organic provenance always set: `classified_by=rules_v0`, `source`, `is_demo=false`.
pub fn attach_intent_mutation(
    event: &V1CanonicalEvent,
    about_node: &GraphNode,
    owner_person_node_id: &str,
    title: &str,
    labels: &[String],
    body: &str,
) -> GraphMutation {
    // Commits are trajectory facts, not purpose claims — never attach intent.
    if about_node.node_type.eq_ignore_ascii_case("Commit") {
        return GraphMutation::default();
    }

    let (itype, conf, evidence) = classify_intent(title, labels, body);
    let (is_private, groups, ver) = (
        event.acl.is_private,
        event.acl.allowed_group_ids.clone(),
        event.acl.acl_version,
    );
    let iid = intent_node_id(owner_person_node_id, &about_node.node_id);
    let summary = if title.is_empty() {
        format!("{} on {}", itype, about_node.resource_id)
    } else {
        format!("{}: {}", itype, title)
    };
    let source = infer_intent_source(event, about_node);
    let intent = GraphNode {
        tenant_id: event.tenant_id.clone(),
        node_id: iid.clone(),
        node_type: "Intent".into(),
        display_name: summary.clone(),
        resource_id: about_node.resource_id.clone(),
        properties: json!({
            "intent_type": itype.as_str(),
            "confidence": conf,
            "evidence": evidence,
            "about_node_id": about_node.node_id,
            "owner_node_id": owner_person_node_id,
            "classified_by": "rules_v0",
            "source": source,
            "is_demo": false,
            "event_id": event.event_id,
        }),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };

    let claims = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: stable_edge_id(
            &event.tenant_id,
            "CLAIMS",
            owner_person_node_id,
            &iid,
        ),
        edge_type: "CLAIMS".into(),
        from_node_id: owner_person_node_id.to_string(),
        to_node_id: iid.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({ "intent_type": itype.as_str() }),
        is_private,
        allowed_group_ids: groups.clone(),
        acl_version: ver,
    };

    let about = GraphEdge {
        tenant_id: event.tenant_id.clone(),
        edge_id: stable_edge_id(&event.tenant_id, "ABOUT", &iid, &about_node.node_id),
        edge_type: "ABOUT".into(),
        from_node_id: iid,
        to_node_id: about_node.node_id.clone(),
        valid_from: event.event_timestamp,
        valid_to: None,
        event_id: event.event_id.clone(),
        properties: json!({}),
        is_private,
        allowed_group_ids: groups,
        acl_version: ver,
    };

    GraphMutation {
        nodes: vec![intent],
        edges: vec![claims, about],
        states: vec![],
    }
}

/// Merge intent mutation into an existing PR/issue mutation.
/// Skips Commit nodes (commit messages are not intents).
pub fn merge_intent_into(
    mut base: GraphMutation,
    event: &V1CanonicalEvent,
    about_node_id: &str,
) -> GraphMutation {
    let about = match base.nodes.iter().find(|n| n.node_id == about_node_id) {
        Some(n) => n.clone(),
        None => return base,
    };
    // Trajectory only — never claim intent on commits.
    if about.node_type.eq_ignore_ascii_case("Commit") {
        return base;
    }
    let owner = base
        .nodes
        .iter()
        .find(|n| n.node_type == "Person")
        .map(|n| n.node_id.clone())
        .unwrap_or_else(|| {
            if event.actor.global_user_id.is_empty() {
                format!("person:prov:{}", event.actor.provider_user_id)
            } else {
                person_node_id(&event.actor.global_user_id)
            }
        });
    let (title, labels, body) = extract_text_from_event(event);
    let intent_m = attach_intent_mutation(event, &about, &owner, &title, &labels, &body);
    base.nodes.extend(intent_m.nodes);
    base.edges.extend(intent_m.edges);
    base
}

// ─── Conflict helpers (organic friction from projected node properties) ─────

fn work_labels(work: &GraphNode) -> Vec<String> {
    let mut labels = Vec::new();
    if let Some(arr) = work.properties.get("labels").and_then(|v| v.as_array()) {
        for l in arr {
            if let Some(s) = l.as_str() {
                labels.push(s.to_ascii_lowercase());
            } else if let Some(n) = l.get("name").and_then(|v| v.as_str()) {
                labels.push(n.to_ascii_lowercase());
            }
        }
    }
    if let Some(s) = work.properties.get("label").and_then(|v| v.as_str()) {
        labels.push(s.to_ascii_lowercase());
    }
    labels
}

fn work_text_hay(work: &GraphNode) -> String {
    let title = work
        .properties
        .get("title")
        .or_else(|| work.properties.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or(work.display_name.as_str());
    let body = work
        .properties
        .get("body")
        .or_else(|| work.properties.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    format!("{} {}", title, body).to_ascii_lowercase()
}

fn parse_prop_datetime(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // GitHub sometimes omits colon in offset or uses Z
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ") {
        return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(DateTime::from_naive_utc_and_offset(dt, Utc));
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc));
    }
    None
}

/// MergeFriction: mergeable false / dirty state / conflict language in title-body.
fn detect_merge_friction(work: &GraphNode) -> Option<Vec<String>> {
    let mut evidence = Vec::new();
    let props = &work.properties;

    if props.get("mergeable").and_then(|v| v.as_bool()) == Some(false) {
        evidence.push("mergeable:false".into());
    }
    if let Some(ms) = props
        .get("mergeable_state")
        .or_else(|| props.get("mergeableState"))
        .and_then(|v| v.as_str())
    {
        let ms_l = ms.to_ascii_lowercase();
        if matches!(
            ms_l.as_str(),
            "dirty" | "conflicted" | "unstable" | "blocked"
        ) {
            evidence.push(format!("mergeable_state:{ms_l}"));
        }
    }
    let hay = work_text_hay(work);
    for pat in [
        "merge conflict",
        "has conflicts",
        "conflicting files",
        "resolve conflicts",
        "fix merge conflicts",
        "conflicts with",
    ] {
        if hay.contains(pat) {
            evidence.push(format!("text:{pat}"));
            break;
        }
    }
    if evidence.is_empty() {
        None
    } else {
        Some(evidence)
    }
}

/// StaleReview: open PR, stale label, or updated_at older than 7 days.
fn detect_stale_review(work: &GraphNode, as_of: DateTime<Utc>) -> Option<Vec<String>> {
    let nt = work.node_type.to_ascii_lowercase();
    // Prefer PRs; issues can still go stale but signal is weaker
    let is_pr = nt.contains("pull") || nt.contains("merge_request") || work.node_id.starts_with("pr:");
    let props = &work.properties;

    let state = props
        .get("state")
        .or_else(|| props.get("status"))
        .or_else(|| props.get("lifecycle"))
        .and_then(|v| v.as_str())
        .unwrap_or("open")
        .to_ascii_lowercase();
    if matches!(
        state.as_str(),
        "closed" | "merged" | "done" | "resolved" | "declined"
    ) {
        return None;
    }

    let mut evidence = Vec::new();
    let labels = work_labels(work);
    if labels.iter().any(|l| l == "stale" || l.contains("stale")) {
        evidence.push("label:stale".into());
    }

    // updated_at / last_activity / pushed_at older than 7d
    for key in [
        "updated_at",
        "updatedAt",
        "last_activity_at",
        "pushed_at",
        "review_requested_at",
    ] {
        if let Some(s) = props.get(key).and_then(|v| v.as_str()) {
            if let Some(dt) = parse_prop_datetime(s) {
                let age = as_of.signed_duration_since(dt);
                if age.num_days() >= 7 {
                    evidence.push(format!("{key}_age_days:{}", age.num_days()));
                    break;
                }
            }
        }
    }

    // review requested + still open + optional requested_reviewers present long open
    if props
        .get("requested_reviewers")
        .or_else(|| props.get("requestedReviewers"))
        .map(|v| {
            v.as_array()
                .map(|a| !a.is_empty())
                .or_else(|| v.as_str().map(|s| !s.is_empty()))
                .unwrap_or(false)
        })
        .unwrap_or(false)
    {
        // Only flag if we already have age signal or no recent update field at all
        if evidence.iter().any(|e| e.contains("_age_days:")) {
            evidence.push("review_requested_stall".into());
        }
    }

    // Without any temporal/label signal, don't invent stale cards
    if evidence.is_empty() {
        return None;
    }
    // Prefer PRs; for issues require explicit stale label
    if !is_pr && !evidence.iter().any(|e| e.starts_with("label:stale")) {
        return None;
    }
    Some(evidence)
}

/// CiBlocked signals on a work node (failure attrs / labels).
fn detect_ci_failure(work: &GraphNode) -> Option<Vec<String>> {
    let mut evidence = Vec::new();
    let props = &work.properties;

    for (key, bad) in [
        ("check_conclusion", "failure"),
        ("check_conclusion", "cancelled"),
        ("ci_status", "failure"),
        ("ci_status", "failed"),
        ("ci_status", "error"),
        ("status", "failure"),
        ("conclusion", "failure"),
        ("checks_state", "failure"),
        ("checks_state", "failing"),
    ] {
        if let Some(s) = props.get(key).and_then(|v| v.as_str()) {
            if s.eq_ignore_ascii_case(bad) {
                evidence.push(format!("{key}:{s}"));
            }
        }
    }
    // Boolean helpers sometimes projected
    if props.get("ci_failed").and_then(|v| v.as_bool()) == Some(true) {
        evidence.push("ci_failed:true".into());
    }
    if props.get("checks_passing").and_then(|v| v.as_bool()) == Some(false) {
        evidence.push("checks_passing:false".into());
    }

    let labels = work_labels(work);
    for l in &labels {
        if l == "ci-fail"
            || l == "ci-failure"
            || l == "checks-fail"
            || l == "ci-failed"
            || l.contains("ci-fail")
        {
            evidence.push(format!("label:{l}"));
        }
    }

    if evidence.is_empty() {
        None
    } else {
        Some(evidence)
    }
}

fn intent_type_of(n: &GraphNode) -> &str {
    n.properties
        .get("intent_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn owner_of(n: &GraphNode) -> Option<String> {
    n.properties
        .get("owner_node_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Detect conflicts from ACL-visible nodes/edges (pure function).
pub fn detect_conflicts(
    tenant_id: &str,
    nodes: &[GraphNode],
    edges: &[GraphEdge],
    as_of: DateTime<Utc>,
) -> Vec<ConflictCard> {
    let mut cards = Vec::new();
    let active: Vec<&GraphEdge> = edges
        .iter()
        .filter(|e| e.valid_from <= as_of && e.valid_to.map(|t| t > as_of).unwrap_or(true))
        .collect();

    // Index intents
    let intents: Vec<&GraphNode> = nodes
        .iter()
        .filter(|n| n.node_type.eq_ignore_ascii_case("Intent"))
        .collect();

    let node_by_id: std::collections::HashMap<&str, &GraphNode> =
        nodes.iter().map(|n| (n.node_id.as_str(), n)).collect();

    // ABOUT: intent → work
    let mut about_work: std::collections::HashMap<String, Vec<&GraphNode>> =
        std::collections::HashMap::new();
    for e in &active {
        if e.edge_type.eq_ignore_ascii_case("ABOUT") {
            if let Some(intent) = intents.iter().find(|i| i.node_id == e.from_node_id) {
                about_work
                    .entry(e.to_node_id.clone())
                    .or_default()
                    .push(*intent);
            }
        }
    }
    // Also index intents by properties.about_node_id when edges missing (seed / partial graphs)
    for intent in &intents {
        if let Some(about) = intent
            .properties
            .get("about_node_id")
            .and_then(|v| v.as_str())
        {
            let list = about_work.entry(about.to_string()).or_default();
            if !list.iter().any(|i| i.node_id == intent.node_id) {
                list.push(*intent);
            }
        }
    }

    // Dual owners / competing intents on same work item
    for (work_id, list) in &about_work {
        if list.len() < 2 {
            continue;
        }
        let mut types: Vec<String> = list
            .iter()
            .map(|i| intent_type_of(i).to_string())
            .filter(|s| !s.is_empty())
            .collect();
        types.sort();
        types.dedup();
        let owners: Vec<String> = list.iter().filter_map(|i| owner_of(i)).collect();
        let unique_owners: std::collections::HashSet<_> = owners.iter().cloned().collect();
        if unique_owners.len() > 1 {
            let has_ship = types.iter().any(|t| t == "SHIP");
            let has_freeze = types.iter().any(|t| t == "FREEZE");
            let (kind, severity, summary) = if has_ship && has_freeze {
                (
                    ConflictKind::ShipVsFreeze,
                    ConflictSeverity::High,
                    format!("SHIP vs FREEZE on {work_id}"),
                )
            } else {
                (
                    ConflictKind::DualOwners,
                    ConflictSeverity::Medium,
                    format!(
                        "Multiple owners claim intents on {work_id}: {}",
                        types.join(", ")
                    ),
                )
            };
            let mut evidence: Vec<String> = list.iter().map(|i| i.display_name.clone()).collect();
            note_mixed_seed(&mut evidence, list);
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_{work_id}_owners"),
                kind,
                summary,
                severity,
                node_ids: std::iter::once(work_id.clone())
                    .chain(list.iter().map(|i| i.node_id.clone()))
                    .collect(),
                owner_node_ids: unique_owners.into_iter().collect(),
                intent_types: types,
                evidence,
                is_demo: intents_all_demo(list),
            });
        } else if types.contains(&"SHIP".into()) && types.contains(&"FREEZE".into()) {
            let mut evidence: Vec<String> = list.iter().map(|i| i.display_name.clone()).collect();
            note_mixed_seed(&mut evidence, list);
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_{work_id}_ship_freeze"),
                kind: ConflictKind::ShipVsFreeze,
                summary: format!("SHIP vs FREEZE on {work_id}"),
                severity: ConflictSeverity::High,
                node_ids: std::iter::once(work_id.clone())
                    .chain(list.iter().map(|i| i.node_id.clone()))
                    .collect(),
                owner_node_ids: owners,
                intent_types: types,
                evidence,
                is_demo: intents_all_demo(list),
            });
        }
    }

    // BLOCKS edges: dual blocks (A blocks B and B blocks A) or multi-blocker fan-in
    let blocks: Vec<&&GraphEdge> = active
        .iter()
        .filter(|e| e.edge_type.eq_ignore_ascii_case("BLOCKS"))
        .collect();
    for i in 0..blocks.len() {
        for j in (i + 1)..blocks.len() {
            let a = blocks[i];
            let b = blocks[j];
            if a.from_node_id == b.to_node_id && a.to_node_id == b.from_node_id {
                // Pull intents on either endpoint for is_demo
                let mut inv: Vec<&GraphNode> = Vec::new();
                for wid in [&a.from_node_id, &a.to_node_id] {
                    if let Some(list) = about_work.get(wid.as_str()) {
                        inv.extend(list.iter().copied());
                    }
                }
                cards.push(ConflictCard {
                    conflict_id: format!(
                        "cfl_{tenant_id}_mutual_{}_{}",
                        a.from_node_id, a.to_node_id
                    ),
                    kind: ConflictKind::DualBlocks,
                    summary: format!(
                        "Mutual BLOCKS between {} and {}",
                        a.from_node_id, a.to_node_id
                    ),
                    severity: ConflictSeverity::High,
                    node_ids: vec![a.from_node_id.clone(), a.to_node_id.clone()],
                    owner_node_ids: vec![],
                    intent_types: vec!["BLOCKED".into()],
                    evidence: vec![a.edge_id.clone(), b.edge_id.clone()],
                    is_demo: intents_all_demo(&inv),
                });
            }
        }
    }

    // Open BLOCKED intents
    for intent in &intents {
        if intent_type_of(intent) == "BLOCKED" {
            let about = intent
                .properties
                .get("about_node_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let inv = vec![*intent];
            let mut evidence = intent
                .properties
                .get("evidence")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            note_mixed_seed(&mut evidence, &inv);
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_blocked_{}", intent.node_id),
                kind: ConflictKind::OpenBlocker,
                summary: intent.display_name.clone(),
                severity: ConflictSeverity::Medium,
                node_ids: vec![intent.node_id.clone(), about.to_string()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect(),
                owner_node_ids: owner_of(intent).into_iter().collect::<Vec<_>>(),
                intent_types: vec!["BLOCKED".into()],
                evidence,
                is_demo: intents_all_demo(&inv),
            });
        }
    }

    // SHIP intent that is target of a BLOCKS edge
    for e in &blocks {
        if let Some(list) = about_work.get(&e.to_node_id) {
            if list.iter().any(|i| intent_type_of(i) == "SHIP") {
                let mut evidence = vec![e.edge_id.clone()];
                note_mixed_seed(&mut evidence, list);
                cards.push(ConflictCard {
                    conflict_id: format!(
                        "cfl_{tenant_id}_blocks_ship_{}_{}",
                        e.from_node_id, e.to_node_id
                    ),
                    kind: ConflictKind::DualBlocks,
                    summary: format!(
                        "{} blocks SHIP target {}",
                        e.from_node_id, e.to_node_id
                    ),
                    severity: ConflictSeverity::High,
                    node_ids: vec![e.from_node_id.clone(), e.to_node_id.clone()],
                    owner_node_ids: vec![],
                    intent_types: vec!["SHIP".into(), "BLOCKED".into()],
                    evidence,
                    is_demo: intents_all_demo(list),
                });
            }
        }
    }

    // ── Organic friction: MergeFriction / StaleReview / CiBlocked ──────────
    // Consider work nodes present in the neighborhood, plus ABOUT targets.
    let mut work_ids: std::collections::HashSet<String> = about_work.keys().cloned().collect();
    for n in nodes {
        let t = n.node_type.to_ascii_lowercase();
        if t.contains("pull")
            || t == "issue"
            || t.contains("merge_request")
            || n.node_id.starts_with("pr:")
            || n.node_id.starts_with("issue:")
        {
            work_ids.insert(n.node_id.clone());
        }
    }

    for work_id in &work_ids {
        let work = match node_by_id.get(work_id.as_str()) {
            Some(w) => *w,
            None => continue,
        };
        let inv: Vec<&GraphNode> = about_work
            .get(work_id)
            .map(|v| v.clone())
            .unwrap_or_default();
        let owners: Vec<String> = inv.iter().filter_map(|i| owner_of(i)).collect();
        let intent_types: Vec<String> = {
            let mut t: Vec<String> = inv
                .iter()
                .map(|i| intent_type_of(i).to_string())
                .filter(|s| !s.is_empty())
                .collect();
            t.sort();
            t.dedup();
            t
        };

        if let Some(mut ev) = detect_merge_friction(work) {
            note_mixed_seed(&mut ev, &inv);
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_merge_friction_{work_id}"),
                kind: ConflictKind::MergeFriction,
                summary: format!("Merge friction on {work_id}"),
                severity: ConflictSeverity::High,
                node_ids: std::iter::once(work_id.clone())
                    .chain(inv.iter().map(|i| i.node_id.clone()))
                    .collect(),
                owner_node_ids: owners.clone(),
                intent_types: intent_types.clone(),
                evidence: ev,
                is_demo: intents_all_demo(&inv)
                    || (inv.is_empty() && node_is_demo(work)),
            });
        }

        if let Some(mut ev) = detect_stale_review(work, as_of) {
            note_mixed_seed(&mut ev, &inv);
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_stale_review_{work_id}"),
                kind: ConflictKind::StaleReview,
                summary: format!("Stale / stalled review on {work_id}"),
                severity: ConflictSeverity::Medium,
                node_ids: std::iter::once(work_id.clone())
                    .chain(inv.iter().map(|i| i.node_id.clone()))
                    .collect(),
                owner_node_ids: owners.clone(),
                intent_types: intent_types.clone(),
                evidence: ev,
                is_demo: intents_all_demo(&inv)
                    || (inv.is_empty() && node_is_demo(work)),
            });
        }

        // CiBlocked only when intent SHIP (claim vs red CI)
        let has_ship = inv.iter().any(|i| intent_type_of(i) == "SHIP");
        if has_ship {
            if let Some(mut ev) = detect_ci_failure(work) {
                note_mixed_seed(&mut ev, &inv);
                cards.push(ConflictCard {
                    conflict_id: format!("cfl_{tenant_id}_ci_blocked_{work_id}"),
                    kind: ConflictKind::CiBlocked,
                    summary: format!("CI blocked SHIP on {work_id}"),
                    severity: ConflictSeverity::High,
                    node_ids: std::iter::once(work_id.clone())
                        .chain(inv.iter().map(|i| i.node_id.clone()))
                        .collect(),
                    owner_node_ids: owners,
                    intent_types: vec!["SHIP".into()],
                    evidence: ev,
                    is_demo: intents_all_demo(&inv),
                });
            }
        }
    }

    // Dedupe by conflict_id
    let mut seen = std::collections::HashSet::new();
    cards.retain(|c| seen.insert(c.conflict_id.clone()));
    cards.sort_by(|a, b| {
        severity_rank(b.severity)
            .cmp(&severity_rank(a.severity))
            .then_with(|| a.conflict_id.cmp(&b.conflict_id))
    });
    cards
}

fn severity_rank(s: ConflictSeverity) -> u8 {
    match s {
        ConflictSeverity::High => 3,
        ConflictSeverity::Medium => 2,
        ConflictSeverity::Low => 1,
    }
}

/// Convenience: classify from a work GraphNode's properties (title/labels).
pub fn claim_from_work_node(
    work: &GraphNode,
    owner_node_id: Option<String>,
) -> IntentClaim {
    let title = work
        .properties
        .get("title")
        .or_else(|| work.properties.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or(work.display_name.as_str())
        .to_string();
    let mut labels = Vec::new();
    if let Some(arr) = work.properties.get("labels").and_then(|v| v.as_array()) {
        for l in arr {
            if let Some(s) = l.as_str() {
                labels.push(s.to_string());
            } else if let Some(n) = l.get("name").and_then(|v| v.as_str()) {
                labels.push(n.to_string());
            }
        }
    }
    let body = work
        .properties
        .get("body")
        .or_else(|| work.properties.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let (intent_type, confidence, evidence) = classify_intent(&title, &labels, &body);
    IntentClaim {
        intent_type,
        summary: if title.is_empty() {
            format!("{}", intent_type)
        } else {
            format!("{intent_type}: {title}")
        },
        about_node_id: work.node_id.clone(),
        owner_node_id,
        confidence,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent_node(
        id: &str,
        itype: &str,
        about: &str,
        owner: &str,
        is_demo: bool,
    ) -> GraphNode {
        GraphNode {
            tenant_id: "t".into(),
            node_id: id.into(),
            node_type: "Intent".into(),
            display_name: format!("{itype}: {about}"),
            resource_id: about.into(),
            properties: json!({
                "intent_type": itype,
                "about_node_id": about,
                "owner_node_id": owner,
                "classified_by": "rules_v0",
                "source": if is_demo { "seed" } else { "github_pr" },
                "is_demo": is_demo,
            }),
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 0,
        }
    }

    fn about_edge(from_intent: &str, to_work: &str) -> GraphEdge {
        GraphEdge {
            tenant_id: "t".into(),
            edge_id: format!("e_{from_intent}"),
            edge_type: "ABOUT".into(),
            from_node_id: from_intent.into(),
            to_node_id: to_work.into(),
            valid_from: Utc::now() - chrono::Duration::hours(1),
            valid_to: None,
            event_id: "ev".into(),
            properties: json!({}),
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 0,
        }
    }

    fn pr_node(id: &str, props: serde_json::Value) -> GraphNode {
        GraphNode {
            tenant_id: "t".into(),
            node_id: id.into(),
            node_type: "PullRequest".into(),
            display_name: id.into(),
            resource_id: id.into(),
            properties: props,
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 0,
        }
    }

    #[test]
    fn classifies_blocked_label() {
        let (t, c, _) = classify_intent("Auth rewrite", &["blocked".into()], "");
        assert_eq!(t, IntentType::Blocked);
        assert!(c >= 0.9);
    }

    #[test]
    fn classifies_ship_feat() {
        let (t, _, ev) = classify_intent("feat: add status digests", &[], "");
        assert_eq!(t, IntentType::Ship);
        assert!(!ev.is_empty());
    }

    #[test]
    fn classifies_freeze() {
        let (t, _, _) = classify_intent("do not merge platform changes", &[], "");
        assert_eq!(t, IntentType::Freeze);
    }

    #[test]
    fn draft_prefers_freeze_not_ship() {
        let (t, c, ev) = classify_intent("WIP feature branch", &["draft".into()], "");
        assert_eq!(t, IntentType::Freeze);
        assert!(c >= 0.7);
        assert!(ev.iter().any(|e| e.contains("draft")));
    }

    #[test]
    fn weak_empty_title_not_confident_ship() {
        let (t, c, ev) = classify_intent("", &[], "pull_request opened");
        // May be SHIP but confidence ≤0.45 with default:open_pr_weak
        assert!(c <= 0.45);
        if t == IntentType::Ship {
            assert!(ev.iter().any(|e| e == "default:open_pr_weak"));
        }
    }

    #[test]
    fn noise_title_uses_open_pr_weak() {
        let (t, c, ev) = classify_intent("...", &[], "pull_request");
        assert_eq!(t, IntentType::Ship);
        assert!(c <= 0.45);
        assert!(ev.iter().any(|e| e == "default:open_pr_weak"));
    }

    #[test]
    fn dual_owners_conflict() {
        let nodes = vec![
            intent_node("intent:pr:1", "SHIP", "pr:1", "person:a", false),
            intent_node("intent:pr:1b", "FREEZE", "pr:1", "person:b", false),
        ];
        let edges = vec![
            about_edge("intent:pr:1", "pr:1"),
            about_edge("intent:pr:1b", "pr:1"),
        ];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        assert!(!cards.is_empty());
        assert!(cards.iter().any(|c| matches!(
            c.kind,
            ConflictKind::ShipVsFreeze | ConflictKind::DualOwners
        )));
        // Organic → not demo
        let card = cards
            .iter()
            .find(|c| matches!(c.kind, ConflictKind::ShipVsFreeze | ConflictKind::DualOwners))
            .unwrap();
        assert!(!card.is_demo);
    }

    #[test]
    fn merge_friction_from_dirty_state() {
        let work = pr_node(
            "pr:org/repo/pr/9",
            json!({
                "title": "feat: landing page",
                "state": "open",
                "mergeable": false,
                "mergeable_state": "dirty",
            }),
        );
        let intent = intent_node(
            "intent:p:pr:9",
            "SHIP",
            "pr:org/repo/pr/9",
            "person:a",
            false,
        );
        let nodes = vec![work, intent];
        let edges = vec![about_edge("intent:p:pr:9", "pr:org/repo/pr/9")];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        let mf = cards
            .iter()
            .find(|c| c.kind == ConflictKind::MergeFriction)
            .expect("MergeFriction card");
        assert_eq!(mf.severity, ConflictSeverity::High);
        assert!(!mf.is_demo);
        assert!(mf.evidence.iter().any(|e| e.contains("mergeable") || e.contains("dirty")));
    }

    #[test]
    fn merge_friction_from_conflict_language() {
        let work = pr_node(
            "pr:x/1",
            json!({
                "title": "fix merge conflicts with main",
                "state": "OPEN",
            }),
        );
        let nodes = vec![work];
        let cards = detect_conflicts("t", &nodes, &[], Utc::now());
        assert!(cards.iter().any(|c| c.kind == ConflictKind::MergeFriction));
    }

    #[test]
    fn stale_review_from_old_updated_at() {
        let old = (Utc::now() - chrono::Duration::days(14)).to_rfc3339();
        let work = pr_node(
            "pr:stale/1",
            json!({
                "title": "long open PR",
                "state": "open",
                "updated_at": old,
            }),
        );
        let nodes = vec![work];
        let cards = detect_conflicts("t", &nodes, &[], Utc::now());
        let sr = cards
            .iter()
            .find(|c| c.kind == ConflictKind::StaleReview)
            .expect("StaleReview");
        assert_eq!(sr.severity, ConflictSeverity::Medium);
        assert!(sr.evidence.iter().any(|e| e.contains("age_days")));
    }

    #[test]
    fn stale_review_from_label() {
        let work = pr_node(
            "pr:stale/2",
            json!({
                "title": "maybe stale",
                "state": "open",
                "labels": ["stale"],
            }),
        );
        let cards = detect_conflicts("t", &[work], &[], Utc::now());
        assert!(cards.iter().any(|c| c.kind == ConflictKind::StaleReview));
    }

    #[test]
    fn ci_blocked_when_ship_and_check_failure() {
        let work = pr_node(
            "pr:ci/1",
            json!({
                "title": "ready to ship feature",
                "state": "open",
                "check_conclusion": "failure",
                "ci_status": "failure",
            }),
        );
        let intent = intent_node("intent:ci:1", "SHIP", "pr:ci/1", "person:a", false);
        let nodes = vec![work, intent];
        let edges = vec![about_edge("intent:ci:1", "pr:ci/1")];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        let ci = cards
            .iter()
            .find(|c| c.kind == ConflictKind::CiBlocked)
            .expect("CiBlocked");
        assert_eq!(ci.severity, ConflictSeverity::High);
        assert!(!ci.is_demo);
    }

    #[test]
    fn ci_blocked_not_without_ship_intent() {
        let work = pr_node(
            "pr:ci/2",
            json!({
                "title": "experiment",
                "state": "open",
                "check_conclusion": "failure",
            }),
        );
        let intent = intent_node("intent:ci:2", "EXPLORE", "pr:ci/2", "person:a", false);
        let nodes = vec![work, intent];
        let edges = vec![about_edge("intent:ci:2", "pr:ci/2")];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        assert!(!cards.iter().any(|c| c.kind == ConflictKind::CiBlocked));
    }

    #[test]
    fn demo_conflict_is_demo_true_when_all_seed() {
        let nodes = vec![
            intent_node("intent:d1", "SHIP", "pr:demo", "person:gu_demo_alice", true),
            intent_node("intent:d2", "FREEZE", "pr:demo", "person:gu_demo_bob", true),
        ];
        let edges = vec![
            about_edge("intent:d1", "pr:demo"),
            about_edge("intent:d2", "pr:demo"),
        ];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        let card = cards
            .iter()
            .find(|c| matches!(c.kind, ConflictKind::ShipVsFreeze | ConflictKind::DualOwners))
            .unwrap();
        assert!(card.is_demo);
    }

    #[test]
    fn mixed_demo_organic_is_not_demo() {
        let nodes = vec![
            intent_node("intent:m1", "SHIP", "pr:mix", "person:real", false),
            intent_node("intent:m2", "FREEZE", "pr:mix", "person:gu_demo_bob", true),
        ];
        let edges = vec![
            about_edge("intent:m1", "pr:mix"),
            about_edge("intent:m2", "pr:mix"),
        ];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        let card = cards
            .iter()
            .find(|c| matches!(c.kind, ConflictKind::ShipVsFreeze | ConflictKind::DualOwners))
            .unwrap();
        assert!(!card.is_demo);
        assert!(card.evidence.iter().any(|e| e == "includes_seed_intent"));
    }

    #[test]
    fn attach_skips_commit_nodes() {
        use crate::v1_event::{V1Acl, V1Actor, V1CanonicalEvent};
        let event = V1CanonicalEvent {
            event_id: "e1".into(),
            tenant_id: "t".into(),
            provider: "github".into(),
            category: "code".into(),
            event_type: "push".into(),
            event_timestamp: Utc::now(),
            ingested_at: Utc::now(),
            actor: V1Actor {
                global_user_id: "gu_x".into(),
                provider_user_id: "x".into(),
                email: String::new(),
                display_name: "x".into(),
            },
            acl: V1Acl {
                tenant_id: "t".into(),
                allowed_group_ids: vec!["g".into()],
                is_private: false,
                acl_version: 1,
            },
            resource_id: "org/repo".into(),
            parent_resource_id: String::new(),
            attributes: json!({ "title": "feat: should not become intent" }),
            raw_payload_s3_uri: String::new(),
            event_sequence_number: 0,
        };
        let commit = GraphNode {
            tenant_id: "t".into(),
            node_id: "commit:abc".into(),
            node_type: "Commit".into(),
            display_name: "abc".into(),
            resource_id: "abc".into(),
            properties: json!({ "message": "feat: ship it" }),
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 0,
        };
        let m = attach_intent_mutation(&event, &commit, "person:gu_x", "feat: ship it", &[], "");
        assert!(m.nodes.is_empty());
        assert!(m.edges.is_empty());
    }

    #[test]
    fn attach_sets_provenance_organic() {
        use crate::v1_event::{V1Acl, V1Actor, V1CanonicalEvent};
        let event = V1CanonicalEvent {
            event_id: "e2".into(),
            tenant_id: "t".into(),
            provider: "github".into(),
            category: "code".into(),
            event_type: "pull_request.opened".into(),
            event_timestamp: Utc::now(),
            ingested_at: Utc::now(),
            actor: V1Actor {
                global_user_id: "gu_x".into(),
                provider_user_id: "x".into(),
                email: String::new(),
                display_name: "x".into(),
            },
            acl: V1Acl {
                tenant_id: "t".into(),
                allowed_group_ids: vec!["g".into()],
                is_private: false,
                acl_version: 1,
            },
            resource_id: "org/repo/pr/1".into(),
            parent_resource_id: "org/repo".into(),
            attributes: json!({ "title": "feat: real work" }),
            raw_payload_s3_uri: String::new(),
            event_sequence_number: 0,
        };
        let pr = GraphNode {
            tenant_id: "t".into(),
            node_id: "pr:org/repo/pr/1".into(),
            node_type: "PullRequest".into(),
            display_name: "feat: real work".into(),
            resource_id: "org/repo/pr/1".into(),
            properties: json!({ "title": "feat: real work" }),
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 0,
        };
        let m = attach_intent_mutation(
            &event,
            &pr,
            "person:gu_x",
            "feat: real work",
            &[],
            "",
        );
        let intent = m.nodes.iter().find(|n| n.node_type == "Intent").unwrap();
        assert_eq!(
            intent.properties.get("classified_by").and_then(|v| v.as_str()),
            Some("rules_v0")
        );
        assert_eq!(
            intent.properties.get("source").and_then(|v| v.as_str()),
            Some("github_pr")
        );
        assert_eq!(
            intent.properties.get("is_demo").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert!(intent.properties.get("confidence").is_some());
        assert!(intent.properties.get("evidence").is_some());
    }
}
