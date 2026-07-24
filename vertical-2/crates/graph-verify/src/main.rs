//! Vertical 2 verification battery TC-G01 … TC-G10 (embedded).

use chrono::{DateTime, Utc};
use clap::Parser;
use graph_core::ids::{person_node_id, pr_node_id, repo_node_id};
use graph_core::membership::{InMemoryMembership, MembershipStore};
use graph_core::model::{ProjectStatus, QueryContext};
use graph_core::project::ProjectEngine;
use graph_core::store::{GraphStore, InMemoryGraphStore};
use graph_core::v1_event::{V1Actor, V1Acl, V1AclRevocation, V1CanonicalEvent};
use serde_json::json;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
struct Args {}

struct TestResult {
    id: &'static str,
    name: &'static str,
    passed: bool,
    detail: String,
}

fn evt(
    id: &str,
    etype: &str,
    ts: &str,
    private: bool,
    groups: &[&str],
    resource: &str,
    parent: &str,
    attrs: serde_json::Value,
) -> V1CanonicalEvent {
    V1CanonicalEvent {
        event_id: id.into(),
        tenant_id: "ten_g".into(),
        provider: "github".into(),
        category: "code".into(),
        event_type: etype.into(),
        event_timestamp: DateTime::parse_from_rfc3339(ts)
            .unwrap()
            .with_timezone(&Utc),
        ingested_at: Utc::now(),
        actor: V1Actor {
            global_user_id: "gu_alice".into(),
            provider_user_id: "42".into(),
            email: "a@x.com".into(),
            display_name: "Alice".into(),
        },
        acl: V1Acl {
            tenant_id: "ten_g".into(),
            allowed_group_ids: groups.iter().map(|s| s.to_string()).collect(),
            is_private: private,
            acl_version: 1,
        },
        resource_id: resource.into(),
        parent_resource_id: parent.into(),
        attributes: attrs,
        raw_payload_s3_uri: String::new(),
        event_sequence_number: 1,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    let _ = Args::parse();
    info!("Vertical 2 graph verification battery");

    let mut results = Vec::new();
    results.push(tc_g01().await);
    results.push(tc_g02().await);
    results.push(tc_g03().await);
    results.push(tc_g04().await);
    results.push(tc_g05().await);
    results.push(tc_g06().await);
    results.push(tc_g07().await);
    results.push(tc_g08().await);
    results.push(tc_g09().await);
    results.push(tc_g10().await);

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("           VERTICAL 2 GRAPH VERIFICATION BATTERY");
    println!("═══════════════════════════════════════════════════════════════════");
    let mut all = true;
    for r in &results {
        let m = if r.passed { "PASS" } else { "FAIL" };
        if !r.passed {
            all = false;
        }
        println!("  [{m}]  {}  —  {}  |  {}", r.id, r.name, r.detail);
    }
    println!("═══════════════════════════════════════════════════════════════════");
    if all {
        println!("  RESULT: ALL CHECKS PASSED — Vertical 2 ready for manual testing");
        Ok(())
    } else {
        std::process::exit(1);
    }
}

async fn tc_g01() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    let o = eng
        .project_event(&evt(
            "g01",
            "pull_request.opened",
            "2026-01-01T00:00:00Z",
            true,
            &["grp_eng"],
            "acme/app/pr/7",
            "acme/app",
            json!({"title": "Feat"}),
        ))
        .await
        .unwrap();
    let nodes = store.count_nodes("ten_g").await.unwrap();
    let edges = store.count_edges("ten_g").await.unwrap();
    let passed = o.status == ProjectStatus::Applied && nodes >= 3 && edges >= 2;
    TestResult {
        id: "TC-G01",
        name: "Projection PR opened",
        passed,
        detail: format!("status={:?} nodes={nodes} edges={edges}", o.status),
    }
}

async fn tc_g02() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    eng.project_event(&evt(
        "g02c",
        "pull_request.closed",
        "2026-01-01T00:00:05Z",
        false,
        &[],
        "acme/app/pr/99",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    eng.project_event(&evt(
        "g02o",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/99",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    let ctx = QueryContext {
        tenant_id: "ten_g".into(),
        global_user_id: "gu_alice".into(),
        group_ids: vec![],
    };
    let st = store
        .get_state(&ctx, &pr_node_id("acme/app/pr/99"), "lifecycle")
        .await
        .unwrap();
    let passed = st.as_ref().map(|s| s.state_value.as_str()) == Some("CLOSED");
    TestResult {
        id: "TC-G02",
        name: "Temporal out-of-order state",
        passed,
        detail: format!("state={st:?}"),
    }
}

async fn tc_g03() -> TestResult {
    let store = InMemoryGraphStore::new();
    let mem = InMemoryMembership::new();
    let eng = ProjectEngine::new(store.clone(), mem.clone());
    eng.project_event(&evt(
        "g03",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        true,
        &["grp_eng"],
        "acme/app/pr/7",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    mem.set_groups("ten_g", "gu_bob", &["grp_sales".into()])
        .await
        .unwrap();
    let bob = QueryContext {
        tenant_id: "ten_g".into(),
        global_user_id: "gu_bob".into(),
        group_ids: mem.get_groups("ten_g", "gu_bob").await.unwrap(),
    };
    let n = store
        .get_node(&bob, &pr_node_id("acme/app/pr/7"))
        .await
        .unwrap();
    let passed = n.is_none();
    TestResult {
        id: "TC-G03",
        name: "ACL private PR hidden",
        passed,
        detail: format!("bob_sees={:?}", n.map(|x| x.node_id)),
    }
}

async fn tc_g04() -> TestResult {
    let store = InMemoryGraphStore::new();
    let mem = InMemoryMembership::new();
    let eng = ProjectEngine::new(store.clone(), mem.clone());
    eng.project_event(&evt(
        "g04",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        true,
        &["grp_eng"],
        "acme/app/pr/7",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    mem.set_groups("ten_g", "gu_alice", &["grp_eng".into()])
        .await
        .unwrap();
    let mut ctx = QueryContext {
        tenant_id: "ten_g".into(),
        global_user_id: "gu_alice".into(),
        group_ids: mem.get_groups("ten_g", "gu_alice").await.unwrap(),
    };
    let before = store
        .get_node(&ctx, &pr_node_id("acme/app/pr/7"))
        .await
        .unwrap();
    mem.remove_group("ten_g", "gu_alice", "grp_eng")
        .await
        .unwrap();
    // Also via ACL revocation path
    eng.project_acl_revocation(&V1AclRevocation {
        event_id: "g04-rev".into(),
        tenant_id: "ten_g".into(),
        global_user_id: "gu_alice".into(),
        provider_user_id: "42".into(),
        provider: "github".into(),
        group_id: "grp_eng".into(),
        change_type: "removed_from_group".into(),
        acl_version: 2,
        timestamp: Utc::now(),
    })
    .await
    .unwrap();
    ctx.group_ids = mem.get_groups("ten_g", "gu_alice").await.unwrap();
    let after = store
        .get_node(&ctx, &pr_node_id("acme/app/pr/7"))
        .await
        .unwrap();
    let passed = before.is_some() && after.is_none() && ctx.group_ids.is_empty();
    TestResult {
        id: "TC-G04",
        name: "ACL revoke hides private PR",
        passed,
        detail: format!("before={} after={} groups={:?}", before.is_some(), after.is_some(), ctx.group_ids),
    }
}

async fn tc_g05() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    let e = evt(
        "g05",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/1",
        "acme/app",
        json!({}),
    );
    let mut dups = 0;
    for _ in 0..1000 {
        let o = eng.project_event(&e).await.unwrap();
        if o.status == ProjectStatus::Duplicate {
            dups += 1;
        }
    }
    let edges = store.count_edges("ten_g").await.unwrap();
    // AUTHORED + BELONGS_TO + CLAIMS + ABOUT (intent v0) once after 1000 replays
    let passed = dups == 999 && edges == 4;
    TestResult {
        id: "TC-G05",
        name: "Idempotent event replay",
        passed,
        detail: format!("duplicates={dups} edges={edges}"),
    }
}

async fn tc_g06() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    eng.project_event(&evt(
        "g06",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/7",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    let ctx = QueryContext {
        tenant_id: "ten_g".into(),
        global_user_id: "gu_alice".into(),
        group_ids: vec![],
    };
    let path = store
        .path(
            &ctx,
            &person_node_id("gu_alice"),
            &repo_node_id("acme/app"),
            3,
        )
        .await
        .unwrap();
    let passed = path.as_ref().map(|p| p.nodes.len()) == Some(3);
    TestResult {
        id: "TC-G06",
        name: "Multi-hop Person→PR→Repo",
        passed,
        detail: format!("path_nodes={:?}", path.map(|p| p.nodes.len())),
    }
}

async fn tc_g07() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    eng.project_event(&evt(
        "g07a",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/1",
        "acme/app",
        json!({"blocks": ["acme/app/pr/2"]}),
    ))
    .await
    .unwrap();
    // ensure target PR node exists for cleaner graph
    eng.project_event(&evt(
        "g07b",
        "pull_request.opened",
        "2026-01-01T00:00:01Z",
        false,
        &[],
        "acme/app/pr/2",
        "acme/app",
        json!({}),
    ))
    .await
    .unwrap();
    let ctx = QueryContext {
        tenant_id: "ten_g".into(),
        global_user_id: "gu_alice".into(),
        group_ids: vec![],
    };
    let b = store
        .blockers(&ctx, &pr_node_id("acme/app/pr/1"))
        .await
        .unwrap();
    let passed = b.iter().any(|e| e.edge_type == "BLOCKS");
    TestResult {
        id: "TC-G07",
        name: "Blockers listing",
        passed,
        detail: format!("blocker_edges={}", b.len()),
    }
}

async fn tc_g08() -> TestResult {
    // Chaos analogue: apply half events, "restart" with new engine same store, finish
    let store = InMemoryGraphStore::new();
    let mem = InMemoryMembership::new();
    let eng1 = ProjectEngine::new(store.clone(), mem.clone());
    for i in 0..50 {
        eng1.project_event(&evt(
            &format!("g08-{i}"),
            "pull_request.opened",
            "2026-01-01T00:00:00Z",
            false,
            &[],
            &format!("acme/app/pr/{i}"),
            "acme/app",
            json!({}),
        ))
        .await
        .unwrap();
    }
    let eng2 = ProjectEngine::new(store.clone(), mem);
    for i in 50..100 {
        eng2.project_event(&evt(
            &format!("g08-{i}"),
            "pull_request.opened",
            "2026-01-01T00:00:00Z",
            false,
            &[],
            &format!("acme/app/pr/{i}"),
            "acme/app",
            json!({}),
        ))
        .await
        .unwrap();
    }
    // replay first 50 as duplicates
    let mut dups = 0;
    for i in 0..50 {
        let o = eng2
            .project_event(&evt(
                &format!("g08-{i}"),
                "pull_request.opened",
                "2026-01-01T00:00:00Z",
                false,
                &[],
                &format!("acme/app/pr/{i}"),
                "acme/app",
                json!({}),
            ))
            .await
            .unwrap();
        if o.status == ProjectStatus::Duplicate {
            dups += 1;
        }
    }
    let applied_ok = store.event_applied("ten_g", "g08-99").await.unwrap();
    let passed = dups == 50 && applied_ok;
    TestResult {
        id: "TC-G08",
        name: "Restart / resume idempotency",
        passed,
        detail: format!("replay_dups={dups} last_applied={applied_ok}"),
    }
}

async fn tc_g09() -> TestResult {
    // Backfill-style bulk historical timestamps
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    for i in 0..20 {
        eng.project_event(&evt(
            &format!("g09-{i}"),
            "pull_request.opened",
            &format!("2025-06-{:02}T12:00:00Z", (i % 28) + 1),
            false,
            &[],
            &format!("acme/app/pr/{i}"),
            "acme/app",
            json!({}),
        ))
        .await
        .unwrap();
    }
    let n = store.count_nodes("ten_g").await.unwrap();
    let passed = n >= 20;
    TestResult {
        id: "TC-G09",
        name: "Historical backfill batch",
        passed,
        detail: format!("nodes={n}"),
    }
}

async fn tc_g10() -> TestResult {
    let store = InMemoryGraphStore::new();
    let eng = ProjectEngine::new(store.clone(), InMemoryMembership::new());
    let mut a = evt(
        "g10a",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/1",
        "acme/app",
        json!({}),
    );
    a.tenant_id = "ten_a".into();
    let mut b = evt(
        "g10b",
        "pull_request.opened",
        "2026-01-01T00:00:00Z",
        false,
        &[],
        "acme/app/pr/1",
        "acme/app",
        json!({}),
    );
    b.tenant_id = "ten_b".into();
    eng.project_event(&a).await.unwrap();
    eng.project_event(&b).await.unwrap();
    let ctx_a = QueryContext {
        tenant_id: "ten_a".into(),
        global_user_id: "gu_alice".into(),
        group_ids: vec![],
    };
    let leak = store
        .get_node(&ctx_a, &pr_node_id("acme/app/pr/1"))
        .await
        .unwrap();
    // tenant A sees its own
    let own_ok = leak.as_ref().map(|n| n.tenant_id.as_str()) == Some("ten_a");
    // tenant B count separate
    let ca = store.count_nodes("ten_a").await.unwrap();
    let cb = store.count_nodes("ten_b").await.unwrap();
    let passed = own_ok && ca >= 3 && cb >= 3;
    TestResult {
        id: "TC-G10",
        name: "Tenant isolation",
        passed,
        detail: format!("ten_a_nodes={ca} ten_b_nodes={cb} own_ok={own_ok}"),
    }
}
