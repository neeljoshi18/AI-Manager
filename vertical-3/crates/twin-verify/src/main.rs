//! Vertical 3 verification battery TC-T01 … TC-T10 (embedded).

use chrono::{Duration, Utc};
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;
use twin_compiler::{
    fixtures::{
        alice_merged_pr_fixture, alice_open_pr_fixture, alice_with_private_pr_fixture,
        bob_no_private_pr_fixture,
    },
    CompileOpts, FixtureGraphSource, LedgerCompiler,
};
use twin_core::egress::env_has_slack_token;
use twin_core::ids::{body_hash, person_twin_id};
use twin_core::model::*;
use twin_core::store::{InMemoryTwinStore, TwinStore};
use twin_delivery::{DeliveryPolicy, DeliveryService, MockSlackClient, SlackClient};

#[derive(Parser)]
struct Args {}

struct TestResult {
    id: &'static str,
    name: &'static str,
    passed: bool,
    detail: String,
}

fn person_twin(tenant: &str, gu: &str, high_auto: bool, shadow_until: Option<chrono::DateTime<Utc>>) -> Twin {
    let now = Utc::now();
    Twin {
        tenant_id: tenant.into(),
        twin_id: person_twin_id(gu),
        twin_kind: TwinKind::Person,
        subject_id: gu.into(),
        display_name: gu.into(),
        timezone: "UTC".into(),
        channel_id: "C_TEAM".into(),
        shadow_until,
        high_auto_publish: high_auto,
        enabled: true,
        config_json: serde_json::json!({}),
        created_at: now,
        updated_at: now,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    let _ = Args::parse();
    info!("Vertical 3 twin verification battery");

    let mut results = Vec::new();
    results.push(tc_t01().await);
    results.push(tc_t02().await);
    results.push(tc_t03().await);
    results.push(tc_t04().await);
    results.push(tc_t05().await);
    results.push(tc_t06().await);
    results.push(tc_t07().await);
    results.push(tc_t08().await);
    results.push(tc_t09().await);
    results.push(tc_t10().await);

    println!();
    println!("═══════════════════════════════════════════════════════════════════");
    println!("           VERTICAL 3 TWIN VERIFICATION BATTERY");
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
        println!("  RESULT: ALL CHECKS PASSED — Vertical 3 ready for smoke/sew");
        Ok(())
    } else {
        std::process::exit(1);
    }
}

async fn tc_t01() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_open_pr_fixture("ten_t"));
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", false, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let items = &out.ledger.ledger.items;
    let has_pr7 = items.iter().any(|i| i.node_id == "pr:acme/app/pr/7");
    let has_evidence = items.iter().any(|i| {
        i.evidence_refs
            .iter()
            .any(|e| e.starts_with("event:") || e.starts_with("edge:"))
    });
    let passed = has_pr7 && has_evidence && !items.is_empty();
    TestResult {
        id: "TC-T01",
        name: "Compile synthetic V2 fixtures → ledger items",
        passed,
        detail: format!(
            "items={} has_pr7={has_pr7} evidence={has_evidence} rollup={:?}",
            items.len(),
            out.ledger.confidence_rollup
        ),
    }
}

async fn tc_t02() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_merged_pr_fixture());
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", true, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    store
        .put_slack_map(SlackUserMap {
            tenant_id: "ten_t".into(),
            global_user_id: "gu_alice".into(),
            slack_user_id: "U_ALICE".into(),
            slack_team_id: String::new(),
        })
        .await
        .unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let pub_rec = store
        .get_publish_by_ledger("ten_t", &out.ledger.ledger_id)
        .await
        .unwrap();
    let passed = out.ledger.confidence_rollup == ConfidenceTier::High
        && pub_rec.is_some()
        && draft.status == DraftStatus::Published;
    TestResult {
        id: "TC-T02",
        name: "High + high_auto_publish → publish_record",
        passed,
        detail: format!(
            "rollup={:?} status={:?} publish={}",
            out.ledger.confidence_rollup,
            draft.status,
            pub_rec.is_some()
        ),
    }
}

async fn tc_t03() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_open_pr_fixture("ten_t"));
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", false, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    store
        .put_slack_map(SlackUserMap {
            tenant_id: "ten_t".into(),
            global_user_id: "gu_alice".into(),
            slack_user_id: "U_ALICE".into(),
            slack_team_id: String::new(),
        })
        .await
        .unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let dm_count = slack.dm_posts().len();
    let (draft2, pub_rec) = service
        .silence_timeout(&twin, "ten_t", &draft.draft_id)
        .await
        .unwrap();
    let channel_posts = slack.channel_posts().len();
    let passed = out.ledger.confidence_rollup == ConfidenceTier::Medium
        && dm_count == 1
        && draft.status == DraftStatus::Pending
        && draft2.status == DraftStatus::Published
        && pub_rec.is_some()
        && channel_posts == 1;
    TestResult {
        id: "TC-T03",
        name: "Medium → DM; silence → publish",
        passed,
        detail: format!(
            "dm={dm_count} channel={channel_posts} final={:?} publish={}",
            draft2.status,
            pub_rec.is_some()
        ),
    }
}

async fn tc_t04() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_open_pr_fixture("ten_t"));
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", false, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    store
        .put_slack_map(SlackUserMap {
            tenant_id: "ten_t".into(),
            global_user_id: "gu_alice".into(),
            slack_user_id: "U_ALICE".into(),
            slack_team_id: String::new(),
        })
        .await
        .unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let vetoed = twin_delivery::veto_draft(store.clone(), "ten_t", &draft.draft_id)
        .await
        .unwrap();
    let pub_attempt = service
        .explicit_publish(&twin, "ten_t", &draft.draft_id)
        .await;
    let pub_rec = store
        .get_publish_by_ledger("ten_t", &out.ledger.ledger_id)
        .await
        .unwrap();
    let channel_posts = slack.channel_posts().len();
    let passed = vetoed.status == DraftStatus::Vetoed
        && pub_rec.is_none()
        && channel_posts == 0
        && pub_attempt.is_err();
    TestResult {
        id: "TC-T04",
        name: "Veto → never channel post",
        passed,
        detail: format!(
            "status={:?} publish={} channel={channel_posts}",
            vetoed.status,
            pub_rec.is_some()
        ),
    }
}

async fn tc_t05() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_open_pr_fixture("ten_t"));
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", false, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    store
        .put_slack_map(SlackUserMap {
            tenant_id: "ten_t".into(),
            global_user_id: "gu_alice".into(),
            slack_user_id: "U_ALICE".into(),
            slack_team_id: String::new(),
        })
        .await
        .unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let edited = "Edited status: finished auth race PR".to_string();
    let _ = twin_delivery::edit_draft(store.clone(), "ten_t", &draft.draft_id, &edited)
        .await
        .unwrap();
    let (_d, pub_rec) = service
        .explicit_publish(&twin, "ten_t", &draft.draft_id)
        .await
        .unwrap();
    let expected_hash = body_hash(&edited);
    let posts = slack.channel_posts();
    let body_ok = posts.iter().any(|p| p.text == edited);
    let hash_ok = pub_rec.as_ref().map(|p| p.body_hash.as_str()) == Some(expected_hash.as_str());
    let passed = body_ok && hash_ok && pub_rec.is_some();
    TestResult {
        id: "TC-T05",
        name: "Edit DM text → published body matches",
        passed,
        detail: format!("hash_ok={hash_ok} body_ok={body_ok}"),
    }
}

async fn tc_t06() -> TestResult {
    // Compiler only sees ACL-filtered GraphView — bob's view has no private PR.
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::empty();
    // Alice eng view includes private PR; bob does not.
    source.set_view("ten_t", "gu_alice", alice_with_private_pr_fixture());
    source.set_view("ten_t", "gu_bob", bob_no_private_pr_fixture());
    let compiler = LedgerCompiler::new(store.clone(), source);
    let bob = person_twin("ten_t", "gu_bob", false, None);
    store.upsert_twin(bob.clone()).await.unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &bob,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let leaked = out
        .ledger
        .ledger
        .items
        .iter()
        .any(|i| i.node_id.contains("secret") || i.node_id == "pr:acme/secret/pr/1");
    // Also ensure alice would see private if her view used (control)
    let alice = person_twin("ten_t", "gu_alice", false, None);
    store.upsert_twin(alice.clone()).await.unwrap();
    let alice_out = compiler
        .compile_person(
            &alice,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let alice_sees = alice_out
        .ledger
        .ledger
        .items
        .iter()
        .any(|i| i.node_id == "pr:acme/secret/pr/1");
    let passed = !leaked && alice_sees;
    TestResult {
        id: "TC-T06",
        name: "ACL: compile cannot include private PR outside groups",
        passed,
        detail: format!("bob_leaked={leaked} alice_sees_private={alice_sees}"),
    }
}

async fn tc_t07() -> TestResult {
    // Twin env must not use Slack token; publish uses mock/proxy path.
    let has_token = env_has_slack_token();
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_merged_pr_fixture());
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", true, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let _ = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    // Mock was called (stand-in for egress proxy); no Authorization in twin process.
    let called = slack.call_count() > 0;
    let passed = !has_token && called;
    TestResult {
        id: "TC-T07",
        name: "Egress: no Slack token in twin env; publish via client",
        passed,
        detail: format!("env_slack_token={has_token} slack_calls={}", slack.call_count()),
    }
}

async fn tc_t08() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_merged_pr_fixture());
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_t", "gu_alice", true, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    // Double schedule publish
    let r1 = service
        .explicit_publish(&twin, "ten_t", &draft.draft_id)
        .await;
    let r2 = service
        .explicit_publish(&twin, "ten_t", &draft.draft_id)
        .await;
    let pub_rec = store
        .get_publish_by_ledger("ten_t", &out.ledger.ledger_id)
        .await
        .unwrap();
    let channel_posts = slack.channel_posts().len();
    // First publish path may have already published via high_auto; second must not create new record
    let single_record = pub_rec.is_some();
    let passed = single_record && channel_posts == 1 && r1.is_ok() && r2.is_ok();
    TestResult {
        id: "TC-T08",
        name: "Exactly-once publish same ledger",
        passed,
        detail: format!(
            "channel_posts={channel_posts} publish={} ts={:?}",
            pub_rec.is_some(),
            pub_rec.as_ref().map(|p| &p.slack_ts)
        ),
    }
}

async fn tc_t09() -> TestResult {
    let store = InMemoryTwinStore::new();
    let source = FixtureGraphSource::new(alice_open_pr_fixture("ten_t"));
    let compiler = LedgerCompiler::new(store.clone(), source);
    let shadow_until = Utc::now() + Duration::days(10);
    let twin = person_twin("ten_t", "gu_alice", false, Some(shadow_until));
    store.upsert_twin(twin.clone()).await.unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let passed = draft.status == DraftStatus::Shadow && slack.call_count() == 0;
    TestResult {
        id: "TC-T09",
        name: "Shadow: compile only, no DM",
        passed,
        detail: format!("status={:?} slack_calls={}", draft.status, slack.call_count()),
    }
}

async fn tc_t10() -> TestResult {
    // Sew path in-process: synthetic V1-shaped event → graph fixture (V2) → V3 compile → draft
    // (Live multi-process sew is scripts/sew_e2e.sh; this verifies the chain logic.)
    let store = InMemoryTwinStore::new();
    // Simulate V2 projection of a PR opened event as ACL-visible neighborhood
    let mut view = alice_open_pr_fixture("ten_sew");
    view.edges[0].event_id = "v1_evt_sew_001".into();
    let source = FixtureGraphSource::new(view);
    let compiler = LedgerCompiler::new(store.clone(), source);
    let twin = person_twin("ten_sew", "gu_alice", false, None);
    store.upsert_twin(twin.clone()).await.unwrap();
    store
        .put_slack_map(SlackUserMap {
            tenant_id: "ten_sew".into(),
            global_user_id: "gu_alice".into(),
            slack_user_id: "U_SEW".into(),
            slack_team_id: String::new(),
        })
        .await
        .unwrap();
    let now = Utc::now();
    let out = compiler
        .compile_person(
            &twin,
            &CompileOpts::window(now - Duration::days(1), now, 2),
        )
        .await
        .unwrap();
    let slack = MockSlackClient::new();
    let service = DeliveryService::new(store.clone(), slack.clone(), DeliveryPolicy::default());
    let draft = service
        .start_after_compile(&twin, &out.ledger, &out.draft_text, now)
        .await
        .unwrap();
    let has_v1_evidence = out.ledger.ledger.items.iter().any(|i| {
        i.evidence_refs
            .iter()
            .any(|e| e.contains("v1_evt_sew_001"))
    });
    let passed = has_v1_evidence
        && !out.ledger.ledger.items.is_empty()
        && draft.status == DraftStatus::Pending
        && !draft.draft_id.is_empty()
        && slack.dm_posts().len() == 1;
    TestResult {
        id: "TC-T10",
        name: "Sew E2E: V1 evidence → V2 fixture → V3 compile → draft",
        passed,
        detail: format!(
            "items={} evidence_v1={has_v1_evidence} draft={:?} dm={}",
            out.ledger.ledger.items.len(),
            draft.status,
            slack.dm_posts().len()
        ),
    }
}
