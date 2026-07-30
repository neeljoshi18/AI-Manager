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
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub hops: usize,
}

impl Default for CompileOpts {
    fn default() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(24);
        Self {
            period_start: start,
            period_end: end,
            hops: 3,
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
        let person_node = format!("person:{}", twin.subject_id);

        let view = self
            .source
            .fetch_person_view(
                &twin.tenant_id,
                &twin.subject_id,
                &person_node,
                opts.hops,
            )
            .await?;

        let graph_as_of = view.graph_as_of.unwrap_or_else(Utc::now);
        let compiled_at = Utc::now();
        let acl_empty = view.nodes.is_empty() && view.edges.is_empty() && view.blockers.is_empty();

        let mut items = Vec::new();
        let mut open_blockers = Vec::new();

        // Map PR / Issue / Commit / Repo activity with evidence from person-linked edges
        for node in &view.nodes {
            if node.node_id == person_node {
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
            let authored = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("AUTHORED")
                    && (e.to_node_id == node.node_id || e.from_node_id == node.node_id)
                    && (e.from_node_id == person_node || e.to_node_id == person_node)
            });
            let assigned = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("ASSIGNED_TO")
                    && (e.to_node_id == person_node || e.from_node_id == person_node)
                    && (e.to_node_id == node.node_id || e.from_node_id == node.node_id)
            });
            let pushed = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("PUSHED_TO")
                    && e.from_node_id == person_node
                    && e.to_node_id == node.node_id
            });
            let checked = view.edges.iter().find(|e| {
                e.edge_type.eq_ignore_ascii_case("CHECKED")
                    && e.from_node_id == person_node
                    && e.to_node_id == node.node_id
            });
            let linked = view.edges.iter().any(|e| {
                (e.from_node_id == person_node && e.to_node_id == node.node_id)
                    || (e.to_node_id == person_node && e.from_node_id == node.node_id)
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
            let conf = if is_commit || pushed.is_some() {
                // Commits/pushes are medium unless we have merge evidence
                score_item_confidence(lifecycle, true)
            } else {
                score_item_confidence(lifecycle, true)
            };

            let mut evidence = Vec::new();
            for e in [authored, assigned, pushed, checked].into_iter().flatten() {
                evidence.push(format!("edge:{}", e.edge_id));
                if !e.event_id.is_empty() {
                    evidence.push(format!("event:{}", e.event_id));
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

            items.push(LedgerItem {
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
            });
        }
        // Cap commit noise: keep at most 5 commit items (most recent-ish order from graph)
        let mut commit_count = 0usize;
        items.retain(|it| {
            if it.kind == "commit" {
                commit_count += 1;
                commit_count <= 5
            } else {
                true
            }
        });

        // Open blockers from ACL-visible BLOCKS edges
        for edge in view.blockers.iter().chain(
            view.edges
                .iter()
                .filter(|e| {
                    e.edge_type.eq_ignore_ascii_case("BLOCKS")
                        || e.edge_type.eq_ignore_ascii_case("BLOCKED_BY")
                }),
        ) {
            let target = if edge.edge_type.eq_ignore_ascii_case("BLOCKED_BY") {
                edge.from_node_id.clone()
            } else {
                edge.to_node_id.clone()
            };
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

        let mut ledger = StatusLedger {
            tenant_id: twin.tenant_id.clone(),
            person_id: twin.subject_id.clone(),
            period: LedgerPeriod {
                start: opts.period_start,
                end: opts.period_end,
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

#[cfg(test)]
mod tests {
    use super::*;
    use twin_core::ids::person_twin_id;
    use twin_core::store::InMemoryTwinStore;

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
                &CompileOpts {
                    period_start: now - chrono::Duration::days(1),
                    period_end: now,
                    hops: 2,
                },
            )
            .await
            .unwrap();
        assert!(!out.ledger.ledger.items.is_empty());
        assert!(out.ledger.ledger.items[0]
            .evidence_refs
            .iter()
            .any(|e| e.starts_with("event:") || e.starts_with("edge:")));
    }
}
