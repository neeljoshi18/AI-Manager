//! Vertical 1 Verification Battery (Spec §5).
//!
//! Executes TC-01 through TC-06 against the embedded stack and reports pass/fail.
//!
//! | ID    | Category   | Target                                      |
//! |-------|------------|---------------------------------------------|
//! | TC-01 | Load       | Sustained ingest; P99 tracked (local scale) |
//! | TC-02 | Idempotency| 10k identical deliveries → 1 record         |
//! | TC-03 | ACL        | Revocation stops private data leakage       |
//! | TC-04 | Schema     | Mutations don't crash; DLQ on hard failure  |
//! | TC-05 | Chaos      | Bus still durable under concurrent load     |
//! | TC-06 | Out-of-order| CLOSED before OPENED → state CLOSED        |

use chrono::Utc;
use clap::Parser;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use telemetry_core::acl::seed_membership;
use telemetry_core::config::AppConfig;
use telemetry_core::model::{
    ActorIdentity, AclSnapshot, BusMessage, BusPayload, BusTopic, CanonicalEventRecord,
    EventQuery, IngestStatus, QueryContext, TenantConfig,
};
use telemetry_core::pipeline::{IngestHeaders, IngestRequest};
use telemetry_core::store::pr_state;
use telemetry_core::wiring::build_embedded;
use telemetry_proto::{EventCategory, SourceProvider};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "telemetry-verify", about = "Vertical 1 verification battery")]
struct Args {
    /// Concurrent workers for load/idempotency tests
    #[arg(long, default_value = "50")]
    workers: usize,
    /// Replay count for TC-02 (spec: 10_000)
    #[arg(long, default_value = "10000")]
    replay_count: usize,
    /// Load request count for TC-01
    #[arg(long, default_value = "5000")]
    load_count: usize,
    /// Fail if P99 exceeds this many ms (local embedded target; prod is 50ms with Redis)
    #[arg(long, default_value = "50")]
    p99_budget_ms: u64,
}

struct TestResult {
    id: &'static str,
    name: &'static str,
    passed: bool,
    detail: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args = Args::parse();
    info!("Vertical 1 verification battery starting");

    let mut results = Vec::new();
    results.push(tc02_idempotency(args.replay_count, args.workers).await);
    results.push(tc03_acl_revocation().await);
    results.push(tc04_schema_mutations().await);
    results.push(tc06_out_of_order().await);
    results.push(tc05_chaos_durability(args.load_count, args.workers).await);
    results.push(tc01_load(args.load_count, args.workers, args.p99_budget_ms).await);

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("           VERTICAL 1 VERIFICATION BATTERY DASHBOARD");
    println!("═══════════════════════════════════════════════════════════════════");
    let mut all_pass = true;
    for r in &results {
        let mark = if r.passed { "PASS" } else { "FAIL" };
        if !r.passed {
            all_pass = false;
        }
        println!("  [{mark}]  {}  —  {}  |  {}", r.id, r.name, r.detail);
    }
    println!("═══════════════════════════════════════════════════════════════════");
    if all_pass {
        println!("  RESULT: ALL CHECKS PASSED — Vertical 1 ready for manual testing");
        Ok(())
    } else {
        error!("one or more verification checks failed");
        std::process::exit(1);
    }
}

fn make_rt(skip_auth: bool) -> telemetry_core::wiring::Vertical1Runtime {
    let mut cfg = AppConfig::default();
    cfg.skip_auth = skip_auth;
    cfg.rate_limit_per_minute = 1_000_000; // don't trip rate limit during load tests
    build_embedded(cfg)
}

fn sample_pr_body(action: &str, number: u64, updated_at: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "action": action,
        "pull_request": {
            "number": number,
            "title": format!("PR {number}"),
            "state": if action == "closed" { "closed" } else { "open" },
            "draft": false,
            "merged": false,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": updated_at,
            "html_url": format!("https://github.com/acme/app/pull/{number}"),
            "user": { "id": 42, "login": "alice" },
            "base": { "ref": "main" },
            "head": { "ref": "feat" },
            "additions": 10,
            "deletions": 2,
            "changed_files": 1
        },
        "repository": { "full_name": "acme/app", "private": true },
        "sender": { "id": 42, "login": "alice", "email": "alice@acme.io" }
    }))
    .unwrap()
}

async fn seed_tenant(rt: &telemetry_core::wiring::Vertical1Runtime, tenant_id: &str, secret: &str) {
    rt.tenants
        .upsert(TenantConfig {
            tenant_id: tenant_id.into(),
            github_webhook_secret: Some(secret.into()),
            gitlab_webhook_secret: None,
            jira_webhook_secret: None,
            linear_webhook_secret: None,
            slack_signing_secret: None,
            teams_webhook_secret: None,
            zendesk_webhook_secret: None,
            default_group_ids: vec!["grp_eng_core".into()],
        })
        .await
        .unwrap();
}

/// TC-02: Replay attack — N identical deliveries → exactly 1 analytical record.
async fn tc02_idempotency(replay_count: usize, workers: usize) -> TestResult {
    let rt = Arc::new(make_rt(true));
    let tenant = "ten_idem";
    seed_tenant(&rt, tenant, "secret").await;

    let delivery_id = "delivery-fixed-idempotency-001";
    let body = sample_pr_body("opened", 1, "2026-01-01T00:00:00Z");
    let accepted = Arc::new(AtomicU64::new(0));
    let duplicates = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::new();
    let per_worker = replay_count / workers;
    let remainder = replay_count % workers;

    for w in 0..workers {
        let rt = rt.clone();
        let body = body.clone();
        let accepted = accepted.clone();
        let duplicates = duplicates.clone();
        let n = per_worker + if w < remainder { 1 } else { 0 };
        handles.push(tokio::spawn(async move {
            for _ in 0..n {
                let req = IngestRequest {
                    tenant_id: tenant.into(),
                    provider: SourceProvider::Github,
                    body: body.clone(),
                    headers: IngestHeaders {
                        delivery_id: Some(delivery_id.into()),
                        event_name: Some("pull_request".into()),
                        ..Default::default()
                    },
                    is_backfill: false,
                };
                match rt.pipeline.ingest(req).await {
                    Ok(o) if o.status == IngestStatus::Accepted => {
                        accepted.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(o) if o.status == IngestStatus::Duplicate => {
                        duplicates.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let unique = rt.store.count_unique(tenant).await.unwrap();
    let acc = accepted.load(Ordering::Relaxed);
    let dup = duplicates.load(Ordering::Relaxed);
    let passed = unique == 1 && acc == 1 && (acc + dup) as usize == replay_count;

    TestResult {
        id: "TC-02",
        name: "Idempotency (replay attack)",
        passed,
        detail: format!(
            "unique_records={unique} accepted={acc} duplicates={dup} total={replay_count}"
        ),
    }
}

/// TC-03: ACL revocation — private events invisible after group removal.
async fn tc03_acl_revocation() -> TestResult {
    let rt = make_rt(true);
    let tenant = "ten_acl";
    seed_tenant(&rt, tenant, "secret").await;

    let uid = seed_membership(
        rt.acl.as_ref(),
        tenant,
        "gh_42",
        "alice@acme.io",
        "Alice",
        &["grp_eng_core", "grp_sec_lead"],
    )
    .await
    .unwrap();

    // Ingest private PR
    let body = sample_pr_body("opened", 7, "2026-01-01T00:00:00Z");
    let outcome = rt
        .pipeline
        .ingest(IngestRequest {
            tenant_id: tenant.into(),
            provider: SourceProvider::Github,
            body,
            headers: IngestHeaders {
                delivery_id: Some(Uuid::new_v4().to_string()),
                event_name: Some("pull_request".into()),
                ..Default::default()
            },
            is_backfill: false,
        })
        .await
        .unwrap();
    assert_eq!(outcome.status, IngestStatus::Accepted);

    let ctx_allowed = QueryContext {
        tenant_id: tenant.into(),
        global_user_id: uid.clone(),
        group_ids: rt.acl.get_user_groups(tenant, &uid).await.unwrap(),
    };
    let filter = EventQuery {
        tenant_id: tenant.into(),
        limit: 100,
        ..Default::default()
    };
    let before = rt.store.query(&ctx_allowed, &filter).await.unwrap();
    if before.is_empty() {
        return TestResult {
            id: "TC-03",
            name: "ACL leakage / revocation",
            passed: false,
            detail: "expected private event visible before revocation".into(),
        };
    }

    // Revoke eng group
    let t0 = Instant::now();
    rt.acl
        .remove_user_from_group(tenant, &uid, "grp_eng_core")
        .await
        .unwrap();
    let revoke_ms = t0.elapsed().as_millis();

    // Immediately query 1000 times
    let mut leaks = 0u64;
    for _ in 0..1000 {
        let groups = rt.acl.get_user_groups(tenant, &uid).await.unwrap();
        let ctx = QueryContext {
            tenant_id: tenant.into(),
            global_user_id: uid.clone(),
            group_ids: groups,
        };
        let rows = rt.store.query(&ctx, &filter).await.unwrap();
        // User still has grp_sec_lead but event only allows grp_eng_core (default).
        // After removing eng, should see 0.
        leaks += rows.len() as u64;
    }

    let passed = leaks == 0 && revoke_ms < 200;
    TestResult {
        id: "TC-03",
        name: "ACL leakage / revocation",
        passed,
        detail: format!(
            "leaks={leaks}/1000 revoke_latency_ms={revoke_ms} (budget <200ms)"
        ),
    }
}

/// TC-04: Schema mutations — extra fields, missing optionals, type noise.
async fn tc04_schema_mutations() -> TestResult {
    let rt = make_rt(true);
    let tenant = "ten_schema";
    seed_tenant(&rt, tenant, "secret").await;

    let cases: Vec<(&str, Vec<u8>, bool)> = vec![
        (
            "extra_fields",
            serde_json::to_vec(&json!({
                "action": "opened",
                "unexpected_new_field": { "nested": true, "arr": [1,2,3] },
                "pull_request": {
                    "number": 9,
                    "title": "x",
                    "state": "open",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "user": { "id": 1, "login": "bob" }
                },
                "repository": { "full_name": "acme/app", "private": false },
                "sender": { "id": 1, "login": "bob" }
            }))
            .unwrap(),
            true, // should accept
        ),
        (
            "missing_optionals",
            serde_json::to_vec(&json!({
                "action": "opened",
                "pull_request": {
                    "number": 10,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                },
                "repository": { "full_name": "acme/app" },
                "sender": { "id": 2, "login": "carol" }
            }))
            .unwrap(),
            true,
        ),
        (
            "not_json",
            b"this is not json {{{".to_vec(),
            false, // dead-letter, no crash
        ),
        (
            // With X-GitHub-Event: pull_request, empty body cannot normalize → DLQ (no crash).
            "empty_object",
            b"{}".to_vec(),
            false,
        ),
        (
            // Without a forced event name, empty object uses the generic GitHub fallback.
            "empty_object_generic",
            b"{}".to_vec(),
            true,
        ),
    ];

    let mut ok = true;
    let mut detail_parts = Vec::new();
    for (name, body, expect_accept) in cases {
        let event_name = if name == "empty_object_generic" {
            None
        } else {
            Some("pull_request".into())
        };
        let result = rt
            .pipeline
            .ingest(IngestRequest {
                tenant_id: tenant.into(),
                provider: SourceProvider::Github,
                body,
                headers: IngestHeaders {
                    delivery_id: Some(format!("schema-{name}")),
                    event_name,
                    ..Default::default()
                },
                is_backfill: false,
            })
            .await;

        match result {
            Ok(o) => {
                let accepted = matches!(
                    o.status,
                    IngestStatus::Accepted | IngestStatus::DeadLettered
                );
                // DeadLettered counts as "no crash" for malformed.
                let pass = if expect_accept {
                    o.status == IngestStatus::Accepted
                } else {
                    o.status == IngestStatus::DeadLettered
                };
                if !pass {
                    ok = false;
                }
                detail_parts.push(format!("{name}={:?}/ok={pass}", o.status));
                let _ = accepted;
            }
            Err(e) => {
                ok = false;
                detail_parts.push(format!("{name}=ERR:{e}"));
            }
        }
    }

    TestResult {
        id: "TC-04",
        name: "Schema mutations",
        passed: ok,
        detail: detail_parts.join("; "),
    }
}

/// TC-06: Out-of-order — CLOSED (T2) arrives before OPENED (T1) → state CLOSED.
async fn tc06_out_of_order() -> TestResult {
    let rt = make_rt(true);
    let tenant = "ten_ooo";
    seed_tenant(&rt, tenant, "secret").await;

    let uid = seed_membership(
        rt.acl.as_ref(),
        tenant,
        "gh_1",
        "a@x.com",
        "A",
        &["grp_eng_core"],
    )
    .await
    .unwrap();

    // CLOSED first (origin T2)
    let closed = sample_pr_body("closed", 99, "2026-01-01T00:00:05Z");
    rt.pipeline
        .ingest(IngestRequest {
            tenant_id: tenant.into(),
            provider: SourceProvider::Github,
            body: closed,
            headers: IngestHeaders {
                delivery_id: Some("ooo-closed".into()),
                event_name: Some("pull_request".into()),
                ..Default::default()
            },
            is_backfill: false,
        })
        .await
        .unwrap();

    // OPENED second (origin T1)
    let opened = sample_pr_body("opened", 99, "2026-01-01T00:00:00Z");
    rt.pipeline
        .ingest(IngestRequest {
            tenant_id: tenant.into(),
            provider: SourceProvider::Github,
            body: opened,
            headers: IngestHeaders {
                delivery_id: Some("ooo-opened".into()),
                event_name: Some("pull_request".into()),
                ..Default::default()
            },
            is_backfill: false,
        })
        .await
        .unwrap();

    let ctx = QueryContext {
        tenant_id: tenant.into(),
        global_user_id: uid,
        group_ids: vec!["grp_eng_core".into()],
    };
    let state = pr_state(rt.store.as_ref(), &ctx, "acme/app/pr/99")
        .await
        .unwrap();

    let passed = state.as_deref() == Some("CLOSED");
    TestResult {
        id: "TC-06",
        name: "Out-of-order event state",
        passed,
        detail: format!("derived_state={state:?} expected=Some(\"CLOSED\")"),
    }
}

/// TC-05: Chaos analogue — concurrent publishers + consumer; zero loss.
async fn tc05_chaos_durability(load_count: usize, workers: usize) -> TestResult {
    let rt = Arc::new(make_rt(true));
    let tenant = "ten_chaos";
    seed_tenant(&rt, tenant, "secret").await;

    // Disable inline store path by publishing only via bus and running consumer.
    // Our embedded pipeline always does inline_store — so we also publish extra
    // bus messages and verify bus durable log integrity separately.
    let bus = rt.bus.clone();
    let published = Arc::new(AtomicU64::new(0));

    let mut handles = Vec::new();
    let per = load_count / workers;
    for w in 0..workers {
        let bus = bus.clone();
        let published = published.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..per {
                let id = format!("chaos-{w}-{i}");
                let msg = BusMessage {
                    topic: BusTopic::EventsRaw,
                    partition_key: tenant.into(),
                    payload: BusPayload::Event(CanonicalEventRecord {
                        event_id: id,
                        tenant_id: tenant.into(),
                        provider: SourceProvider::Github,
                        category: EventCategory::Code,
                        event_type: "chaos.ping".into(),
                        event_timestamp: Utc::now(),
                        ingested_at: Utc::now(),
                        actor: ActorIdentity::default(),
                        acl: AclSnapshot {
                            tenant_id: tenant.into(),
                            allowed_group_ids: vec![],
                            is_private: false,
                            acl_version: 1,
                        },
                        resource_id: "chaos".into(),
                        parent_resource_id: "chaos".into(),
                        attributes: json!({}),
                        raw_payload_s3_uri: String::new(),
                        event_sequence_number: (w * per + i) as u64,
                    }),
                };
                if bus.publish(msg).await.is_ok() {
                    published.fetch_add(1, Ordering::Relaxed);
                }
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    // Subscribe late — must still read from durable log (zero data loss).
    let mut sub = bus
        .subscribe(BusTopic::EventsRaw, "chaos-consumer")
        .await
        .unwrap();
    let expected = (per * workers) as u64;
    // Drain the entire durable log in one shot — proves late-join catch-up.
    let drained = sub.drain_log();
    let received = drained.len() as u64;

    let pub_count = published.load(Ordering::Relaxed);
    let passed = pub_count == expected && received == expected;
    TestResult {
        id: "TC-05",
        name: "Chaos / durable bus (zero loss)",
        passed,
        detail: format!("published={pub_count} received={received} expected={expected}"),
    }
}

/// TC-01: Load — concurrent ingest, measure latency percentiles.
async fn tc01_load(load_count: usize, workers: usize, p99_budget_ms: u64) -> TestResult {
    let rt = Arc::new(make_rt(true));
    let tenant = "ten_load";
    seed_tenant(&rt, tenant, "secret").await;

    let ok = Arc::new(AtomicU64::new(0));
    let err = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let mut handles = Vec::new();
    let per = load_count / workers;
    for w in 0..workers {
        let rt = rt.clone();
        let ok = ok.clone();
        let err = err.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..per {
                let body = sample_pr_body("opened", (w * per + i) as u64, "2026-06-01T12:00:00Z");
                let req = IngestRequest {
                    tenant_id: tenant.into(),
                    provider: SourceProvider::Github,
                    body,
                    headers: IngestHeaders {
                        delivery_id: Some(format!("load-{w}-{i}")),
                        event_name: Some("pull_request".into()),
                        ..Default::default()
                    },
                    is_backfill: false,
                };
                match rt.pipeline.ingest(req).await {
                    Ok(o) if o.status == IngestStatus::Accepted => {
                        ok.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(_) => {
                        ok.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        err.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let elapsed = start.elapsed();
    let snap = rt.metrics.snapshot();
    let unique = rt.store.count_unique(tenant).await.unwrap();
    let accepted = ok.load(Ordering::Relaxed);
    let errors = err.load(Ordering::Relaxed);
    let rps = accepted as f64 / elapsed.as_secs_f64().max(0.001);

    // Spec targets 25k rps on GKE; local embedded validates correctness + P99 budget.
    let passed = errors == 0 && unique == accepted && snap.p99_ms <= p99_budget_ms;

    TestResult {
        id: "TC-01",
        name: "Load & latency",
        passed,
        detail: format!(
            "n={accepted} errors={errors} unique={unique} rps={rps:.0} p50={}ms p95={}ms p99={}ms (budget {}ms)",
            snap.p50_ms, snap.p95_ms, snap.p99_ms, p99_budget_ms
        ),
    }
}
