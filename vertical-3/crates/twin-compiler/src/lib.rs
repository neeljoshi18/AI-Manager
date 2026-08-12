//! Ledger compiler: ACL-scoped V2 graph views → StatusLedger snapshots.
//!
//! Invariant: never god-mode SQL into context_graph — only GraphSource (HTTP V2 or fixtures).

pub mod fixtures;
mod graph_source;
mod http_v2;
mod overlay;

pub use fixtures::FixtureGraphSource;
pub use graph_source::GraphSource;
pub use http_v2::HttpV2GraphSource;
pub use overlay::OverlayGraphSource;

use chrono::{DateTime, Utc};
use twin_core::confidence::{apply_rollup, score_item_confidence};
use twin_core::ids::ledger_id_for;
use twin_core::ledger_text::render_draft_text;
use twin_core::model::*;
use twin_core::store::TwinStore;
use twin_core::{TwinError, TwinResult};
use uuid::Uuid;

/// Compile options for a single run.
#[derive(Debug, Clone)]
pub struct CompileOpts {
    /// Aligned wall-clock bucket (stable ledger_id within the bucket).
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    /// Rolling activity lookback used to include/exclude graph signals.
    /// Defaults to the same as period when unset via constructors; twin-api sets rolling lookback.
    pub activity_start: DateTime<Utc>,
    pub activity_end: DateTime<Utc>,
    pub hops: usize,
}

impl Default for CompileOpts {
    fn default() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(24);
        Self {
            period_start: start,
            period_end: end,
            activity_start: start,
            activity_end: end,
            hops: 3,
        }
    }
}

impl CompileOpts {
    /// Convenience: one window for both ledger period and activity filter.
    pub fn window(start: DateTime<Utc>, end: DateTime<Utc>, hops: usize) -> Self {
        Self {
            period_start: start,
            period_end: end,
            activity_start: start,
            activity_end: end,
            hops,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileOutcome {
    pub run_id: String,
    pub ledger: LedgerSnapshot,
    pub draft_text: String,
    pub acl_empty: bool,
}

/// Compiler that reads GraphSource as the twin person and writes ledgers via TwinStore.
pub struct LedgerCompiler {
    store: std::sync::Arc<dyn TwinStore>,
    source: std::sync::Arc<dyn GraphSource>,
}

impl LedgerCompiler {
    pub fn new(
        store: std::sync::Arc<dyn TwinStore>,
        source: std::sync::Arc<dyn GraphSource>,
    ) -> Self {
        Self { store, source }
    }

    /// Compile ledger for a person twin using V2 ACL APIs (or fixtures).
    pub async fn compile_person(
        &self,
        twin: &Twin,
        opts: &CompileOpts,
    ) -> TwinResult<CompileOutcome> {
        if twin.twin_kind != TwinKind::Person {
            return Err(TwinError::Validation(
                "compile_person requires person twin".into(),
            ));
        }
        if twin.tenant_id.is_empty() {
            return Err(TwinError::Validation("tenant_id required".into()));
        }

        let run_id = format!("run_{}", Uuid::new_v4());
        let started = Utc::now();

        // Primary subject + any gu_* aliases (historical identity after restarts)
        let mut subject_ids: Vec<String> = vec![twin.subject_id.clone()];
        if let Some(arr) = twin
            .config_json
            .get("provider_aliases")
            .and_then(|v| v.as_array())
        {
            for a in arr {
                if let Some(s) = a.as_str() {
                    let s = s.trim();
                    if s.starts_with("gu_") && !subject_ids.iter().any(|x| x == s) {
                        subject_ids.push(s.to_string());
                    }
                }
            }
        }

        let mut view = GraphView::default();
        let mut person_nodes: Vec<String> = Vec::new();
        for sid in &subject_ids {
            let pn = format!("person:{sid}");
            person_nodes.push(pn.clone());
            match self
                .source
                .fetch_person_view(&twin.tenant_id, sid, &pn, opts.hops)
                .await
            {
                Ok(part) => merge_graph_view(&mut view, part),
                Err(e) => {
                    tracing::debug!(subject = %sid, error = %e, "compile fetch_person_view skip");
                }
            }
        }
        let graph_as_of = view.graph_as_of.unwrap_or_else(Utc::now);
        let compiled_at = Utc::now();
        let acl_empty = view.nodes.is_empty() && view.edges.is_empty() && view.blockers.is_empty();

        let mut open_blockers = Vec::new();

        let is_person_endpoint = |id: &str| person_nodes.iter().any(|p| p == id);

        // Map PR / Issue / Commit / Repo activity with evidence from person-linked edges.
        // Activity lookback is rolling (activity_start..activity_end); open PR/issue stay in.
        let act_start = opts.activity_start;
        let act_end = opts.activity_end;
        let mut ranked: Vec<(Option<DateTime<Utc>>, LedgerItem)> = Vec::new();

        for node in &view.nodes {
            if is_person_endpoint(&node.node_id) {
                continue;
            }
            // Never put demo/seed theater work objects into digests (story-1, intent_demo PRs).
            if node_view_is_demo_seed(node) {
                continue;
            }
            let ntype = node.node_type.as_str();
            let is_pr = ntype.eq_ignore_ascii_case("PullRequest")
                || node.node_id.starts_with("pr:")
                || node.node_id.contains("/pr/");
            let is_issue = ntype.eq_ignore_ascii_case("Issue")
                || ntype.eq_ignore_ascii_case("Ticket")
                || node.node_id.starts_with("issue:")
                || node.node_id.starts_with("ticket:");
            let is_commit = ntype.eq_ignore_ascii_case("Commit")
                || node.node_id.starts_with("commit:");
            let is_repo =
                ntype.eq_ignore_ascii_case("Repo") || node.node_id.starts_with("repo:");
            if !(is_pr || is_issue || is_commit || is_repo) {
                continue;
            }

            // Activity: AUTHORED / ASSIGNED / PUSHED_TO / CHECKED / any person↔node edge
            // Match any historical gu_* alias for this twin (identity restarts).
            let authored = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("AUTHORED")
                    && (e.to_node_id == node.node_id || e.from_node_id == node.node_id)
                    && (is_person_endpoint(&e.from_node_id) || is_person_endpoint(&e.to_node_id))
            });
            let assigned = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("ASSIGNED_TO")
                    && (is_person_endpoint(&e.from_node_id) || is_person_endpoint(&e.to_node_id))
                    && (e.to_node_id == node.node_id || e.from_node_id == node.node_id)
            });
            let pushed = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("PUSHED_TO")
                    && is_person_endpoint(&e.from_node_id)
                    && e.to_node_id == node.node_id
            });
            let checked = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("CHECKED")
                    && is_person_endpoint(&e.from_node_id)
                    && e.to_node_id == node.node_id
            });
            let linked = view.edges.iter().any(|e| {
                (is_person_endpoint(&e.from_node_id) && e.to_node_id == node.node_id)
                    || (is_person_endpoint(&e.to_node_id) && e.from_node_id == node.node_id)
            });
            if authored.is_none()
                && assigned.is_none()
                && pushed.is_none()
                && checked.is_none()
                && !linked
            {
                continue;
            }

            let state = view
                .states
                .iter()
                .find(|s| s.node_id == node.node_id && s.state_key == "lifecycle")
                .or_else(|| {
                    view.states
                        .iter()
                        .find(|s| s.node_id == node.node_id && s.state_key == "status")
                });

            let lifecycle = state
                .map(|s| s.state_value.as_str())
                .unwrap_or(if is_commit || is_repo { "OPEN" } else { "OPEN" });
            let open_work = (is_pr || is_issue)
                && !matches!(lifecycle, "MERGED" | "CLOSED" | "DONE" | "RESOLVED");

            // Newest person-linked edge time (or state as_of for closed work)
            let mut activity_ts: Option<DateTime<Utc>> = None;
            for e in [authored, assigned, pushed, checked].into_iter().flatten() {
                if let Some(ts) = e.valid_from {
                    activity_ts = Some(activity_ts.map_or(ts, |cur| cur.max(ts)));
                }
            }
            if activity_ts.is_none() {
                for e in view.edges.iter().filter(|e| {
                    (is_person_endpoint(&e.from_node_id) && e.to_node_id == node.node_id)
                        || (is_person_endpoint(&e.to_node_id) && e.from_node_id == node.node_id)
                }) {
                    if let Some(ts) = e.valid_from {
                        activity_ts = Some(activity_ts.map_or(ts, |cur| cur.max(ts)));
                    }
                }
            }
            if let Some(s) = state {
                activity_ts = Some(activity_ts.map_or(s.as_of, |cur| cur.max(s.as_of)));
            }

            // Rolling lookback: timestamped signals outside window drop unless open PR/issue.
            // Missing timestamps (fixtures / pre-valid_from payloads) stay included.
            if !open_work {
                if let Some(ts) = activity_ts {
                    if ts < act_start || ts > act_end {
                        continue;
                    }
                }
            }

            let conf = score_item_confidence(lifecycle, true);

            let mut evidence = Vec::new();
            for e in [authored, assigned, pushed, checked].into_iter().flatten() {
                evidence.push(format!("edge:{}", e.edge_id));
                if !e.event_id.is_empty() {
                    evidence.push(format!("event:{}", e.event_id));
                }
                if let Some(ts) = e.valid_from {
                    evidence.push(format!("at:{}", ts.to_rfc3339()));
                }
            }
            if let Some(s) = state {
                if !s.event_id.is_empty() {
                    evidence.push(format!("event:{}", s.event_id));
                }
            }
            if evidence.is_empty() {
                evidence.push(format!("node:{}", node.node_id));
            }
            // Dedupe evidence while preserving order
            let mut seen_ev = std::collections::HashSet::new();
            evidence.retain(|e| seen_ev.insert(e.clone()));

            let kind = if is_pr {
                "pr"
            } else if is_issue {
                "issue"
            } else if is_commit {
                "commit"
            } else {
                "repo"
            };

            let title = node
                .properties
                .get("title")
                .or_else(|| node.properties.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or(node.display_name.as_str());
            let summary = if is_commit {
                if title.is_empty() {
                    format!("Commit {}", node.resource_id.chars().take(7).collect::<String>())
                } else {
                    format!("Commit: {title}")
                }
            } else if is_repo {
                if pushed.is_some() {
                    format!("Pushed to {}", if title.is_empty() { &node.resource_id } else { title })
                } else if checked.is_some() {
                    format!("CI/activity on {}", if title.is_empty() { &node.resource_id } else { title })
                } else {
                    format!("Active on {}", if title.is_empty() { &node.resource_id } else { title })
                }
            } else if title.is_empty() {
                format!("{} {} ({lifecycle})", kind.to_uppercase(), node.resource_id)
            } else if matches!(lifecycle, "MERGED" | "CLOSED" | "DONE") {
                format!("{lifecycle} {kind}: {title}")
            } else {
                format!("Open {kind}: {title}")
            };

            ranked.push((
                activity_ts,
                LedgerItem {
                    kind: kind.into(),
                    resource_id: if node.resource_id.is_empty() {
                        node.node_id.clone()
                    } else {
                        node.resource_id.clone()
                    },
                    node_id: node.node_id.clone(),
                    summary,
                    confidence: conf,
                    evidence_refs: evidence,
                },
            ));
        }
        // Newest activity first; cap commit noise at 5
        ranked.sort_by(|a, b| b.0.cmp(&a.0));
        let mut commit_count = 0usize;
        let mut items = Vec::new();
        for (_ts, it) in ranked {
            if it.kind == "commit" {
                commit_count += 1;
                if commit_count > 5 {
                    continue;
                }
            }
            items.push(it);
        }

        // Open blockers from ACL-visible BLOCKS edges — exclude demo/seed theater
        // (story-1 graph_story BLOCKS must not write "blocked" on real digests).
        for edge in view.blockers.iter().chain(
            view.edges
                .iter()
                .filter(|e| {
                    e.edge_type.eq_ignore_ascii_case("BLOCKS")
                        || e.edge_type.eq_ignore_ascii_case("BLOCKED_BY")
                }),
        ) {
            if edge_is_demo_seed(edge, &view.nodes) {
                continue;
            }
            let target = if edge.edge_type.eq_ignore_ascii_case("BLOCKED_BY") {
                edge.from_node_id.clone()
            } else {
                edge.to_node_id.clone()
            };
            // Also skip if either endpoint node is a known seed PR / demo intent
            if node_id_is_demo_seed(&target)
                || node_id_is_demo_seed(&edge.from_node_id)
                || node_id_is_demo_seed(&edge.to_node_id)
            {
                continue;
            }
            let summary = edge
                .properties
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Blocked via {}", edge.edge_type));
            open_blockers.push(BlockerItem {
                node_id: target,
                summary,
                confidence: ConfidenceTier::Blocker,
                evidence_refs: vec![
                    format!("edge:{}", edge.edge_id),
                    format!("event:{}", edge.event_id),
                ]
                .into_iter()
                .filter(|s| !s.ends_with(':'))
                .collect(),
            });
        }
        // Dedupe blockers by node_id
        open_blockers.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        open_blockers.dedup_by(|a, b| a.node_id == b.node_id);

        // Human-facing period = rolling activity lookback; ledger_id stays on aligned bucket.
        let mut ledger = StatusLedger {
            tenant_id: twin.tenant_id.clone(),
            person_id: twin.subject_id.clone(),
            period: LedgerPeriod {
                start: opts.activity_start,
                end: opts.activity_end,
            },
            confidence_rollup: ConfidenceTier::Medium,
            items,
            open_blockers,
            graph_as_of,
            compiled_at,
        };
        apply_rollup(&mut ledger);

        let ledger_id = ledger_id_for(
            &twin.tenant_id,
            &twin.twin_id,
            opts.period_start,
            opts.period_end,
        );
        let snap = LedgerSnapshot {
            tenant_id: twin.tenant_id.clone(),
            ledger_id: ledger_id.clone(),
            twin_id: twin.twin_id.clone(),
            period_start: opts.period_start,
            period_end: opts.period_end,
            confidence_rollup: ledger.confidence_rollup,
            ledger: ledger.clone(),
            graph_as_of,
            compiled_at,
        };

        self.store.put_ledger(snap.clone()).await?;

        let draft_text = render_draft_text(&ledger);

        let finished = Utc::now();
        self.store
            .put_compile_run(CompileRun {
                tenant_id: twin.tenant_id.clone(),
                run_id: run_id.clone(),
                twin_id: twin.twin_id.clone(),
                status: if acl_empty { "ok".into() } else { "ok".into() },
                error_text: String::new(),
                started_at: started,
                finished_at: Some(finished),
            })
            .await?;

        Ok(CompileOutcome {
            run_id,
            ledger: snap,
            draft_text,
            acl_empty,
        })
    }
}

/// Build a standard synthetic fixture for tests (TC-T01).
pub fn synthetic_alice_graph(tenant: &str) -> GraphView {
    fixtures::alice_open_pr_fixture(tenant)
}

fn merge_graph_view(into: &mut GraphView, part: GraphView) {
    for n in part.nodes {
        if !into.nodes.iter().any(|x| x.node_id == n.node_id) {
            into.nodes.push(n);
        }
    }
    for e in part.edges {
        if !into.edges.iter().any(|x| x.edge_id == e.edge_id) {
            into.edges.push(e);
        }
    }
    for s in part.states {
        if !into
            .states
            .iter()
            .any(|x| x.node_id == s.node_id && x.state_key == s.state_key)
        {
            into.states.push(s);
        }
    }
    for b in part.blockers {
        if !into.blockers.iter().any(|x| x.edge_id == b.edge_id) {
            into.blockers.push(b);
        }
    }
    if into.graph_as_of.is_none() {
        into.graph_as_of = part.graph_as_of;
    }
}

/// True when node id is known demo/seed theater (story-1, intent_demo, gu_demo).
fn node_id_is_demo_seed(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("/pr/story-1")
        || l.contains("pr:neeljoshi18/ai-manager/pr/story-1")
        || l.contains("gu_demo_")
        || l.contains("demo-repo")
        || l.contains("intent_demo")
        || l.contains(":story-1")
}

/// Full GraphNodeView check: id path + properties.seed / is_demo.
fn node_view_is_demo_seed(n: &GraphNodeView) -> bool {
    if node_id_is_demo_seed(&n.node_id) || node_id_is_demo_seed(&n.resource_id) {
        return true;
    }
    if n.properties.get("is_demo").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    if let Some(seed) = n.properties.get("seed").and_then(|v| v.as_str()) {
        let s = seed.to_ascii_lowercase();
        if s.contains("graph_story") || s.contains("intent_demo") || s.contains("demo") {
            return true;
        }
    }
    // Display name from seed story intents recycled as PR titles
    let lab = n.display_name.to_ascii_lowercase();
    if lab.contains("hold merge until demo")
        || lab.contains("ready for pilot") && n.node_id.contains("story-1")
        || lab.contains("waiting on partner review") && n.node_id.contains("story-1")
    {
        return true;
    }
    false
}

/// Drop BLOCKS edges that come from seed graph_story / intent_demo theater.
fn edge_is_demo_seed(edge: &GraphEdgeView, nodes: &[GraphNodeView]) -> bool {
    let eid = edge.event_id.to_ascii_lowercase();
    if eid.contains("story:blocks")
        || eid.contains("seed:")
        || eid.contains("intent_demo")
        || eid.contains("graph_story")
        || eid.starts_with("seed")
    {
        return true;
    }
    let props = &edge.properties;
    if props.get("is_demo").and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    if let Some(seed) = props.get("seed").and_then(|v| v.as_str()) {
        let s = seed.to_ascii_lowercase();
        if s.contains("graph_story") || s.contains("intent_demo") || s.contains("demo") {
            return true;
        }
    }
    // Endpoint nodes tagged seed/demo
    for nid in [&edge.from_node_id, &edge.to_node_id] {
        if node_id_is_demo_seed(nid) {
            return true;
        }
        if let Some(n) = nodes.iter().find(|n| &n.node_id == nid) {
            if n.properties.get("is_demo").and_then(|v| v.as_bool()) == Some(true) {
                return true;
            }
            if let Some(seed) = n.properties.get("seed").and_then(|v| v.as_str()) {
                let s = seed.to_ascii_lowercase();
                if s.contains("graph_story") || s.contains("intent_demo") || s == "seed" {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use twin_core::ids::person_twin_id;
    use twin_core::store::InMemoryTwinStore;

    #[test]
    fn seed_blocks_edges_are_filtered() {
        assert!(node_id_is_demo_seed(
            "pr:neeljoshi18/AI-Manager/pr/story-1"
        ));
        assert!(!node_id_is_demo_seed("pr:neeljoshi18/AI-Manager/pr/42"));
        let edge = GraphEdgeView {
            edge_id: "e1".into(),
            edge_type: "BLOCKS".into(),
            from_node_id: "pr:neeljoshi18/AI-Manager/pr/story-1".into(),
            to_node_id: "person:gu_x".into(),
            event_id: "event:story:blocks1".into(),
            properties: serde_json::json!({"seed": "graph_story"}),
            is_private: false,
            valid_from: None,
        };
        assert!(edge_is_demo_seed(&edge, &[]));
        let live = GraphEdgeView {
            edge_id: "e2".into(),
            edge_type: "BLOCKS".into(),
            from_node_id: "pr:neeljoshi18/AI-Manager/pr/99".into(),
            to_node_id: "pr:other/repo/pr/1".into(),
            event_id: "poll:pr:neeljoshi18/AI-Manager:99:2026-08-10".into(),
            properties: serde_json::json!({}),
            is_private: false,
            valid_from: None,
        };
        assert!(!edge_is_demo_seed(&live, &[]));
    }

    #[tokio::test]
    async fn compile_synthetic_items() {
        let store = InMemoryTwinStore::new();
        let source = FixtureGraphSource::new(synthetic_alice_graph("ten_t"));
        let compiler = LedgerCompiler::new(store.clone(), source);
        let now = Utc::now();
        let twin = Twin {
            tenant_id: "ten_t".into(),
            twin_id: person_twin_id("gu_alice"),
            twin_kind: TwinKind::Person,
            subject_id: "gu_alice".into(),
            display_name: "Alice".into(),
            timezone: "UTC".into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        };
        store.upsert_twin(twin.clone()).await.unwrap();
        let out = compiler
            .compile_person(
                &twin,
                &CompileOpts::window(now - chrono::Duration::days(1), now, 2),
            )
            .await
            .unwrap();
        assert!(!out.ledger.ledger.items.is_empty());
        assert!(out.ledger.ledger.items[0]
            .evidence_refs
            .iter()
            .any(|e| e.starts_with("event:") || e.starts_with("edge:")));
    }

    #[tokio::test]
    async fn multi_alias_merges_activity() {
        let store = InMemoryTwinStore::new();
        let source = FixtureGraphSource::empty();
        // Primary subject has no edges; alias gu_old has commit activity
        let now = Utc::now();
        source.set_view(
            "ten_t",
            "gu_new",
            GraphView {
                nodes: vec![GraphNodeView {
                    node_id: "person:gu_new".into(),
                    node_type: "Person".into(),
                    display_name: "Neel".into(),
                    resource_id: "gu_new".into(),
                    properties: serde_json::json!({}),
                    is_private: false,
                }],
                edges: vec![],
                states: vec![],
                blockers: vec![],
                graph_as_of: Some(now),
            },
        );
        source.set_view(
            "ten_t",
            "gu_old",
            GraphView {
                nodes: vec![
                    GraphNodeView {
                        node_id: "person:gu_old".into(),
                        node_type: "Person".into(),
                        display_name: "Neel".into(),
                        resource_id: "gu_old".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                    },
                    GraphNodeView {
                        node_id: "commit:org/r:abc".into(),
                        node_type: "Commit".into(),
                        display_name: "digest windows".into(),
                        resource_id: "abc1234".into(),
                        properties: serde_json::json!({"message": "digest windows"}),
                        is_private: false,
                    },
                    GraphNodeView {
                        node_id: "repo:org/r".into(),
                        node_type: "Repo".into(),
                        display_name: "org/r".into(),
                        resource_id: "org/r".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                    },
                ],
                edges: vec![
                    GraphEdgeView {
                        edge_id: "a1".into(),
                        edge_type: "AUTHORED".into(),
                        from_node_id: "person:gu_old".into(),
                        to_node_id: "commit:org/r:abc".into(),
                        event_id: "evt_c".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                        valid_from: Some(now - chrono::Duration::hours(2)),
                    },
                    GraphEdgeView {
                        edge_id: "p1".into(),
                        edge_type: "PUSHED_TO".into(),
                        from_node_id: "person:gu_old".into(),
                        to_node_id: "repo:org/r".into(),
                        event_id: "evt_p".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                        valid_from: Some(now - chrono::Duration::hours(2)),
                    },
                ],
                states: vec![],
                blockers: vec![],
                graph_as_of: Some(now),
            },
        );
        let compiler = LedgerCompiler::new(store.clone(), source);
        let twin = Twin {
            tenant_id: "ten_t".into(),
            twin_id: person_twin_id("gu_new"),
            twin_kind: TwinKind::Person,
            subject_id: "gu_new".into(),
            display_name: "Neel".into(),
            timezone: "UTC".into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({"provider_aliases": ["neel", "gu_old"]}),
            created_at: now,
            updated_at: now,
        };
        store.upsert_twin(twin.clone()).await.unwrap();
        let out = compiler
            .compile_person(&twin, &CompileOpts::window(now - chrono::Duration::hours(24), now, 3))
            .await
            .unwrap();
        assert!(
            out.ledger.ledger.items.iter().any(|i| i.kind == "commit"),
            "expected commit from gu_old alias: {:?}",
            out.ledger.ledger.items
        );
        assert!(out.ledger.ledger.items.iter().any(|i| i.kind == "repo"));
        assert!(out.ledger.ledger.items[0]
            .evidence_refs
            .iter()
            .any(|e| e.starts_with("at:")));
    }

    #[tokio::test]
    async fn window_excludes_old_commits_keeps_open_pr() {
        let store = InMemoryTwinStore::new();
        let source = FixtureGraphSource::empty();
        let now = Utc::now();
        let old = now - chrono::Duration::hours(48);
        source.set_view(
            "ten_t",
            "gu_a",
            GraphView {
                nodes: vec![
                    GraphNodeView {
                        node_id: "person:gu_a".into(),
                        node_type: "Person".into(),
                        display_name: "A".into(),
                        resource_id: "gu_a".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                    },
                    GraphNodeView {
                        node_id: "commit:org/r:old".into(),
                        node_type: "Commit".into(),
                        display_name: "old work".into(),
                        resource_id: "oldsha".into(),
                        properties: serde_json::json!({"message": "old work"}),
                        is_private: false,
                    },
                    GraphNodeView {
                        node_id: "pr:org/r/pr/1".into(),
                        node_type: "PullRequest".into(),
                        display_name: "still open".into(),
                        resource_id: "org/r/pr/1".into(),
                        properties: serde_json::json!({"title": "still open"}),
                        is_private: false,
                    },
                ],
                edges: vec![
                    GraphEdgeView {
                        edge_id: "c_old".into(),
                        edge_type: "AUTHORED".into(),
                        from_node_id: "person:gu_a".into(),
                        to_node_id: "commit:org/r:old".into(),
                        event_id: "e_old".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                        valid_from: Some(old),
                    },
                    GraphEdgeView {
                        edge_id: "pr_old".into(),
                        edge_type: "AUTHORED".into(),
                        from_node_id: "person:gu_a".into(),
                        to_node_id: "pr:org/r/pr/1".into(),
                        event_id: "e_pr".into(),
                        properties: serde_json::json!({}),
                        is_private: false,
                        valid_from: Some(old),
                    },
                ],
                states: vec![EntityStateView {
                    node_id: "pr:org/r/pr/1".into(),
                    state_key: "lifecycle".into(),
                    state_value: "OPEN".into(),
                    event_id: "e_pr".into(),
                    as_of: old,
                }],
                blockers: vec![],
                graph_as_of: Some(now),
            },
        );
        let compiler = LedgerCompiler::new(store.clone(), source);
        let twin = Twin {
            tenant_id: "ten_t".into(),
            twin_id: person_twin_id("gu_a"),
            twin_kind: TwinKind::Person,
            subject_id: "gu_a".into(),
            display_name: "A".into(),
            timezone: "UTC".into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        };
        store.upsert_twin(twin.clone()).await.unwrap();
        let out = compiler
            .compile_person(&twin, &CompileOpts::window(now - chrono::Duration::hours(24), now, 2))
            .await
            .unwrap();
        assert!(
            !out.ledger.ledger.items.iter().any(|i| i.kind == "commit"),
            "old commit should be outside 24h lookback"
        );
        assert!(
            out.ledger.ledger.items.iter().any(|i| i.kind == "pr"),
            "open PR stays in digest: {:?}",
            out.ledger.ledger.items
        );
    }

    #[tokio::test]
    async fn dual_person_distinct_digests() {
        let store = InMemoryTwinStore::new();
        let source = FixtureGraphSource::empty();
        let now = Utc::now();
        for (sid, name, pr) in [
            ("gu_p1", "Alice", "pr:org/r/pr/10"),
            ("gu_p2", "Bob", "pr:org/r/pr/20"),
        ] {
            source.set_view(
                "ten_t",
                sid,
                GraphView {
                    nodes: vec![
                        GraphNodeView {
                            node_id: format!("person:{sid}"),
                            node_type: "Person".into(),
                            display_name: name.into(),
                            resource_id: sid.into(),
                            properties: serde_json::json!({}),
                            is_private: false,
                        },
                        GraphNodeView {
                            node_id: pr.into(),
                            node_type: "PullRequest".into(),
                            display_name: format!("{name} work"),
                            resource_id: pr.trim_start_matches("pr:").into(),
                            properties: serde_json::json!({"title": format!("{name} work")}),
                            is_private: false,
                        },
                    ],
                    edges: vec![GraphEdgeView {
                        edge_id: format!("a_{sid}"),
                        edge_type: "AUTHORED".into(),
                        from_node_id: format!("person:{sid}"),
                        to_node_id: pr.into(),
                        event_id: format!("e_{sid}"),
                        properties: serde_json::json!({}),
                        is_private: false,
                        valid_from: Some(now - chrono::Duration::hours(1)),
                    }],
                    states: vec![EntityStateView {
                        node_id: pr.into(),
                        state_key: "lifecycle".into(),
                        state_value: "OPEN".into(),
                        event_id: format!("e_{sid}"),
                        as_of: now,
                    }],
                    blockers: vec![],
                    graph_as_of: Some(now),
                },
            );
        }
        let compiler = LedgerCompiler::new(store.clone(), source);
        let opts = CompileOpts::window(now - chrono::Duration::hours(24), now, 2);
        let mut digests = Vec::new();
        for (sid, name) in [("gu_p1", "Alice"), ("gu_p2", "Bob")] {
            let twin = Twin {
                tenant_id: "ten_t".into(),
                twin_id: person_twin_id(sid),
                twin_kind: TwinKind::Person,
                subject_id: sid.into(),
                display_name: name.into(),
                timezone: "UTC".into(),
                channel_id: "C1".into(),
                shadow_until: None,
                high_auto_publish: false,
                enabled: true,
                config_json: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            };
            store.upsert_twin(twin.clone()).await.unwrap();
            let out = compiler.compile_person(&twin, &opts).await.unwrap();
            assert_eq!(out.ledger.ledger.items.len(), 1);
            digests.push(out.ledger.ledger.items[0].node_id.clone());
        }
        assert_ne!(digests[0], digests[1], "each person keeps their own PR");
    }
}
