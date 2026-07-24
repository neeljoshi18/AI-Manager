//! Intent classification v0 + conflict detection (rules only — no LLM).
//!
//! Pipeline (roadmap): extract → type → attach to graph → conflict detect → surface.
//! Facts = webhook/work items; intents = claims about purpose (SHIP / BLOCKED / …).

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

    // Open PR without signal → mild SHIP (shipping work-in-progress default for eng digests)
    if hay.contains("pull_request") || title.len() > 3 {
        evidence.push("default:work_item".into());
        return (IntentType::Ship, 0.4, evidence);
    }

    evidence.push("default:other".into());
    (IntentType::Other, 0.3, evidence)
}

/// Build Intent node + CLAIMS (person→intent) + ABOUT (intent→work) edges.
pub fn attach_intent_mutation(
    event: &V1CanonicalEvent,
    about_node: &GraphNode,
    owner_person_node_id: &str,
    title: &str,
    labels: &[String],
    body: &str,
) -> GraphMutation {
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
pub fn merge_intent_into(
    mut base: GraphMutation,
    event: &V1CanonicalEvent,
    about_node_id: &str,
) -> GraphMutation {
    let about = match base.nodes.iter().find(|n| n.node_id == about_node_id) {
        Some(n) => n.clone(),
        None => return base,
    };
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

    // Dual owners / competing intents on same work item
    for (work_id, list) in &about_work {
        if list.len() < 2 {
            // Still flag SHIP+FREEZE if somehow two types on one node properties
            continue;
        }
        let mut types: Vec<String> = list
            .iter()
            .filter_map(|i| {
                i.properties
                    .get("intent_type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
        types.sort();
        types.dedup();
        let owners: Vec<String> = list
            .iter()
            .filter_map(|i| {
                i.properties
                    .get("owner_node_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();
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
                evidence: list
                    .iter()
                    .map(|i| i.display_name.clone())
                    .collect(),
            });
        } else if types.contains(&"SHIP".into()) && types.contains(&"FREEZE".into()) {
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
                evidence: list.iter().map(|i| i.display_name.clone()).collect(),
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
                });
            }
        }
    }

    // Open BLOCKED intents
    for intent in &intents {
        let ty = intent
            .properties
            .get("intent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if ty == "BLOCKED" {
            let about = intent
                .properties
                .get("about_node_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            cards.push(ConflictCard {
                conflict_id: format!("cfl_{tenant_id}_blocked_{}", intent.node_id),
                kind: ConflictKind::OpenBlocker,
                summary: intent.display_name.clone(),
                severity: ConflictSeverity::Medium,
                node_ids: vec![intent.node_id.clone(), about.to_string()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect(),
                owner_node_ids: intent
                    .properties
                    .get("owner_node_id")
                    .and_then(|v| v.as_str())
                    .map(|s| vec![s.to_string()])
                    .unwrap_or_default(),
                intent_types: vec!["BLOCKED".into()],
                evidence: intent
                    .properties
                    .get("evidence")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
    }

    // SHIP intent that is target of a BLOCKS edge
    for e in &blocks {
        if let Some(list) = about_work.get(&e.to_node_id) {
            if list.iter().any(|i| {
                i.properties
                    .get("intent_type")
                    .and_then(|v| v.as_str())
                    == Some("SHIP")
            }) {
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
                    evidence: vec![e.edge_id.clone()],
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
    fn dual_owners_conflict() {
        let nodes = vec![
            GraphNode {
                tenant_id: "t".into(),
                node_id: "intent:pr:1".into(),
                node_type: "Intent".into(),
                display_name: "SHIP: a".into(),
                resource_id: "r".into(),
                properties: json!({
                    "intent_type": "SHIP",
                    "about_node_id": "pr:1",
                    "owner_node_id": "person:a"
                }),
                is_private: false,
                allowed_group_ids: vec![],
                acl_version: 0,
            },
            GraphNode {
                tenant_id: "t".into(),
                node_id: "intent:pr:1b".into(),
                node_type: "Intent".into(),
                display_name: "FREEZE: b".into(),
                resource_id: "r".into(),
                properties: json!({
                    "intent_type": "FREEZE",
                    "about_node_id": "pr:1",
                    "owner_node_id": "person:b"
                }),
                is_private: false,
                allowed_group_ids: vec![],
                acl_version: 0,
            },
        ];
        let edges = vec![
            GraphEdge {
                tenant_id: "t".into(),
                edge_id: "e1".into(),
                edge_type: "ABOUT".into(),
                from_node_id: "intent:pr:1".into(),
                to_node_id: "pr:1".into(),
                valid_from: Utc::now() - chrono::Duration::hours(1),
                valid_to: None,
                event_id: "ev1".into(),
                properties: json!({}),
                is_private: false,
                allowed_group_ids: vec![],
                acl_version: 0,
            },
            GraphEdge {
                tenant_id: "t".into(),
                edge_id: "e2".into(),
                edge_type: "ABOUT".into(),
                from_node_id: "intent:pr:1b".into(),
                to_node_id: "pr:1".into(),
                valid_from: Utc::now() - chrono::Duration::hours(1),
                valid_to: None,
                event_id: "ev2".into(),
                properties: json!({}),
                is_private: false,
                allowed_group_ids: vec![],
                acl_version: 0,
            },
        ];
        let cards = detect_conflicts("t", &nodes, &edges, Utc::now());
        assert!(!cards.is_empty());
        assert!(cards.iter().any(|c| matches!(
            c.kind,
            ConflictKind::ShipVsFreeze | ConflictKind::DualOwners
        )));
    }
}
