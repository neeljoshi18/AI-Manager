//! Vertical 3 twin-api — status twins, ledgers, veto-first delivery (:18083).
//! Demo console at `/demo/` for founder/lead visibility (M4 Sew & Show).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Duration, Timelike, Utc};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;
use twin_compiler::{
    CompileOpts, FixtureGraphSource, HttpV2GraphSource, LedgerCompiler, OverlayGraphSource,
};
use twin_core::config::TwinConfig;
use twin_core::ids::person_twin_id;
use twin_core::model::*;
use twin_core::store::{InMemoryTwinStore, TwinStore};
use twin_core::TwinError;
use twin_delivery::{
    DeliveryAdapterKind, DeliveryClient, DeliveryPolicy, DeliveryService, EgressSlackClient,
    EgressTeamsClient, MockSlackClient, MockTeamsClient,
};

mod observe;
mod intent_engine;
use observe::EventObserver;

#[derive(Default)]
struct Metrics {
    compile_ok: AtomicU64,
    compile_error: AtomicU64,
    drafts_sent: AtomicU64,
    veto_total: AtomicU64,
    publish_ok: AtomicU64,
    publish_fail: AtomicU64,
    acl_empty: AtomicU64,
    egress_fail: AtomicU64,
    /// Compiles with no ledger items / blockers (empty window skip).
    empty_windows: AtomicU64,
    /// Conflict monitor ticks that found ≥1 card.
    conflict_hits: AtomicU64,
    monitor_ticks: AtomicU64,
    /// Notify Policy v1: DMs not sent (unchanged / daily_cap / quiet).
    dms_suppressed: AtomicU64,
}

#[derive(Clone)]
struct AppState {
    store: Arc<dyn TwinStore>,
    /// When embedded: concrete store for disk persist of team map + digests (A2/A3).
    embedded_store: Option<Arc<InMemoryTwinStore>>,
    /// Path for twin state file (None = no persist).
    twin_persist_path: Option<PathBuf>,
    compiler: Arc<LedgerCompiler>,
    /// Fixture source when embedded — allows inject for tests via admin route.
    fixture: Option<Arc<FixtureGraphSource>>,
    /// Active chat delivery adapter (Slack, Teams, or mock).
    slack: Arc<dyn DeliveryClient>,
    policy: DeliveryPolicy,
    mode: String,
    /// "mock" | "egress" | "teams"
    slack_mode: String,
    /// "slack" | "teams" | "mock"
    delivery_adapter: String,
    metrics: Arc<Metrics>,
    cfg: TwinConfig,
    /// Last demo simulation snapshot per tenant (for console).
    last_demo: Arc<Mutex<std::collections::HashMap<String, serde_json::Value>>>,
    /// Last Slack notify time per (tenant, twin_id) for debounce.
    last_notify: Arc<Mutex<std::collections::HashMap<(String, String), chrono::DateTime<Utc>>>>,
    /// Cached team pulse (conflicts + intent counts) per tenant from thin monitor.
    last_pulse: Arc<Mutex<std::collections::HashMap<String, serde_json::Value>>>,
    /// Live event log (embedded + optional Neon Postgres).
    observer: EventObserver,
}

fn twin_persist_path_from_env() -> Option<PathBuf> {
    match std::env::var("TWIN_EMBEDDED_STATE_PATH") {
        Ok(p) if !p.trim().is_empty() => Some(PathBuf::from(p.trim())),
        _ => None,
    }
}

fn persist_embedded(st: &AppState) {
    let (Some(store), Some(path)) = (&st.embedded_store, &st.twin_persist_path) else {
        return;
    };
    if let Err(e) = store.save_to_path(path) {
        tracing::warn!(error = %e, path = %path.display(), "twin persist save failed");
    }
    // Continuous Neon mirror (debounced full upsert) — no manual sync required
    mirror_to_neon(st);
}

/// Dual-write embedded twin state → Neon when OBSERVE_DATABASE_URL is connected.
fn mirror_to_neon(st: &AppState) {
    if !st.observer.external_connected() {
        return;
    }
    let Some(store) = st.embedded_store.clone() else {
        return;
    };
    let obs = st.observer.clone();
    let tenant = std::env::var("DEFAULT_TENANT_ID")
        .or_else(|_| std::env::var("SEED_TEAM_TENANT"))
        .unwrap_or_else(|_| "ten_github".into());
    tokio::spawn(async move {
        match obs.sync_store(&tenant, store.as_ref()).await {
            Ok(body) => {
                let twins = body.get("twins").and_then(|v| v.as_u64()).unwrap_or(0);
                let drafts = body.get("drafts").and_then(|v| v.as_u64()).unwrap_or(0);
                tracing::debug!(twins, drafts, "neon continuous mirror ok");
            }
            Err(e) => tracing::warn!(error = %e, "neon continuous mirror failed"),
        }
    });
}

/// Seed person twins from SLACK_USER_MAP — **one twin per unique Slack user id**.
/// Format: `githubLogin:U…,numericId:U…,otherLogin:U2…`
async fn seed_team_from_env(store: &dyn TwinStore) {
    let raw = std::env::var("SEED_SLACK_USER_MAP")
        .or_else(|_| std::env::var("SLACK_USER_MAP"))
        .unwrap_or_default();
    if raw.trim().is_empty() {
        return;
    }
    let tenant = std::env::var("SEED_TEAM_TENANT").unwrap_or_else(|_| "ten_github".into());
    let channel = std::env::var("SEED_TEAM_CHANNEL").unwrap_or_default();
    let now = Utc::now();

    // Group keys by slack user id first
    let mut by_slack: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() || !part.contains(':') {
            continue;
        }
        let (key, slack) = part.split_once(':').unwrap();
        let key = key.trim().to_string();
        let slack = slack.trim().to_string();
        if key.is_empty() || slack.is_empty() {
            continue;
        }
        by_slack.entry(slack).or_default().push(key);
    }

    let mut seeded = 0u32;
    for (slack, mut keys) in by_slack {
        // Prefer login (non-numeric, not gu_*) for display; keep all as aliases
        keys.sort_by(|a, b| {
            let score = |k: &str| {
                if k.starts_with("gu_") {
                    2
                } else if k.chars().all(|c| c.is_ascii_digit()) {
                    1
                } else {
                    0
                }
            };
            score(a).cmp(&score(b)).then_with(|| a.cmp(b))
        });
        let login = keys
            .iter()
            .find(|k| !k.starts_with("gu_") && !k.chars().all(|c| c.is_ascii_digit()))
            .cloned()
            .unwrap_or_else(|| keys[0].clone());
        let display = login.clone();

        // If any enabled twin already maps this slack, only merge aliases — do not create another
        let maps = store.list_slack_maps(&tenant).await.unwrap_or_default();
        let twins = store.list_twins(&tenant).await.unwrap_or_default();
        if let Some(existing_sub) = maps
            .iter()
            .filter(|m| m.slack_user_id == slack)
            .find_map(|m| {
                twins
                    .iter()
                    .find(|t| {
                        t.twin_kind == TwinKind::Person
                            && t.enabled
                            && t.subject_id == m.global_user_id
                            && !t.subject_id.starts_with("gu_seed_")
                    })
                    .map(|t| t.subject_id.clone())
            })
            .or_else(|| {
                maps.iter()
                    .find(|m| m.slack_user_id == slack)
                    .map(|m| m.global_user_id.clone())
            })
        {
            // Merge aliases onto that twin
            let twin_id = person_twin_id(&existing_sub);
            if let Ok(Some(mut tw)) = store.get_twin(&tenant, &twin_id).await {
                let mut aliases: Vec<String> = tw
                    .config_json
                    .get("provider_aliases")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                for k in &keys {
                    if !aliases.iter().any(|a| a == k) {
                        aliases.push(k.clone());
                    }
                    let _ = store
                        .put_slack_map(SlackUserMap {
                            tenant_id: tenant.clone(),
                            global_user_id: k.clone(),
                            slack_user_id: slack.clone(),
                            slack_team_id: String::new(),
                        })
                        .await;
                }
                if let Some(obj) = tw.config_json.as_object_mut() {
                    obj.insert("provider_aliases".into(), json!(aliases));
                }
                if tw.display_name.starts_with("user_") || tw.display_name.is_empty() {
                    tw.display_name = display;
                }
                tw.updated_at = now;
                let _ = store.upsert_twin(tw).await;
            }
            continue;
        }

        // Canonical provisional subject: gu_seed_<login> (stable across boots until real gu merges)
        let subject = format!("gu_seed_{login}");
        let twin_id = person_twin_id(&subject);
        let twin = Twin {
            tenant_id: tenant.clone(),
            twin_id: twin_id.clone(),
            twin_kind: TwinKind::Person,
            subject_id: subject.clone(),
            display_name: display,
            timezone: "UTC".into(),
            channel_id: channel.clone(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: json!({ "provider_aliases": keys, "seeded": true }),
            created_at: now,
            updated_at: now,
        };
        let _ = store.upsert_twin(twin).await;
        let _ = store
            .put_slack_map(SlackUserMap {
                tenant_id: tenant.clone(),
                global_user_id: subject,
                slack_user_id: slack.clone(),
                slack_team_id: String::new(),
            })
            .await;
        for k in keys {
            let _ = store
                .put_slack_map(SlackUserMap {
                    tenant_id: tenant.clone(),
                    global_user_id: k,
                    slack_user_id: slack.clone(),
                    slack_team_id: String::new(),
                })
                .await;
        }
        seeded += 1;
    }
    // Collapse historical junk twins (multiple gu_* for same Slack)
    let pruned = prune_duplicate_slack_twins(store, &tenant).await;
    if seeded > 0 || pruned > 0 {
        info!(
            seeded,
            pruned,
            tenant = %tenant,
            "team seed from SLACK_USER_MAP (one twin per Slack user)"
        );
    }
}

/// Collect provider_aliases strings from a twin config.
fn twin_alias_list(t: &Twin) -> Vec<String> {
    t.config_json
        .get("provider_aliases")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Merge alias keys into twin config (dedupe, preserve order).
fn merge_aliases_into_twin(t: &mut Twin, extra: impl IntoIterator<Item = String>) {
    let mut aliases = twin_alias_list(t);
    for k in extra {
        let k = k.trim().to_string();
        if k.is_empty() {
            continue;
        }
        if !aliases.iter().any(|a| a == &k) {
            aliases.push(k);
        }
    }
    if !t.config_json.is_object() {
        t.config_json = json!({});
    }
    if let Some(obj) = t.config_json.as_object_mut() {
        obj.insert("provider_aliases".into(), json!(aliases));
    }
}

/// Membership user ids for V2 ACL: primary subject + historical gu_* aliases.
fn membership_user_ids_for_twins(twins: &[Twin]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        let s = s.trim();
        if s.is_empty() {
            return;
        }
        if !ids.iter().any(|x| x == s) {
            ids.push(s.to_string());
        }
    };
    for t in twins
        .iter()
        .filter(|t| t.enabled && t.twin_kind == TwinKind::Person)
    {
        push(&t.subject_id);
        for a in twin_alias_list(t) {
            // Only gu_* need V2 user rows for neighborhood ACL as that person.
            if a.starts_with("gu_") {
                push(&a);
            }
        }
    }
    ids
}

/// Keep one enabled person twin per Slack user id; disable the rest (floating graph ghosts).
/// **Critical:** fold disabled gu_* subjects + aliases into the keeper so digests still
/// multi-identity-compile historical graph edges.
async fn prune_duplicate_slack_twins(store: &dyn TwinStore, tenant: &str) -> u32 {
    let maps = store.list_slack_maps(tenant).await.unwrap_or_default();
    let twins = store.list_twins(tenant).await.unwrap_or_default();
    let mut by_slack: std::collections::HashMap<String, Vec<Twin>> =
        std::collections::HashMap::new();
    for t in twins
        .into_iter()
        .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
    {
        let slack = maps
            .iter()
            .find(|m| m.global_user_id == t.subject_id)
            .map(|m| m.slack_user_id.clone())
            .unwrap_or_default();
        if slack.is_empty() {
            continue;
        }
        by_slack.entry(slack).or_default().push(t);
    }
    let mut pruned = 0u32;
    let now = Utc::now();
    for (slack, mut group) in by_slack {
        if group.len() <= 1 {
            continue;
        }
        // Prefer: real gu_* (not gu_seed_), then most recently updated
        group.sort_by(|a, b| {
            let score = |t: &Twin| {
                let seed = if t.subject_id.starts_with("gu_seed_") {
                    0
                } else if t.subject_id.starts_with("gu_") {
                    2
                } else {
                    1
                };
                (seed, t.updated_at)
            };
            score(b).cmp(&score(a))
        });
        let mut keep = group.remove(0);
        let mut fold: Vec<String> = Vec::new();
        for mut dead in group {
            // Fold identity so compiler still fetches person:gu_old neighborhoods
            fold.push(dead.subject_id.clone());
            fold.extend(twin_alias_list(&dead));
            dead.enabled = false;
            dead.updated_at = now;
            if let Some(obj) = dead.config_json.as_object_mut() {
                obj.insert("disabled_reason".into(), json!("duplicate_slack_prune"));
                obj.insert("merged_into".into(), json!(keep.twin_id));
            }
            if store.upsert_twin(dead).await.is_ok() {
                pruned += 1;
            }
        }
        if !fold.is_empty() {
            merge_aliases_into_twin(&mut keep, fold.clone());
            keep.updated_at = now;
            let _ = store.upsert_twin(keep.clone()).await;
            // Slack map rows for folded gu_* → same Slack (bridge + ACL)
            for k in fold {
                if k.starts_with("gu_") || k.chars().all(|c| c.is_ascii_digit()) {
                    let _ = store
                        .put_slack_map(SlackUserMap {
                            tenant_id: tenant.to_string(),
                            global_user_id: k,
                            slack_user_id: slack.clone(),
                            slack_team_id: String::new(),
                        })
                        .await;
                }
            }
        }
    }
    pruned
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cfg = TwinConfig::from_env();
    let state = build_state(cfg.clone()).await?;

    // Persist embedded twin state periodically (team map + digests survive restart).
    if state.embedded_store.is_some() && state.twin_persist_path.is_some() {
        let st = state.clone();
        tokio::spawn(async move {
            let every = std::time::Duration::from_secs(
                std::env::var("TWIN_PERSIST_INTERVAL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30),
            );
            loop {
                tokio::time::sleep(every).await;
                persist_embedded(&st);
            }
        });
    }

    // Scheduled compile + batched notify (ingest is continuous; Slack is not)
    if cfg.compile_interval_secs > 0 {
        let st = state.clone();
        let interval = std::time::Duration::from_secs(cfg.compile_interval_secs as u64);
        tokio::spawn(async move {
            info!(
                secs = st.cfg.compile_interval_secs,
                notify = st.cfg.notify_interval_secs,
                window = st.cfg.status_window_secs,
                "status scheduler started (batched DMs)"
            );
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = run_scheduled_compiles(&st).await {
                    tracing::warn!(error = %e, "scheduled compile tick failed");
                }
                // Save after compile tick so latest drafts land on disk
                persist_embedded(&st);
                // Thin monitor: ingest-ish health + conflict cards (no Slack spam)
                if let Err(e) = run_thin_monitors(&st).await {
                    tracing::debug!(error = %e, "thin monitor tick failed");
                }
            }
        });
    }

    // Periodic V2 graph → Neon export (SQL insights; does not block request path)
    if state.observer.external_connected() {
        let st = state.clone();
        let interval_secs: u64 = std::env::var("GRAPH_NEON_EXPORT_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900);
        tokio::spawn(async move {
            info!(
                secs = interval_secs,
                "graph neon export loop started (periodic + on-demand)"
            );
            // Short initial delay so V2 can finish booting with twin-api
            tokio::time::sleep(std::time::Duration::from_secs(45)).await;
            loop {
                // try_lock: skip tick if on-demand export holds the gate
                match export_graph_to_neon_inner(&st, None, false).await {
                    Ok(_) => {}
                    Err(e) if e.contains("already in progress") => {
                        tracing::debug!(error = %e, "graph neon export tick skipped");
                    }
                    Err(e) => tracing::warn!(error = %e, "graph neon export tick failed"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
            }
        });
    }

    let demo_dir = demo_static_dir();
    let app_dir = app_static_dir();
    info!(?demo_dir, ?app_dir, "static directories");
    let demo_index = ServeFile::new(demo_dir.join("index.html"));
    let demo_files = ServeDir::new(&demo_dir).append_index_html_on_directories(true);
    let app_index = ServeFile::new(app_dir.join("index.html"));
    let app_files = ServeDir::new(&app_dir).append_index_html_on_directories(true);

    let api = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .route("/", get(|| async { Redirect::temporary("/app/") }))
        .route("/app", get(|| async { Redirect::permanent("/app/") }))
        .route("/demo", get(|| async { Redirect::permanent("/demo/") }))
        .route("/v3/demo/status", get(demo_status))
        .route("/v3/demo/simulate", post(demo_simulate))
        .route("/v3/demo/latest", get(demo_latest))
        .route("/v3/onboarding/status", get(onboarding_status))
        .route("/v3/oauth/status", get(oauth_status))
        .route("/v3/oauth/slack/start", get(oauth_slack_start))
        .route("/v3/oauth/slack/callback", get(oauth_slack_callback))
        .route("/v3/oauth/github/start", get(oauth_github_start))
        .route("/v3/oauth/teams/start", get(oauth_teams_start))
        .route(
            "/v3/tenants/{tenant_id}/twins",
            get(list_twins_route).post(upsert_twin),
        )
        .route(
            "/v3/tenants/{tenant_id}/roles",
            get(get_roles).put(put_roles),
        )
        .route(
            "/v3/tenants/{tenant_id}/tomorrow_focus",
            get(get_tomorrow_focus).put(put_tomorrow_focus),
        )
        .route(
            "/v3/tenants/{tenant_id}/events",
            get(list_events),
        )
        .route(
            "/v3/tenants/{tenant_id}/sync_to_db",
            post(sync_to_db),
        )
        .route(
            "/v3/tenants/{tenant_id}/sync_graph_to_db",
            post(sync_graph_to_db),
        )
        .route("/v3/observe/status", get(observe_status))
        .route(
            "/v3/tenants/{tenant_id}/twins/{twin_id}",
            get(get_twin),
        )
        .route("/v3/tenants/{tenant_id}/team", get(get_team))
        .route(
            "/v3/tenants/{tenant_id}/team/members",
            post(upsert_team_member),
        )
        .route(
            "/v3/tenants/{tenant_id}/team/compile",
            post(compile_team),
        )
        .route(
            "/v3/tenants/{tenant_id}/seed/intent_demo",
            post(seed_intent_demo_proxy),
        )
        .route(
            "/v3/tenants/{tenant_id}/team/prune",
            post(prune_team_duplicates),
        )
        .route(
            "/v3/tenants/{tenant_id}/graph/ensure_users",
            post(ensure_graph_users),
        )
        .route(
            "/v3/tenants/{tenant_id}/pilot_readiness",
            get(pilot_readiness),
        )
        .route("/v3/tenants/{tenant_id}/pulse", get(get_pulse))
        .route("/v3/tenants/{tenant_id}/conflicts", get(get_conflicts_proxy))
        .route("/v3/tenants/{tenant_id}/graph", get(get_graph_snapshot))
        .route(
            "/v3/tenants/{tenant_id}/people/{subject_id}/profile",
            get(get_person_profile),
        )
        .route(
            "/v3/tenants/{tenant_id}/people/{subject_id}/follow_through",
            get(get_follow_through),
        )
        // In-house Intent Engine (plans/intent-research.md + 2026-08-07 design)
        .route(
            "/v3/tenants/{tenant_id}/intent/engine",
            get(intent_engine_status),
        )
        .route(
            "/v3/tenants/{tenant_id}/intent/ledger",
            get(intent_ledger),
        )
        .route(
            "/v3/tenants/{tenant_id}/intent/claims",
            post(intent_claim_create),
        )
        .route(
            "/v3/tenants/{tenant_id}/intent/claims/{claim_id}/supersede",
            post(intent_claim_supersede),
        )
        .route(
            "/v3/tenants/{tenant_id}/insights/dev",
            get(dev_insights),
        )
        .route(
            "/v3/tenants/{tenant_id}/seed/dual_digests",
            post(seed_dual_digests),
        )
        .route(
            "/v3/tenants/{tenant_id}/seed/graph_story",
            post(seed_graph_story),
        )
        .route(
            "/v3/tenants/{tenant_id}/twins/{twin_id}/compile",
            post(compile_twin),
        )
        .route(
            "/v3/tenants/{tenant_id}/ledgers/{ledger_id}",
            get(get_ledger),
        )
        .route(
            "/v3/tenants/{tenant_id}/drafts/{draft_id}",
            get(get_draft),
        )
        .route(
            "/v3/tenants/{tenant_id}/drafts/{draft_id}/veto",
            post(veto_draft),
        )
        .route(
            "/v3/tenants/{tenant_id}/drafts/{draft_id}/publish",
            post(publish_draft),
        )
        .route(
            "/v3/tenants/{tenant_id}/drafts/{draft_id}/edit",
            post(edit_draft),
        )
        .route(
            "/v3/tenants/{tenant_id}/drafts/{draft_id}/silence",
            post(silence_draft),
        )
        .route("/v3/slack/interactions", post(slack_interactions))
        .route("/v3/slack/events", post(slack_events))
        .route("/v3/teams/messages", post(teams_messages))
        .route(
            "/v3/tenants/{tenant_id}/fixtures",
            post(set_fixture),
        )
        .with_state(state);

    let app = Router::new()
        .merge(api)
        .nest_service("/app/", app_files)
        .route_service("/app/index.html", app_index)
        .nest_service("/demo/", demo_files)
        .route_service("/demo/index.html", demo_index)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = cfg.http_bind.parse()?;
    info!(%addr, mode = %cfg.runtime_mode, "twin-api listening (product /app/ · lab /demo/)");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn demo_static_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DEMO_STATIC_DIR") {
        return PathBuf::from(p);
    }
    // crates/twin-api -> vertical-3/demo-static
    let from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo-static");
    if from_manifest.exists() {
        return from_manifest;
    }
    PathBuf::from("demo-static")
}

fn app_static_dir() -> PathBuf {
    if let Ok(p) = std::env::var("APP_STATIC_DIR") {
        return PathBuf::from(p);
    }
    let from_manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app-static");
    if from_manifest.exists() {
        return from_manifest;
    }
    PathBuf::from("app-static")
}

async fn build_state(cfg: TwinConfig) -> anyhow::Result<AppState> {
    let metrics = Arc::new(Metrics::default());
    let mut policy = DeliveryPolicy::default();
    policy.medium_veto_window_secs = cfg.medium_veto_window_secs;
    policy.blocker_veto_window_secs = cfg.blocker_veto_window_secs;

    let last_demo = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let last_notify = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let last_pulse = Arc::new(Mutex::new(std::collections::HashMap::new()));

    if cfg.is_embedded() {
        info!("runtime mode=embedded");
        let mem = InMemoryTwinStore::new();
        let persist_path = twin_persist_path_from_env();
        if let Some(ref path) = persist_path {
            match mem.load_from_path(path) {
                Ok(true) => info!(path = %path.display(), "restored embedded twin state"),
                Ok(false) => info!(path = %path.display(), "no twin state file yet"),
                Err(e) => tracing::warn!(error = %e, "twin state load failed"),
            }
        }
        // Multi-person seed from map env (idempotent; fills gaps after cold start)
        seed_team_from_env(mem.as_ref()).await;
        let _ = prune_duplicate_slack_twins(mem.as_ref(), "ten_github").await;
        if let Some(ref path) = persist_path {
            if let Err(e) = mem.save_to_path(path) {
                tracing::warn!(error = %e, "initial twin persist failed");
            }
        }
        let store: Arc<dyn TwinStore> = mem.clone();
        let fixture = FixtureGraphSource::empty();
        // Fixture (demo inject) wins; otherwise live V2 graph-api ACL reads
        let overlay = OverlayGraphSource::new(fixture.clone(), &cfg.v2_base_url);
        let compiler = Arc::new(LedgerCompiler::new(store.clone(), overlay));
        let (slack, slack_mode, delivery_adapter) = build_delivery_client(&cfg)?;
        let observer = EventObserver::from_env(Some(mem.clone())).await;
        return Ok(AppState {
            store,
            embedded_store: Some(mem),
            twin_persist_path: persist_path,
            compiler,
            fixture: Some(fixture),
            slack,
            policy,
            mode: "embedded".into(),
            slack_mode,
            delivery_adapter,
            metrics,
            cfg,
            last_demo,
            last_notify: last_notify.clone(),
            last_pulse: last_pulse.clone(),
            observer,
        });
    }

    use twin_core::store_crdb::CrdbTwinStore;
    let url = cfg
        .cockroach_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("COCKROACH_URL required for production mode"))?;
    info!(%url, "connecting cockroach status_twins");
    let store: Arc<dyn TwinStore> = CrdbTwinStore::connect(&url).await?;
    // Prefer V2 HTTP; also keep fixture for demo simulate when DEMO_FIXTURES=true
    let fixture = FixtureGraphSource::empty();
    let use_fixture_overlay = std::env::var("DEMO_FIXTURES")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let source: Arc<dyn twin_compiler::GraphSource> = if use_fixture_overlay {
        fixture.clone()
    } else {
        HttpV2GraphSource::new(&cfg.v2_base_url)
    };
    let compiler = Arc::new(LedgerCompiler::new(store.clone(), source));
    let (slack, slack_mode, delivery_adapter) = if std::env::var("FORCE_MOCK_SLACK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        let mock: Arc<dyn DeliveryClient> = MockSlackClient::new();
        (mock, "mock".into(), "mock".into())
    } else {
        build_delivery_client(&cfg)?
    };
    Ok(AppState {
        store,
        embedded_store: None,
        twin_persist_path: None,
        compiler,
        fixture: Some(fixture),
        slack,
        policy,
        mode: "production".into(),
        slack_mode,
        delivery_adapter,
        metrics,
        cfg,
        last_demo,
        last_notify,
        last_pulse,
        observer: EventObserver::from_env(None).await,
    })
}

/// Select Slack (default) or Teams delivery adapter. Slack path unchanged unless DELIVERY_ADAPTER=teams.
fn build_delivery_client(
    cfg: &TwinConfig,
) -> anyhow::Result<(Arc<dyn DeliveryClient>, String, String)> {
    let kind = DeliveryAdapterKind::from_env();
    let force_egress_slack = std::env::var("USE_EGRESS_SLACK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let force_egress_teams = std::env::var("USE_EGRESS_TEAMS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    match kind {
        DeliveryAdapterKind::Teams => {
            if force_egress_teams || force_egress_slack {
                let egress = twin_core::EgressClient::new(twin_core::EgressConfig {
                    proxy_url: cfg.egress_proxy_url.clone(),
                    enforce: cfg.egress_enforce,
                })?;
                info!("Teams delivery via egress proxy (DELIVERY_ADAPTER=teams)");
                Ok((
                    Arc::new(EgressTeamsClient::new(egress)),
                    "teams".into(),
                    "teams".into(),
                ))
            } else {
                info!("Teams delivery: mock (set USE_EGRESS_TEAMS=true + vault TEAMS_BOT_TOKEN)");
                let mock: Arc<dyn DeliveryClient> = MockTeamsClient::new();
                Ok((mock, "mock".into(), "teams".into()))
            }
        }
        DeliveryAdapterKind::Mock => {
            info!("delivery adapter: mock");
            let mock: Arc<dyn DeliveryClient> = MockSlackClient::new();
            Ok((mock, "mock".into(), "mock".into()))
        }
        DeliveryAdapterKind::Slack => {
            if force_egress_slack {
                let egress = twin_core::EgressClient::new(twin_core::EgressConfig {
                    proxy_url: cfg.egress_proxy_url.clone(),
                    enforce: cfg.egress_enforce,
                })?;
                info!("Slack delivery via egress proxy (USE_EGRESS_SLACK=true)");
                Ok((
                    Arc::new(EgressSlackClient::new(egress)),
                    "egress".into(),
                    "slack".into(),
                ))
            } else {
                info!("Slack delivery: mock (set USE_EGRESS_SLACK=true for real DMs)");
                let mock: Arc<dyn DeliveryClient> = MockSlackClient::new();
                Ok((mock, "mock".into(), "slack".into()))
            }
        }
    }
}

async fn ensure_v2_membership(st: &AppState, tenant: &str, user_ids: &[String]) {
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    else {
        return;
    };
    for uid in user_ids {
        let _ = client
            .post(format!("{v2}/v2/tenants/{tenant}/users"))
            .json(&json!({
                "global_user_id": uid,
                "groups": ["grp_eng", "grp_default"],
            }))
            .send()
            .await;
    }
    let _ = client
        .post(format!("{v2}/v2/tenants/{tenant}/users"))
        .json(&json!({
            "global_user_id": "bridge_reader",
            "groups": ["grp_eng", "grp_default"],
        }))
        .send()
        .await;
}

async fn run_scheduled_compiles(st: &AppState) -> anyhow::Result<()> {
    // Scan tenants we know from last_demo + default demo/github tenants
    let mut tenants: Vec<String> = st.last_demo.lock().keys().cloned().collect();
    for t in ["ten_github", "ten_demo", "ten_live", "ten_q", "ten_platform"] {
        if !tenants.iter().any(|x| x == t) {
            tenants.push(t.into());
        }
    }
    let now = Utc::now();
    let (period_start, period_end) = st.cfg.aligned_period(now);
    let (activity_start, activity_end) = st.cfg.activity_lookback(now);
    for tenant in tenants {
        let twins = st.store.list_twins(&tenant).await.unwrap_or_default();
        // Include gu_* aliases so multi-identity digests work on the scheduler path too.
        let membership_ids = membership_user_ids_for_twins(&twins);
        ensure_v2_membership(st, &tenant, &membership_ids).await;
        for twin in twins.into_iter().filter(|t| t.enabled) {
            if twin.twin_kind != TwinKind::Person {
                continue;
            }
            // Clear fixture so overlay hits V2 when available
            if let Some(fx) = st.fixture.as_ref() {
                fx.set_view(
                    &tenant,
                    &twin.subject_id,
                    GraphView {
                        nodes: vec![],
                        edges: vec![],
                        states: vec![],
                        blockers: vec![],
                        graph_as_of: None,
                    },
                );
            }
            let opts = CompileOpts {
                period_start,
                period_end,
                activity_start,
                activity_end,
                hops: 3,
            };
            let outcome = match st.compiler.compile_person(&twin, &opts).await {
                Ok(o) => o,
                Err(e) => {
                    tracing::debug!(twin = %twin.twin_id, error = %e, "schedule compile skip");
                    continue;
                }
            };
            st.metrics.compile_ok.fetch_add(1, Ordering::Relaxed);

            let key = (tenant.clone(), twin.twin_id.clone());
            let should_notify = {
                let map = st.last_notify.lock();
                match map.get(&key) {
                    None => true,
                    Some(last) => {
                        (now - *last).num_seconds() >= st.cfg.notify_interval_secs
                    }
                }
            };
            // Empty medium windows: don't nag
            let empty = outcome.ledger.ledger.items.is_empty()
                && outcome.ledger.ledger.open_blockers.is_empty();
            if empty {
                st.metrics.empty_windows.fetch_add(1, Ordering::Relaxed);
            }
            let allow_notify = should_notify && !empty;

            let service =
                DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
            let del = service
                .start_after_compile_opts(
                    &twin,
                    &outcome.ledger,
                    &outcome.draft_text,
                    now,
                    allow_notify,
                    false, // never force on scheduler — Notify Policy v1
                )
                .await?;
            if del.dm_sent {
                st.last_notify.lock().insert(key, now);
                st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
                info!(
                    twin = %twin.twin_id,
                    ledger = %outcome.ledger.ledger_id,
                    "scheduled status DM sent"
                );
            } else if allow_notify {
                if let Some(reason) = del.suppressed {
                    st.metrics.dms_suppressed.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!(
                        twin = %twin.twin_id,
                        reason,
                        "status DM suppressed (notify policy v1)"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Thin monitor workers (M6): graph delta → conflicts cache; multi-person readiness.
/// Does **not** Slack-DM (ADR-014). Surfaces via `/v3/tenants/{id}/pulse`.
async fn run_thin_monitors(st: &AppState) -> anyhow::Result<()> {
    st.metrics.monitor_ticks.fetch_add(1, Ordering::Relaxed);
    let mut tenants: Vec<String> = st.last_demo.lock().keys().cloned().collect();
    for t in ["ten_github", "ten_demo", "ten_live", "ten_q", "ten_platform"] {
        if !tenants.iter().any(|x| x == t) {
            tenants.push(t.into());
        }
    }
    // Also scan tenants that already have twins
    for t in tenants.clone() {
        let twins = st.store.list_twins(&t).await.unwrap_or_default();
        if twins.is_empty() && !matches!(t.as_str(), "ten_github" | "ten_demo") {
            continue;
        }
        let maps = st.store.list_slack_maps(&t).await.unwrap_or_default();
        let person_twins: Vec<_> = twins
            .iter()
            .filter(|tw| tw.twin_kind == TwinKind::Person && tw.enabled)
            .collect();
        // Unique Slack among enabled person twins (same as Team API — not map-row count).
        let mut uniq_slack: std::collections::HashSet<String> = std::collections::HashSet::new();
        for tw in &person_twins {
            if let Some(m) = maps.iter().find(|m| m.global_user_id == tw.subject_id) {
                if !m.slack_user_id.is_empty() {
                    uniq_slack.insert(m.slack_user_id.clone());
                }
            }
        }
        let mapped = uniq_slack.len();
        // Prefer a real mapped human as ACL reader for V2 conflicts
        let reader = maps
            .first()
            .map(|m| m.global_user_id.clone())
            .or_else(|| person_twins.first().map(|tw| tw.subject_id.clone()))
            .unwrap_or_else(|| "bridge_reader".into());

        let v2 = st.cfg.v2_base_url.trim_end_matches('/');
        let conflicts_url = format!(
            "{v2}/v2/tenants/{t}/conflicts?user_id={}&limit=30",
            urlencoding_simple(&reader)
        );
        let intents_url = format!(
            "{v2}/v2/tenants/{t}/intents?user_id={}&limit=50",
            urlencoding_simple(&reader)
        );
        let conflicts = probe_json(&conflicts_url).await;
        let intents = probe_json(&intents_url).await;
        let all_conflict_cards = conflicts
            .as_ref()
            .and_then(|v| v.get("conflicts"))
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        let all_intent_sample = intents
            .as_ref()
            .and_then(|v| v.get("intents"))
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        // Default product pulse excludes intent_demo seed (alice/bob theater).
        let live_conflicts: Vec<serde_json::Value> = all_conflict_cards
            .iter()
            .filter(|c| !json_looks_like_demo_seed(c))
            .cloned()
            .map(with_is_demo_tag)
            .collect();
        let demo_conflicts: Vec<serde_json::Value> = all_conflict_cards
            .iter()
            .filter(|c| json_looks_like_demo_seed(c))
            .cloned()
            .map(with_is_demo_tag)
            .collect();
        let live_intents: Vec<serde_json::Value> = all_intent_sample
            .iter()
            .filter(|i| !json_looks_like_demo_seed(i))
            .cloned()
            .map(with_is_demo_tag)
            .collect();
        let demo_intents: Vec<serde_json::Value> = all_intent_sample
            .iter()
            .filter(|i| json_looks_like_demo_seed(i))
            .cloned()
            .map(with_is_demo_tag)
            .collect();
        let conflict_count = live_conflicts.len() as u64;
        if conflict_count > 0 {
            st.metrics.conflict_hits.fetch_add(1, Ordering::Relaxed);
        }
        let intent_count = live_intents.len() as u64;
        let v1_base = st.cfg.v1_base_url.trim_end_matches('/');
        let v1_health = probe_json(&format!("{v1_base}/healthz")).await;
        let pulse = json!({
            "tenant_id": t,
            "as_of": Utc::now().to_rfc3339(),
            "team": {
                "person_twins": person_twins.len(),
                "slack_mapped": mapped,
                "unique_slack_users": mapped,
                "multi_person_ready": mapped >= 2 && person_twins.len() >= 2,
            },
            "intents": {
                "count": intent_count,
                "sample": live_intents,
                "demo_count": demo_intents.len(),
            },
            "conflicts": {
                "count": conflict_count,
                "cards": live_conflicts,
                "demo_count": demo_conflicts.len(),
                "demo_cards": demo_conflicts,
                "engine": "rules_v0",
                "note": "Primary cards exclude intent_demo seed; demo_* fields keep Load intent demo visible; is_demo tags on cards",
            },
            "ingest": {
                "v1_up": v1_health.is_some(),
                "v1_accepted": v1_health.as_ref().and_then(|v| v.get("accepted").cloned()),
                "v1_last_accepted_unix": v1_health.as_ref().and_then(|v| v.get("last_accepted_unix").cloned()),
            },
            "monitor": "thin_v0",
        });
        st.last_pulse.lock().insert(t, pulse);
    }
    Ok(())
}

/// True if a conflict/intent JSON blob is from the SHIP-vs-FREEZE intent_demo seed.
fn json_looks_like_demo_seed(v: &serde_json::Value) -> bool {
    let blob = v.to_string().to_ascii_lowercase();
    blob.contains("gu_demo_")
        || blob.contains("demo-repo")
        || blob.contains("intent_demo")
        || blob.contains("seed:intent_demo")
        || blob.contains("\"seed\":\"intent_demo\"")
        || blob.contains("\"is_demo\":true")
        || blob.contains("\"seed\":\"graph_story\"")
        || blob.contains("seed:graph_story")
        // Historical story seed PR used in pilot demos (not multi-repo flywheel work)
        || blob.contains("/pr/story-1")
        || blob.contains("pr:neeljoshi18/ai-manager/pr/story-1")
}

/// Tag a conflict/intent JSON value with `is_demo` for product consumers.
fn with_is_demo_tag(mut v: serde_json::Value) -> serde_json::Value {
    let is_demo = json_looks_like_demo_seed(&v)
        || v.get("is_demo").and_then(|x| x.as_bool()).unwrap_or(false)
        || v.pointer("/properties/seed")
            .and_then(|x| x.as_str())
            .map(|s| s.contains("demo") || s.contains("seed"))
            .unwrap_or(false)
        || v.pointer("/properties/is_demo")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("is_demo".into(), json!(is_demo));
    }
    v
}

fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

async fn healthz() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "twin-api",
        "vertical": 3,
        "demo": "/demo/"
    }))
}

async fn probe(url: &str) -> bool {
    // 5s: small VPS + busy V1 must not false-fail the flywheel status surface
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok();
    let Some(c) = client else {
        return false;
    };
    c.get(url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn probe_json(url: &str) -> Option<serde_json::Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;
    let res = client.get(url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    // Prefer JSON; some services (egress) return plain "ok".
    match res.json().await {
        Ok(v) => Some(v),
        Err(_) => Some(serde_json::json!({ "status": "ok" })),
    }
}

fn env_present(name: &str) -> bool {
    std::env::var(name)
        .map(|v| !v.trim().is_empty() && !v.contains("REPLACE") && !v.contains("example"))
        .unwrap_or(false)
}

/// Onboarding + OAuth readiness (never returns secret values).
async fn onboarding_status(State(st): State<AppState>) -> impl IntoResponse {
    let v1_base = st.cfg.v1_base_url.trim_end_matches('/');
    let v2_base = st.cfg.v2_base_url.trim_end_matches('/');
    let v1 = probe(&format!("{v1_base}/healthz")).await;
    let v2 = probe(&format!("{v2_base}/healthz")).await;
    let egress = match &st.cfg.egress_proxy_url {
        Some(u) => probe(&format!("{}/healthz", u.trim_end_matches('/'))).await,
        None => false,
    };
    let slack_oauth_ready = env_present("SLACK_CLIENT_ID") && env_present("SLACK_CLIENT_SECRET");
    let github_app_ready = env_present("GITHUB_APP_ID") && env_present("GITHUB_APP_CLIENT_ID");
    let public_base = std::env::var("PUBLIC_BASE_URL").unwrap_or_default();
    let public_ok = !public_base.is_empty() && public_base.starts_with("https://");
    let v1_health = if v1 {
        probe_json(&format!("{v1_base}/healthz")).await
    } else {
        None
    };
    let github_ingest_ok = v1_health
        .as_ref()
        .and_then(|v| v.get("accepted").and_then(|x| x.as_u64()))
        .map(|n| n > 0)
        .unwrap_or(false);

    // Multi-person readiness (A2) from default pilot tenant — unique Slack among
    // *enabled person twins* (not alias-only map rows or disabled duplicates).
    let seed_tenant = std::env::var("SEED_TEAM_TENANT").unwrap_or_else(|_| "ten_github".into());
    let maps = st
        .store
        .list_slack_maps(&seed_tenant)
        .await
        .unwrap_or_default();
    let person_twins_list: Vec<Twin> = st
        .store
        .list_twins(&seed_tenant)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
        .collect();
    let person_twins = person_twins_list.len();
    let mut uniq_slack: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in &person_twins_list {
        if let Some(m) = maps.iter().find(|m| m.global_user_id == t.subject_id) {
            if !m.slack_user_id.is_empty() {
                uniq_slack.insert(m.slack_user_id.clone());
            }
        }
    }
    let multi_ready = uniq_slack.len() >= 2 && person_twins >= 2;
    let digests_with_dm = {
        let mut n = 0usize;
        for t in &person_twins_list {
            if let Ok(drafts) = st.store.list_drafts_for_twin(&seed_tenant, &t.twin_id).await {
                if drafts.iter().any(|d| !d.slack_dm_ts.is_empty()) {
                    n += 1;
                }
            }
        }
        n
    };
    let digests_with_content = {
        let mut n = 0usize;
        for t in &person_twins_list {
            if let Ok(drafts) = st.store.list_drafts_for_twin(&seed_tenant, &t.twin_id).await {
                if drafts.iter().any(|d| {
                    !d.draft_text.contains("nothing invented")
                        && !d.draft_text.contains("No code or ticket signals")
                        && d.draft_text.lines().any(|l| l.trim_start().starts_with('•'))
                }) {
                    n += 1;
                }
            }
        }
        n
    };

    // Steps for product wizard (UI drives progression; this is server truth).
    let steps = json!([
        {
            "id": "stack",
            "title": "Stack running",
            "done": v1 && v2,
            "detail": if v1 && v2 { "V1 + V2 reachable" } else { "Start ./scripts/dev_up.sh or docker compose app" }
        },
        {
            "id": "egress",
            "title": "Egress vault",
            "done": egress,
            "detail": if egress { "Proxy up — tokens stay in vault only" } else { "Start egress with secrets/dev_secrets.json" }
        },
        {
            "id": "slack",
            "title": "Connect Slack",
            "done": st.slack_mode == "egress" && egress,
            "detail": if slack_oauth_ready {
                "OAuth credentials present — use Connect Slack"
            } else if st.slack_mode == "egress" {
                "Manual vault token path active (OAuth client not set)"
            } else {
                "Set USE_EGRESS_SLACK + vault token, or SLACK_CLIENT_ID/SECRET"
            }
        },
        {
            "id": "github",
            "title": "Connect GitHub",
            "done": github_ingest_ok || github_app_ready,
            "detail": if github_ingest_ok {
                "V1 has accepted at least one event this process"
            } else if github_app_ready {
                "GitHub App env present — complete install on GitHub"
            } else {
                "Webhook → V1 or set GITHUB_APP_* (see deploy/oauth/)"
            }
        },
        {
            "id": "team",
            "title": "Map ≥2 people",
            "done": multi_ready,
            "detail": format!(
                "{}/2 unique Slack maps · {} person twins on {}",
                uniq_slack.len().min(2),
                person_twins,
                seed_tenant
            )
        },
        {
            "id": "shadow",
            "title": "Shadow / batch notify",
            "done": !st.cfg.notify_on_compile_default && st.cfg.notify_interval_secs > 0,
            "detail": format!(
                "notify every {}s · window {}s · policy=v1 · on_compile={}",
                st.cfg.notify_interval_secs,
                st.cfg.status_window_secs,
                st.cfg.notify_on_compile_default
            )
        },
        {
            "id": "first_dm",
            "title": "First status digests",
            "done": digests_with_dm >= 1,
            "detail": if digests_with_dm >= 2 {
                format!("{digests_with_dm} people have received a DM — multi-person path live")
            } else if digests_with_dm == 1 {
                "1 person has a DM — compile team or wait for windows for the second".to_string()
            } else {
                "Send test status, Team → Compile all digests, or wait for scheduler".to_string()
            }
        },
        {
            "id": "a2_content",
            "title": "Non-empty digests (A2)",
            "done": digests_with_content >= 2,
            "detail": format!(
                "{digests_with_content}/2 people with real digest content · unique Slack {} · window {}s",
                uniq_slack.len(),
                st.cfg.status_window_secs
            )
        }
    ]);

    Json(json!({
        "steps": steps,
        "public_base_url_set": public_ok,
        "slack_oauth_ready": slack_oauth_ready,
        "github_app_ready": github_app_ready,
        "slack_mode": st.slack_mode,
        "delivery_adapter": st.delivery_adapter,
        "pilot": {
            "tenant": seed_tenant,
            "multi_person_ready": multi_ready,
            "unique_slack_users": uniq_slack.len(),
            "person_twins": person_twins,
            "digests_with_dm": digests_with_dm,
            "digests_with_content": digests_with_content,
            "status_window_secs": st.cfg.status_window_secs,
            "notify_policy": "v1_change_only_daily_cap",
        },
        "manifests": {
            "slack": "deploy/oauth/slack-app-manifest.json",
            "github": "deploy/oauth/github-app-manifest.yml",
            "docs": "deploy/oauth/README.md"
        },
        "note": "OAuth install redirects require human-provided client credentials (never in git)."
    }))
}

/// True if vault JSON has a non-empty value for `key` — never returns the secret itself.
fn vault_key_present(key: &str) -> bool {
    let Some(path) = oauth_vault_path() else {
        return false;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Public install status for Connections / Cockpit (never returns secret values).
async fn oauth_status(State(st): State<AppState>) -> impl IntoResponse {
    let public = std::env::var("PUBLIC_BASE_URL").unwrap_or_default();
    let tenant = std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into());
    let slack_oauth = env_present("SLACK_CLIENT_ID") && env_present("SLACK_CLIENT_SECRET");
    let gh_app = env_present("GITHUB_APP_ID") || env_present("GITHUB_APP_SLUG");
    let vault_writable = oauth_vault_path()
        .map(|p| p.parent().map(|d| d.exists()).unwrap_or(false))
        .unwrap_or(false);
    let slack_bot_in_vault = vault_key_present("SLACK_BOT_TOKEN");
    let webhook = if public.starts_with("https://") {
        format!("{public}/v1/tenants/{tenant}/webhooks/github")
    } else {
        format!("https://YOUR_HOST/v1/tenants/{tenant}/webhooks/github")
    };
    let app_slug = std::env::var("GITHUB_APP_SLUG").unwrap_or_else(|_| "ai-manager".into());
    let github_install = if gh_app {
        Some(format!("https://github.com/apps/{app_slug}/installations/new"))
    } else {
        None
    };
    let slack_scopes = vec!["chat:write", "im:write", "users:read"];
    // Checklist: booleans only — safe for champion UI (no secrets).
    let install_checklist = json!([
        {
            "id": "slack_connect",
            "label": "Connect Slack (bot install OAuth)",
            "done": slack_bot_in_vault || slack_oauth,
            "hint": if slack_bot_in_vault {
                "Bot token present in vault — restart egress once after first connect"
            } else if slack_oauth {
                "OAuth credentials ready — click Connect Slack"
            } else {
                "Set SLACK_CLIENT_ID/SECRET or paste SLACK_BOT_TOKEN into vault"
            },
        },
        {
            "id": "slack_bot_channel",
            "label": "Invite bot to team channel (optional — DMs still work)",
            "done": false,
            "hint": "Channel posts need the bot in the channel; digests fall back to DMs for mapped people",
        },
        {
            "id": "github_install",
            "label": "Install GitHub App on org/repos",
            "done": gh_app,
            "hint": if gh_app {
                "App env ready — install on org, copy webhook URL into App settings if needed"
            } else {
                "Set GITHUB_APP_SLUG / GITHUB_APP_ID, or wire webhooks manually to V1"
            },
        },
        {
            "id": "map_team",
            "label": "Map pod under Team (Slack user ids)",
            "done": false,
            "hint": "Team → bulk import or add members so digests know who gets which DM",
        },
        {
            "id": "graph_healthy",
            "label": "Graph healthy + digests compiling",
            "done": false,
            "hint": "After GitHub install, wait for V1→bridge→V2; open Cockpit / Graph",
        },
    ]);
    let next_steps = json!({
        "slack": [
            "Invite the AI Manager bot to your team channel for channel posts (DMs still work if you skip this)",
            "Map your eng pod under Team (Slack user ids)",
            "Open Cockpit for digests and pulse",
        ],
        "github": [
            "Copy the webhook URL into the GitHub App settings if not already set",
            "Install the App on the org/repos that should feed status",
            "Wait ~1 min for graph to fill, then open Graph / Cockpit",
        ],
        "order": [
            "Connect Slack (delivery)",
            "Invite bot to channel (optional; DM fallback works)",
            "Install GitHub App (work signals)",
            "Map pod under Team",
            "Open Cockpit",
        ],
    });
    Json(json!({
        "tenant_id": tenant,
        "public_base_url": public,
        "slack": {
            "oauth_credentials": slack_oauth,
            "bot_token_in_vault": slack_bot_in_vault,
            "scopes": slack_scopes,
            "egress_mode": st.slack_mode,
            "vault_write_path_set": oauth_vault_path().is_some(),
            "vault_parent_exists": vault_writable,
            "manual_path": "vertical-security/secrets/dev_secrets.json → SLACK_BOT_TOKEN (egress only)",
            "manifest": "deploy/oauth/slack-app-manifest.json",
            "callback": format!("{}/v3/oauth/slack/callback", public.trim_end_matches('/')),
            "note": if slack_bot_in_vault {
                "Slack bot token in vault. Restart egress once after first connect so delivery reloads. Invite bot to team channel for channel posts — DMs still work without that."
            } else if slack_oauth {
                "Connect Slack opens authorize URL; callback writes SLACK_BOT_TOKEN to vault if OAUTH_VAULT_PATH set. Restart egress after first OAuth."
            } else {
                "Set SLACK_CLIENT_ID + SLACK_CLIENT_SECRET in deploy/.env.staging, or paste bot token into vault manually."
            },
        },
        "github": {
            "app_env_present": gh_app,
            "install_url": github_install,
            "webhook_url": webhook,
            "manifest": "deploy/oauth/github-app-manifest.yml",
            "manual_path": "Install GitHub App on org/repos; set WEBHOOK_SECRET_ten_github in vault",
            "note": "GitHub = work signals. Webhooks hit V1; graph fills via bridge. No LOC rankings.",
        },
        "teams": teams_oauth_status_json(&st, &public),
        "sso": {
            "status": "roadmap",
            "providers": ["google"],
            "note": "Google/SSO is identity plane only (seats + roles). Still Connect chat + GitHub for data/delivery. Ships with multi-tenant packaging.",
        },
        "delivery_adapter": st.delivery_adapter,
        "delivery_mode": st.slack_mode,
        "next_steps": next_steps,
        "install_checklist": install_checklist,
        "doctrine": {
            "slack": "delivery (digests, Approve / Edit / Don't send)",
            "github": "work signals (PRs, issues, pushes → graph)",
            "not": "LOC rankings, silent 1:1 wiretaps, document search",
        },
    }))
}

fn teams_oauth_status_json(st: &AppState, public: &str) -> serde_json::Value {
    let app_id = env_present("TEAMS_APP_ID");
    let vault_hint = oauth_vault_path().is_some();
    let ready = app_id && (st.delivery_adapter == "teams" || env_present("TEAMS_APP_ID"));
    // "configured" when public app id present; real send needs vault TEAMS_BOT_TOKEN + USE_EGRESS_TEAMS
    let status = if st.delivery_adapter == "teams" && st.slack_mode == "teams" {
        "ready"
    } else if app_id {
        "configured"
    } else {
        "manual"
    };
    json!({
        "status": status,
        "app_id_present": app_id,
        "adapter_active": st.delivery_adapter == "teams",
        "egress_mode": st.slack_mode,
        "vault_write_path_set": vault_hint,
        "messaging_endpoint": format!("{}/v3/teams/messages", public.trim_end_matches('/')),
        "manifest": "deploy/oauth/teams-app-manifest.json",
        "manual_path": "vertical-security/secrets/dev_secrets.json → TEAMS_BOT_TOKEN (egress only)",
        "env": {
            "TEAMS_APP_ID": "public bot app id",
            "TEAMS_TENANT_ID": "optional Azure AD tenant",
            "TEAMS_SERVICE_URL": "optional Bot Framework service URL",
            "DELIVERY_ADAPTER": "teams to select adapter (default slack)",
            "USE_EGRESS_TEAMS": "true for real connector posts",
        },
        "note": if status == "ready" {
            "Teams adapter active — digests use Adaptive Cards (Approve / Edit / Don't send). Map teams_user_id on Team members."
        } else if app_id {
            "TEAMS_APP_ID present. Put TEAMS_BOT_TOKEN in vault, set DELIVERY_ADAPTER=teams + USE_EGRESS_TEAMS=true, restart egress."
        } else {
            "Microsoft Teams: same digests + Approve loop. Set TEAMS_APP_ID + vault TEAMS_BOT_TOKEN, or keep Slack as default."
        },
        "ready": ready && st.delivery_adapter == "teams",
    })
}

async fn oauth_teams_start() -> impl IntoResponse {
    let public = std::env::var("PUBLIC_BASE_URL").unwrap_or_default();
    let app_id = std::env::var("TEAMS_APP_ID").unwrap_or_default();
    if app_id.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({
                "ready": false,
                "error": "teams_not_configured",
                "message": "Set TEAMS_APP_ID (public) and vault TEAMS_BOT_TOKEN. Install the Teams app from deploy/oauth/teams-app-manifest.json in Azure Bot / Teams Developer Portal.",
                "manual_path": "vertical-security/secrets/dev_secrets.json → TEAMS_BOT_TOKEN",
                "manifest": "deploy/oauth/teams-app-manifest.json",
                "messaging_endpoint": format!("{}/v3/teams/messages", public.trim_end_matches('/')),
            })),
        )
            .into_response();
    }
    // Teams admin consent / Azure portal — no single universal OAuth URL like Slack.
    let admin_url = format!(
        "https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationMenuBlade/~/Overview/appId/{}",
        urlencoding_slack(&app_id)
    );
    (
        StatusCode::OK,
        Json(json!({
            "ready": true,
            "app_id": app_id,
            "install_url": admin_url,
            "messaging_endpoint": format!("{}/v3/teams/messages", public.trim_end_matches('/')),
            "manifest": "deploy/oauth/teams-app-manifest.json",
            "note": "Create Azure Bot with this app id; set messaging endpoint above; put Bot Framework token in vault as TEAMS_BOT_TOKEN; DELIVERY_ADAPTER=teams + USE_EGRESS_TEAMS=true; map teams_user_id on members. Restart egress after vault write.",
        })),
    )
        .into_response()
}

fn oauth_vault_path() -> Option<PathBuf> {
    let p = std::env::var("OAUTH_VAULT_PATH")
        .or_else(|_| std::env::var("SECRETS_FILE"))
        .ok()?;
    let p = p.trim();
    if p.is_empty() {
        return None;
    }
    Some(PathBuf::from(p))
}

fn upsert_vault_secret(path: &std::path::Path, key: &str, value: &str) -> Result<(), String> {
    let mut map: serde_json::Map<String, serde_json::Value> = if path.exists() {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        v.as_object().cloned().unwrap_or_default()
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        serde_json::Map::new()
    };
    map.insert(key.to_string(), json!(value));
    let pretty = serde_json::to_string_pretty(&serde_json::Value::Object(map))
        .map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

async fn oauth_slack_start() -> impl IntoResponse {
    if !env_present("SLACK_CLIENT_ID") || !env_present("SLACK_CLIENT_SECRET") {
        // 200 so product UI never looks "broken 501"
        return (
            StatusCode::OK,
            Json(json!({
                "ready": false,
                "error": "slack_oauth_not_configured",
                "message": "Set SLACK_CLIENT_ID and SLACK_CLIENT_SECRET in deploy/.env.staging. Until then use vault SLACK_BOT_TOKEN via egress.",
                "manual_path": "vertical-security/secrets/dev_secrets.json",
                "manifest": "deploy/oauth/slack-app-manifest.json",
            })),
        );
    }
    let client_id = std::env::var("SLACK_CLIENT_ID").unwrap_or_default();
    let redirect = std::env::var("SLACK_REDIRECT_URI").unwrap_or_else(|_| {
        format!(
            "{}/v3/oauth/slack/callback",
            std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "https://REPLACE_ME".into())
        )
    });
    let scopes = "chat:write,im:write,users:read";
    let url = format!(
        "https://slack.com/oauth/v2/authorize?client_id={}&scope={}&redirect_uri={}",
        urlencoding_slack(&client_id),
        urlencoding_slack(scopes),
        urlencoding_slack(&redirect)
    );
    (
        StatusCode::OK,
        Json(json!({
            "ready": true,
            "authorize_url": url,
            "redirect_uri": redirect,
            "note": "Opens Slack install. Callback stores bot token in OAUTH_VAULT_PATH (egress vault). Restart egress after first connect so delivery picks up the token."
        })),
    )
}

#[derive(Deserialize)]
struct SlackOAuthCb {
    code: Option<String>,
    error: Option<String>,
    state: Option<String>,
}

/// Slack OAuth redirect target — exchanges code, writes SLACK_BOT_TOKEN to vault (ADR-012 path).
async fn oauth_slack_callback(Query(q): Query<SlackOAuthCb>) -> impl IntoResponse {
    if let Some(err) = q.error {
        return Html(oauth_html(
            "Slack connect cancelled",
            &format!("Slack returned error: <code>{}</code>", esc_html(&err)),
            false,
        ))
        .into_response();
    }
    let Some(code) = q.code.filter(|c| !c.is_empty()) else {
        return Html(oauth_html(
            "Missing code",
            "No OAuth <code>code</code> query param. Start again from Connections → Connect Slack.",
            false,
        ))
        .into_response();
    };
    if !env_present("SLACK_CLIENT_ID") || !env_present("SLACK_CLIENT_SECRET") {
        return Html(oauth_html(
            "OAuth not configured",
            "Server missing SLACK_CLIENT_ID / SLACK_CLIENT_SECRET.",
            false,
        ))
        .into_response();
    }
    let client_id = std::env::var("SLACK_CLIENT_ID").unwrap_or_default();
    let client_secret = std::env::var("SLACK_CLIENT_SECRET").unwrap_or_default();
    let redirect = std::env::var("SLACK_REDIRECT_URI").unwrap_or_else(|_| {
        format!(
            "{}/v3/oauth/slack/callback",
            std::env::var("PUBLIC_BASE_URL").unwrap_or_default()
        )
    });
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return Html(oauth_html("Client error", &esc_html(&e.to_string()), false)).into_response();
        }
    };
    let res = client
        .post("https://slack.com/api/oauth.v2.access")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code.as_str()),
            ("redirect_uri", redirect.as_str()),
        ])
        .send()
        .await;
    let body: serde_json::Value = match res {
        Ok(r) => r.json().await.unwrap_or(json!({"ok": false, "error": "bad_json"})),
        Err(e) => {
            return Html(oauth_html("Token exchange failed", &esc_html(&e.to_string()), false))
                .into_response();
        }
    };
    if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        let err = body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        return Html(oauth_html(
            "Slack rejected install",
            &format!("<code>{}</code> — check redirect URL matches Slack app settings.", esc_html(err)),
            false,
        ))
        .into_response();
    }
    // Bot token lives under access_token for bot installs (oauth.v2)
    let token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .or_else(|| {
            body.get("bot")
                .and_then(|b| b.get("bot_access_token"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();
    if token.is_empty() || !token.starts_with("xoxb-") {
        return Html(oauth_html(
            "No bot token in response",
            "Install may have been user-only. Ensure bot scopes chat:write,im:write.",
            false,
        ))
        .into_response();
    }
    let team = body
        .get("team")
        .and_then(|t| t.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("workspace");
    // Vault write result — never surface token or vault path (ADR-012).
    // Restart-egress note only when write succeeded (product goal).
    let vault_msg = if let Some(path) = oauth_vault_path() {
        match upsert_vault_secret(&path, "SLACK_BOT_TOKEN", &token) {
            Ok(()) => {
                tracing::info!(team = %team, "slack oauth vault updated");
                "<p class=\"ok\">Bot token saved to the egress vault (never on twin-api env). <strong>Restart the egress container once</strong> so delivery reloads secrets.</p>".to_string()
            }
            Err(e) => {
                tracing::warn!(error = %e, "slack oauth vault write failed");
                "<p class=\"warn\">Token received but vault write failed — paste the bot token into the egress vault manually (<code>SLACK_BOT_TOKEN</code>), then restart egress. No token is shown on this page.</p>".to_string()
            }
        }
    } else {
        "<p class=\"warn\"><code>OAUTH_VAULT_PATH</code> not set — token not written. Paste the bot token into the egress vault manually, then restart egress.</p>".to_string()
    };
    let _ = q.state; // reserved for CSRF later
    Html(oauth_html(
        "Slack connected",
        &format!(
            r#"<p>Workspace <strong>{team}</strong> authorized. Slack is <strong>delivery</strong> — digests with Approve / Edit / Don't send. (GitHub is work signals.)</p>
{vault_msg}
<h2 style="font-size:1.05rem;margin-top:1.25rem">Next steps</h2>
<ol class="steps">
  <li><strong>Invite the bot</strong> to your team channel if you want channel posts. <span class="muted">Skip this and digests still go as DMs to mapped people.</span></li>
  <li><strong>Map your pod</strong> under Team (Slack user ids) so each person gets the right digest.</li>
  <li><strong>Open Cockpit</strong> for digests, pulse, and tomorrow focus.</li>
</ol>
<p class="muted">No secrets are shown on this page. Tokens stay in the egress vault only (ADR-012).</p>"#,
            team = esc_html(team),
            vault_msg = vault_msg,
        ),
        true,
    ))
    .into_response()
}

fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn oauth_html(title: &str, body: &str, ok: bool) -> String {
    let color = if ok { "#111" } else { "#7f1d1d" };
    let connections_href = if ok {
        "/app/?view=connections&connected=slack"
    } else {
        "/app/?view=connections"
    };
    let refresh = if ok {
        format!(
            r#"<meta http-equiv="refresh" content="4;url={connections_href}"/>"#
        )
    } else {
        String::new()
    };
    let redirect_note = if ok {
        format!(
            r#"<p class="muted" id="redir-note">Returning to Connections in a few seconds…</p>
<script>
setTimeout(function(){{ location.href = "{connections_href}"; }}, 3500);
</script>"#
        )
    } else {
        String::new()
    };
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"/><title>{title}</title>
{refresh}
<style>
body{{font-family:ui-sans-serif,system-ui,sans-serif;max-width:40rem;margin:3rem auto;padding:0 1rem;color:#111;line-height:1.45}}
h1{{font-size:1.35rem;color:{color}}}
.muted{{color:#737373;font-size:0.9rem}}
.warn{{color:#9a3412}}
.ok{{color:#14532d}}
ol.steps{{margin:0.5rem 0 1rem 1.2rem;padding:0}}
ol.steps li{{margin:0.4rem 0}}
a.btn{{display:inline-block;margin-top:1rem;padding:0.6rem 1rem;background:#111;color:#fff;text-decoration:none;border-radius:6px}}
a.btn.secondary{{background:#fff;color:#111;border:1px solid #111;margin-left:0.5rem}}
code{{font-size:0.85em}}
</style></head><body>
<h1>{title}</h1>
{body}
<p>
  <a class="btn" href="{connections_href}">Open Connections</a>
  <a class="btn secondary" href="/app/?view=cockpit">Open Cockpit</a>
</p>
{redirect_note}
<p class="muted">AI Manager · tokens never logged · ADR-012 egress vault · Slack = delivery · GitHub = work</p>
</body></html>"#
    )
}

async fn oauth_github_start() -> impl IntoResponse {
    let app_slug = std::env::var("GITHUB_APP_SLUG").unwrap_or_else(|_| "ai-manager".into());
    let public = std::env::var("PUBLIC_BASE_URL").unwrap_or_default();
    let tenant = std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into());
    let webhook = if public.starts_with("https://") {
        format!("{}/v1/tenants/{}/webhooks/github", public.trim_end_matches('/'), tenant)
    } else {
        format!("/v1/tenants/{tenant}/webhooks/github")
    };
    if !env_present("GITHUB_APP_ID") && !env_present("GITHUB_APP_SLUG") {
        return (
            StatusCode::OK,
            Json(json!({
                "ready": false,
                "error": "github_app_not_configured",
                "message": "Set GITHUB_APP_SLUG (and GITHUB_APP_ID) in deploy/.env.staging. Manual webhooks to V1 still work.",
                "webhook_url": webhook,
                "manifest": "deploy/oauth/github-app-manifest.yml",
            })),
        );
    }
    let url = format!("https://github.com/apps/{app_slug}/installations/new");
    (
        StatusCode::OK,
        Json(json!({
            "ready": true,
            "install_url": url,
            "webhook_url": webhook,
            "app_slug": app_slug,
            "note": "Install App on org/repos. Webhooks must hit webhook_url with HMAC secret in vault."
        })),
    )
}

fn urlencoding_slack(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

async fn demo_status(State(st): State<AppState>) -> impl IntoResponse {
    let v1_base = st.cfg.v1_base_url.trim_end_matches('/');
    let v2_base = st.cfg.v2_base_url.trim_end_matches('/');
    let v1_health = probe_json(&format!("{v1_base}/healthz")).await;
    let v1 = v1_health.is_some();
    let v2 = probe(&format!("{v2_base}/healthz")).await;
    let egress = match &st.cfg.egress_proxy_url {
        Some(u) => probe(&format!("{}/healthz", u.trim_end_matches('/'))).await,
        None => false,
    };

    let now = Utc::now().timestamp() as u64;
    let last_accepted_unix = v1_health
        .as_ref()
        .and_then(|v| v.get("last_accepted_unix").and_then(|x| x.as_u64()))
        .filter(|&t| t > 0);
    let accepted = v1_health
        .as_ref()
        .and_then(|v| v.get("accepted").and_then(|x| x.as_u64()));
    let last_event_age_secs = last_accepted_unix.map(|t| now.saturating_sub(t));

    // Graph durability signal (A3): distinguish V2 down vs empty vs filled.
    // Prefer default pilot tenant; embedded stats are cheap.
    let tenant = std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into());
    let (graph_nodes, graph_edges, graph_status, graph_message) = if !v2 {
        (
            None,
            None,
            "v2_down",
            "V2 graph-api down — autoheal restarts; bridge pauses then recovery-mode re-projects",
        )
    } else {
        let stats = probe_json(&format!("{v2_base}/v2/tenants/{tenant}/stats")).await;
        let nodes = stats
            .as_ref()
            .and_then(|v| v.get("nodes").and_then(|x| x.as_u64()));
        let edges = stats
            .as_ref()
            .and_then(|v| v.get("edges").and_then(|x| x.as_u64()));
        match nodes {
            Some(0) => (
                Some(0u64),
                edges,
                "empty",
                "Map empty — bridge recovery mode re-projects V1 events (target <2 min)",
            ),
            Some(n) => (
                Some(n),
                edges,
                "ok",
                "Graph has nodes; live map at /app/ → Graph",
            ),
            None => (
                None,
                None,
                "stats_unavailable",
                "V2 up but stats unreachable — check bridge / graph-api logs",
            ),
        }
    };

    Json(json!({
        "v3": true,
        "v1": v1,
        "v2": v2,
        "egress": egress,
        "mode": st.mode,
        "slack_mode": st.slack_mode,
        "delivery_adapter": st.delivery_adapter,
        "demo": "/demo/",
        "app": "/app/",
        "v1_base_url": v1_base,
        "v2_base_url": v2_base,
        "status_window_secs": st.cfg.status_window_secs,
        "notify_interval_secs": st.cfg.notify_interval_secs,
        "compile_interval_secs": st.cfg.compile_interval_secs,
        "notify_on_compile_default": st.cfg.notify_on_compile_default,
        // Connections health: last successful ingest (not only process up)
        "v1_accepted": accepted,
        "v1_last_accepted_unix": last_accepted_unix,
        "v1_last_event_age_secs": last_event_age_secs,
        // Graph durability (A3) — never show "live" mystery 0/0 without status
        "graph_tenant": tenant,
        "graph_nodes": graph_nodes,
        "graph_edges": graph_edges,
        "graph_status": graph_status,
        "graph_message": graph_message,
        "durability": {
            "v2": probe_json(&format!("{v2_base}/v2/durability")).await,
            "twin_state_path": std::env::var("TWIN_EMBEDDED_STATE_PATH").ok(),
            "note": "Graph/twin survive container restarts via docker volumes; V1 events flush every write",
        },
        "notify_policy": "v1_change_only_daily_cap",
        "slack_oauth_ready": env_present("SLACK_CLIENT_ID") && env_present("SLACK_CLIENT_SECRET"),
        "github_app_ready": env_present("GITHUB_APP_ID"),
    }))
}

#[derive(Deserialize)]
struct DemoSimulateBody {
    tenant_id: Option<String>,
    global_user_id: Option<String>,
    display_name: Option<String>,
    slack_user_id: Option<String>,
    channel_id: Option<String>,
    skip_shadow: Option<bool>,
    pr_title: Option<String>,
    resource_id: Option<String>,
}

async fn demo_simulate(
    State(st): State<AppState>,
    Json(body): Json<DemoSimulateBody>,
) -> Result<impl IntoResponse, ApiError> {
    let tenant = body.tenant_id.unwrap_or_else(|| "ten_demo".into());
    let user = body.global_user_id.unwrap_or_else(|| "gu_alice".into());
    let name = body.display_name.unwrap_or_else(|| "Alice".into());
    let slack_uid = body.slack_user_id.unwrap_or_else(|| "U_DEMO".into());
    let channel = body.channel_id.unwrap_or_else(|| "C_DEMO".into());
    let title = body.pr_title.unwrap_or_else(|| "Demo: fix auth race".into());
    let resource = body
        .resource_id
        .unwrap_or_else(|| "acme/app/pr/7".into());
    let event_id = format!("demo-{}", Utc::now().timestamp());
    let pr_node = format!("pr:{resource}");
    let person = format!("person:{user}");

    // Prefer live V2 project when graph-api is up (real sew); else fixture overlay.
    let v2_up = probe(&format!("{}/healthz", st.cfg.v2_base_url.trim_end_matches('/'))).await;
    let mut source_path = "fixture";
    if v2_up {
        source_path = "v2";
        let client = reqwest::Client::new();
        let _ = client
            .post(format!(
                "{}/v2/tenants/{}/users",
                st.cfg.v2_base_url.trim_end_matches('/'),
                tenant
            ))
            .json(&json!({ "global_user_id": user, "groups": ["grp_eng"] }))
            .send()
            .await;
        let project = json!({
            "event_id": event_id,
            "tenant_id": tenant,
            "provider": "github",
            "category": "code",
            "event_type": "pull_request.opened",
            "event_timestamp": Utc::now().to_rfc3339(),
            "ingested_at": Utc::now().to_rfc3339(),
            "actor": {
                "global_user_id": user,
                "provider_user_id": "42",
                "display_name": name
            },
            "acl": {
                "tenant_id": tenant,
                "allowed_group_ids": ["grp_eng"],
                "is_private": false,
                "acl_version": 1
            },
            "resource_id": resource,
            "parent_resource_id": resource
                .split("/pr/")
                .next()
                .unwrap_or("acme/app"),
            "attributes": { "title": title }
        });
        let _ = client
            .post(format!("{}/v2/project", st.cfg.v2_base_url.trim_end_matches('/')))
            .json(&project)
            .send()
            .await
            .map_err(|e| ApiError::from(TwinError::Upstream(format!("v2 project: {e}"))))?;
        // Clear fixture so OverlayGraphSource reads V2
        if let Some(fx) = st.fixture.as_ref() {
            fx.set_view(
                &tenant,
                &user,
                GraphView {
                    nodes: vec![],
                    edges: vec![],
                    states: vec![],
                    blockers: vec![],
                    graph_as_of: None,
                },
            );
        }
    } else {
        let fixture = st.fixture.as_ref().ok_or_else(|| {
            ApiError::bad("demo simulate needs V2 up or fixture graph (start graph-api or embedded fixtures)")
        })?;
        let view = GraphView {
            nodes: vec![
                GraphNodeView {
                    node_id: person.clone(),
                    node_type: "Person".into(),
                    display_name: name.clone(),
                    resource_id: user.clone(),
                    properties: json!({}),
                    is_private: false,
                },
                GraphNodeView {
                    node_id: pr_node.clone(),
                    node_type: "PullRequest".into(),
                    display_name: title.clone(),
                    resource_id: resource.clone(),
                    properties: json!({ "title": title }),
                    is_private: false,
                },
            ],
            edges: vec![GraphEdgeView {
                edge_id: format!("authored:{event_id}"),
                edge_type: "AUTHORED".into(),
                from_node_id: person,
                to_node_id: pr_node.clone(),
                event_id: event_id.clone(),
                properties: json!({}),
                is_private: false,
                valid_from: Some(Utc::now()),
            }],
            states: vec![EntityStateView {
                node_id: pr_node,
                state_key: "lifecycle".into(),
                state_value: "OPEN".into(),
                event_id: event_id.clone(),
                as_of: Utc::now(),
            }],
            blockers: vec![],
            graph_as_of: Some(Utc::now()),
        };
        fixture.set_view(&tenant, &user, view);
    }

    let now = Utc::now();
    let twin_id = person_twin_id(&user);
    let twin = Twin {
        tenant_id: tenant.clone(),
        twin_id: twin_id.clone(),
        twin_kind: TwinKind::Person,
        subject_id: user.clone(),
        display_name: name,
        timezone: "UTC".into(),
        channel_id: channel,
        shadow_until: if body.skip_shadow.unwrap_or(true) {
            None
        } else {
            Some(now + Duration::days(st.cfg.shadow_mode_days))
        },
        high_auto_publish: false,
        enabled: true,
        config_json: json!({ "demo": true }),
        created_at: now,
        updated_at: now,
    };
    st.store.upsert_twin(twin.clone()).await.map_err(ApiError::from)?;
    st.store
        .put_slack_map(SlackUserMap {
            tenant_id: tenant.clone(),
            global_user_id: user,
            slack_user_id: slack_uid,
            slack_team_id: String::new(),
        })
        .await
        .map_err(ApiError::from)?;

    let (period_start, period_end) = st.cfg.aligned_period(now);
    let (activity_start, activity_end) = st.cfg.activity_lookback(now);
    let opts = CompileOpts {
        period_start,
        period_end,
        activity_start,
        activity_end,
        hops: 2,
    };
    let outcome = st
        .compiler
        .compile_person(&twin, &opts)
        .await
        .map_err(ApiError::from)?;
    st.metrics.compile_ok.fetch_add(1, Ordering::Relaxed);

    // Demo button is an explicit on-demand notify tool (force_notify bypasses daily cap)
    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    let del = service
        .start_after_compile_opts(
            &twin,
            &outcome.ledger,
            &outcome.draft_text,
            now,
            true,
            true,
        )
        .await
        .map_err(ApiError::from)?;
    let draft = del.draft;
    if del.dm_sent {
        st.last_notify
            .lock()
            .insert((tenant.clone(), twin.twin_id.clone()), now);
        st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
    }

    let payload = json!({
        "run_id": outcome.run_id,
        "ledger_id": outcome.ledger.ledger_id,
        "confidence_rollup": outcome.ledger.confidence_rollup,
        "ledger": outcome.ledger.ledger,
        "draft": draft,
        "acl_empty": outcome.acl_empty,
        "event_id": event_id,
        "slack_mode": st.slack_mode,
        "delivery_adapter": st.delivery_adapter,
        "graph_source": source_path,
        "demo": true,
    });
    st.last_demo.lock().insert(tenant, payload.clone());
    Ok(Json(payload))
}

#[derive(Deserialize)]
struct LatestQ {
    tenant_id: Option<String>,
}

async fn demo_latest(
    State(st): State<AppState>,
    Query(q): Query<LatestQ>,
) -> Result<impl IntoResponse, ApiError> {
    let tenant = q.tenant_id.unwrap_or_else(|| "ten_demo".into());
    let cached = {
        let map = st.last_demo.lock();
        map.get(&tenant).cloned()
    };
    match cached {
        Some(v) => {
            let mut out = v;
            let draft_id = out
                .pointer("/draft/draft_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            if let Some(draft_id) = draft_id {
                if let Ok(Some(d)) = st.store.get_draft(&tenant, &draft_id).await {
                    if let Some(obj) = out.as_object_mut() {
                        obj.insert("draft".into(), serde_json::to_value(d).unwrap_or(json!({})));
                    }
                }
            }
            Ok(Json(out))
        }
        None => Err(ApiError::not_found("no demo run yet")),
    }
}

async fn readyz(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({ "status": "ready", "mode": st.mode }))
}

async fn metrics(State(st): State<AppState>) -> impl IntoResponse {
    let dms = st.metrics.drafts_sent.load(Ordering::Relaxed);
    let vetoes = st.metrics.veto_total.load(Ordering::Relaxed);
    let publishes = st.metrics.publish_ok.load(Ordering::Relaxed);
    let decided = vetoes + publishes;
    let veto_rate = if decided == 0 {
        0.0
    } else {
        vetoes as f64 / decided as f64
    };
    Json(json!({
        "service": "twin-api",
        "mode": st.mode,
        "twin_compile_total_ok": st.metrics.compile_ok.load(Ordering::Relaxed),
        "twin_compile_total_error": st.metrics.compile_error.load(Ordering::Relaxed),
        "twin_drafts_sent_total": dms,
        "twin_veto_total": vetoes,
        "twin_publish_total_ok": publishes,
        "twin_publish_total_fail": st.metrics.publish_fail.load(Ordering::Relaxed),
        "twin_acl_empty_total": st.metrics.acl_empty.load(Ordering::Relaxed),
        "twin_egress_fail_total": st.metrics.egress_fail.load(Ordering::Relaxed),
        // M6 beta metrics stubs
        "twin_empty_windows_total": st.metrics.empty_windows.load(Ordering::Relaxed),
        "twin_conflict_hits_total": st.metrics.conflict_hits.load(Ordering::Relaxed),
        "twin_monitor_ticks_total": st.metrics.monitor_ticks.load(Ordering::Relaxed),
        "twin_dms_suppressed_total": st.metrics.dms_suppressed.load(Ordering::Relaxed),
        "twin_veto_rate": veto_rate,
        "twin_dms_sent_total": dms,
        "notify_policy": "v1_change_only_daily_cap",
    }))
}

async fn list_twins_route(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "tenant_id": tenant_id, "twins": twins })))
}

/// Multi-person team map: person twins + Slack destinations (beta gate: ≥2 humans).
async fn get_team(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let maps = st
        .store
        .list_slack_maps(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let map_by: std::collections::HashMap<_, _> = maps
        .iter()
        .map(|m| (m.global_user_id.clone(), m.clone()))
        .collect();
    let mut members = Vec::new();
    for t in twins
        .iter()
        .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
    {
        let slack = map_by.get(&t.subject_id);
        let teams_user_id = t
            .config_json
            .get("teams_user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let role = t
            .config_json
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("champion");
        let aliases = t
            .config_json
            .get("provider_aliases")
            .cloned()
            .unwrap_or(json!([]));
        // Latest draft for multi-person digests board (A2 proof surface)
        let latest = st
            .store
            .list_drafts_for_twin(&tenant_id, &t.twin_id)
            .await
            .ok()
            .and_then(|v| v.into_iter().next());
        let last_digest = latest.as_ref().map(|d| {
            let emptyish = d.draft_text.contains("No code or ticket signals")
                || d.draft_text.contains("nothing invented");
            // Rough item signal from draft bullets (structure-first text)
            let bullet_items = d
                .draft_text
                .lines()
                .filter(|l| l.trim_start().starts_with('•') && !l.contains("nothing invented") && !l.contains("No code or ticket"))
                .count();
            json!({
                "draft_id": d.draft_id,
                "ledger_id": d.ledger_id,
                "status": d.status.as_str(),
                "status_label": match d.status {
                    DraftStatus::Vetoed => "don't send",
                    DraftStatus::Pending => "pending approve",
                    DraftStatus::Edited => "edited",
                    DraftStatus::Published => "shared",
                    DraftStatus::Shadow => "shadow",
                    DraftStatus::PublishQueued => "queued",
                    DraftStatus::ForceHuman => "needs human",
                    _ => d.status.as_str(),
                },
                "dm_sent": !d.slack_dm_ts.is_empty(),
                "slack_dm_ts": d.slack_dm_ts,
                "updated_at": d.updated_at,
                "preview": d.draft_text.chars().take(160).collect::<String>(),
                "empty_placeholder": emptyish,
                "approx_item_count": if emptyish { 0 } else { bullet_items },
                "has_content": !emptyish && bullet_items > 0,
            })
        });
        let chat_mapped = slack.is_some() || !teams_user_id.is_empty();
        members.push(json!({
            "twin_id": t.twin_id,
            "subject_id": t.subject_id,
            "display_name": t.display_name,
            "enabled": t.enabled,
            "channel_id": t.channel_id,
            "slack_user_id": slack.map(|s| s.slack_user_id.clone()),
            "teams_user_id": if teams_user_id.is_empty() { serde_json::Value::Null } else { json!(teams_user_id) },
            "role": role,
            "slack_mapped": slack.is_some(),
            "chat_mapped": chat_mapped,
            "provider_aliases": aliases,
            "shadow_until": t.shadow_until,
            "last_digest": last_digest,
        }));
    }
    // Do not list alias-only slack map rows (login/numeric keys) as extra members —
    // they were creating empty "ghost" rows and inflated multi-person noise.
    let mapped = members
        .iter()
        .filter(|m| m.get("chat_mapped").and_then(|v| v.as_bool()) == Some(true)
            || m.get("slack_mapped").and_then(|v| v.as_bool()) == Some(true))
        .count();
    // Unique chat destinations among *enabled person twins* (Slack or Teams).
    // Same human mapped thrice under one chat id must not count as multi-person.
    let mut uniq_slack: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut enabled_person_twins = 0usize;
    for m in &members {
        let enabled = m.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let has_twin = m.get("twin_id").and_then(|v| v.as_str()).is_some();
        if enabled && has_twin {
            enabled_person_twins += 1;
            if let Some(s) = m.get("slack_user_id").and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    uniq_slack.insert(format!("slack:{s}"));
                }
            }
            if let Some(s) = m.get("teams_user_id").and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    uniq_slack.insert(format!("teams:{s}"));
                }
            }
        }
    }
    let multi_person_ready = uniq_slack.len() >= 2 && enabled_person_twins >= 2;
    // Flatten map for bridge: subject + provider aliases → slack
    let mut bridge_map = serde_json::Map::new();
    for m in &members {
        let slack = m
            .get("slack_user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if slack.is_empty() {
            continue;
        }
        if let Some(sub) = m.get("subject_id").and_then(|v| v.as_str()) {
            if !sub.is_empty() {
                bridge_map.insert(sub.to_string(), json!(slack));
            }
        }
        if let Some(arr) = m.get("provider_aliases").and_then(|v| v.as_array()) {
            for a in arr {
                if let Some(s) = a.as_str() {
                    if !s.is_empty() {
                        bridge_map.insert(s.to_string(), json!(slack));
                    }
                }
            }
        }
    }
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "members": members,
        "person_count": members.len(),
        "slack_mapped_count": mapped,
        "unique_slack_users": uniq_slack.len(),
        "enabled_person_twins": enabled_person_twins,
        "multi_person_ready": multi_person_ready,
        "bridge_slack_map": bridge_map,
        "note": "Map ≥2 humans (distinct Slack user IDs) for multi-member digests. Bridge merges this with SLACK_USER_MAP env.",
    })))
}

#[derive(Deserialize)]
struct TeamMemberBody {
    subject_id: String,
    display_name: Option<String>,
    /// Slack user id (optional when mapping Teams-only member).
    #[serde(default)]
    slack_user_id: Option<String>,
    /// Teams / AAD user id for Teams adapter delivery.
    teams_user_id: Option<String>,
    channel_id: Option<String>,
    /// GitHub login / provider ids that should resolve to this Slack user (bridge map).
    provider_aliases: Option<Vec<String>>,
    enabled: Option<bool>,
    skip_shadow: Option<bool>,
    /// champion | member (stored on twin.config_json.role)
    role: Option<String>,
}

/// Upsert one team member (person twin + Slack map + optional provider aliases).
async fn upsert_team_member(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<TeamMemberBody>,
) -> Result<impl IntoResponse, ApiError> {
    if body.subject_id.trim().is_empty() {
        return Err(ApiError::bad("subject_id required"));
    }
    let slack_uid = body
        .slack_user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let teams_uid = body
        .teams_user_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if slack_uid.is_none() && teams_uid.is_none() {
        return Err(ApiError::bad(
            "slack_user_id or teams_user_id required (chat delivery destination)",
        ));
    }
    let now = Utc::now();
    let twin_id = person_twin_id(&body.subject_id);
    let existing = st
        .store
        .get_twin(&tenant_id, &twin_id)
        .await
        .map_err(ApiError::from)?;
    let mut config = existing
        .as_ref()
        .map(|t| t.config_json.clone())
        .unwrap_or_else(|| json!({}));
    if let Some(tid) = &teams_uid {
        if let Some(obj) = config.as_object_mut() {
            obj.insert("teams_user_id".into(), json!(tid));
        } else {
            config = json!({ "teams_user_id": tid });
        }
    }
    if let Some(role) = body.role.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let role = if role.eq_ignore_ascii_case("champion") {
            "champion"
        } else {
            "member"
        };
        if let Some(obj) = config.as_object_mut() {
            obj.insert("role".into(), json!(role));
        }
    }
    // Merge aliases (never clobber historical gu_* from prior prune/seed).
    if let Some(aliases) = &body.provider_aliases {
        let mut merged: Vec<String> = config
            .get("provider_aliases")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        for a in aliases {
            let a = a.trim();
            if a.is_empty() {
                continue;
            }
            if !merged.iter().any(|x| x == a) {
                merged.push(a.to_string());
            }
        }
        if let Some(obj) = config.as_object_mut() {
            obj.insert("provider_aliases".into(), json!(merged));
        } else {
            config = json!({ "provider_aliases": merged });
        }
    }
    let shadow_until = if body.skip_shadow.unwrap_or(false) || st.cfg.shadow_mode_days <= 0 {
        None
    } else {
        existing
            .as_ref()
            .and_then(|t| t.shadow_until)
            .or_else(|| Some(now + Duration::days(st.cfg.shadow_mode_days)))
    };
    let twin = Twin {
        tenant_id: tenant_id.clone(),
        twin_id: twin_id.clone(),
        twin_kind: TwinKind::Person,
        subject_id: body.subject_id.clone(),
        display_name: body
            .display_name
            .or_else(|| existing.as_ref().map(|t| t.display_name.clone()))
            .unwrap_or_else(|| body.subject_id.clone()),
        timezone: existing
            .as_ref()
            .map(|t| t.timezone.clone())
            .unwrap_or_else(|| "UTC".into()),
        channel_id: body
            .channel_id
            .or_else(|| existing.as_ref().map(|t| t.channel_id.clone()))
            .unwrap_or_default(),
        shadow_until,
        high_auto_publish: existing
            .as_ref()
            .map(|t| t.high_auto_publish)
            .unwrap_or(false),
        enabled: body
            .enabled
            .unwrap_or(existing.as_ref().map(|t| t.enabled).unwrap_or(true)),
        config_json: config,
        created_at: existing.as_ref().map(|t| t.created_at).unwrap_or(now),
        updated_at: now,
    };
    st.store
        .upsert_twin(twin.clone())
        .await
        .map_err(ApiError::from)?;
    if let Some(ref slack_user_id) = slack_uid {
        st.store
            .put_slack_map(SlackUserMap {
                tenant_id: tenant_id.clone(),
                global_user_id: body.subject_id.clone(),
                slack_user_id: slack_user_id.clone(),
                slack_team_id: String::new(),
            })
            .await
            .map_err(ApiError::from)?;
        // Alias keys also map for bridge (login / numeric id)
        if let Some(aliases) = &body.provider_aliases {
            for a in aliases {
                let a = a.trim();
                if a.is_empty() {
                    continue;
                }
                let _ = st
                    .store
                    .put_slack_map(SlackUserMap {
                        tenant_id: tenant_id.clone(),
                        global_user_id: a.to_string(),
                        slack_user_id: slack_user_id.clone(),
                        slack_team_id: String::new(),
                    })
                    .await;
            }
        }
        let _ = prune_duplicate_slack_twins(st.store.as_ref(), &tenant_id).await;
    }
    persist_embedded(&st);
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "twin": twin,
            "slack_user_id": slack_uid,
            "teams_user_id": teams_uid,
            "role": twin.config_json.get("role"),
            "provider_aliases": body.provider_aliases.unwrap_or_default(),
        })),
    ))
}

#[derive(Deserialize)]
struct TeamCompileBody {
    /// When true, force Slack DM (demo). Default false — Notify Policy v1 applies.
    force_notify: Option<bool>,
    /// When true (default), allow_notify if policy permits. Quiet compile when false.
    allow_notify: Option<bool>,
}

/// Compile every enabled person twin in the tenant (multi-person dry-run).
/// Default: notify only if Notify Policy v1 allows (change + daily cap).
async fn compile_team(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    body: Option<Json<TeamCompileBody>>,
) -> Result<impl IntoResponse, ApiError> {
    let force = body
        .as_ref()
        .and_then(|b| b.force_notify)
        .unwrap_or(false);
    let allow = body
        .as_ref()
        .and_then(|b| b.allow_notify)
        .unwrap_or(true);
    let now = Utc::now();
    let (period_start, period_end) = st.cfg.aligned_period(now);
    let (activity_start, activity_end) = st.cfg.activity_lookback(now);
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    let mut results = Vec::new();
    // Ensure V2 membership so neighborhood is ACL-visible for each person twin
    // Also seed any gu_* provider aliases so multi-identity merge can see ACL neighborhoods.
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    if let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        for t in twins.iter().filter(|t| t.enabled && t.twin_kind == TwinKind::Person) {
            let mut guids = vec![t.subject_id.clone()];
            if let Some(arr) = t
                .config_json
                .get("provider_aliases")
                .and_then(|v| v.as_array())
            {
                for a in arr {
                    if let Some(s) = a.as_str() {
                        let s = s.trim();
                        if s.starts_with("gu_") && !guids.iter().any(|x| x == s) {
                            guids.push(s.to_string());
                        }
                    }
                }
            }
            for gid in guids {
                let _ = client
                    .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
                    .json(&json!({
                        "global_user_id": gid,
                        "groups": ["grp_eng", "grp_default"],
                    }))
                    .send()
                    .await;
            }
        }
        let _ = client
            .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
            .json(&json!({
                "global_user_id": "bridge_reader",
                "groups": ["grp_eng", "grp_default"],
            }))
            .send()
            .await;
    }
    for twin in twins
        .into_iter()
        .filter(|t| t.enabled && t.twin_kind == TwinKind::Person)
    {
        if let Some(fx) = st.fixture.as_ref() {
            fx.set_view(
                &tenant_id,
                &twin.subject_id,
                GraphView {
                    nodes: vec![],
                    edges: vec![],
                    states: vec![],
                    blockers: vec![],
                    graph_as_of: None,
                },
            );
        }
        let opts = CompileOpts {
            period_start,
            period_end,
            activity_start,
            activity_end,
            hops: 3,
        };
        let outcome = match st.compiler.compile_person(&twin, &opts).await {
            Ok(o) => o,
            Err(e) => {
                results.push(json!({
                    "twin_id": twin.twin_id,
                    "display_name": twin.display_name,
                    "ok": false,
                    "error": e.to_string(),
                }));
                continue;
            }
        };
        st.metrics.compile_ok.fetch_add(1, Ordering::Relaxed);
        let empty = outcome.ledger.ledger.items.is_empty()
            && outcome.ledger.ledger.open_blockers.is_empty();
        if empty {
            st.metrics.empty_windows.fetch_add(1, Ordering::Relaxed);
        }
        let del = service
            .start_after_compile_opts(
                &twin,
                &outcome.ledger,
                &outcome.draft_text,
                now,
                allow && !empty,
                force,
            )
            .await
            .map_err(ApiError::from)?;
        if del.dm_sent {
            st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
            st.last_notify
                .lock()
                .insert((tenant_id.clone(), twin.twin_id.clone()), now);
        } else if allow && !empty {
            if del.suppressed.is_some() {
                st.metrics.dms_suppressed.fetch_add(1, Ordering::Relaxed);
            }
        }
        let item_kinds: Vec<String> = outcome
            .ledger
            .ledger
            .items
            .iter()
            .map(|i| i.kind.clone())
            .collect();
        let item_summaries: Vec<String> = outcome
            .ledger
            .ledger
            .items
            .iter()
            .take(5)
            .map(|i| i.summary.clone())
            .collect();
        let empty_reason = if empty {
            if outcome.acl_empty {
                Some("no_neighborhood")
            } else {
                Some("no_activity_in_lookback")
            }
        } else {
            None
        };
        let aliases_merged = twin
            .config_json
            .get("provider_aliases")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .filter(|s| s.starts_with("gu_"))
                    .count()
            })
            .unwrap_or(0);
        results.push(json!({
            "twin_id": twin.twin_id,
            "subject_id": twin.subject_id,
            "display_name": twin.display_name,
            "ok": true,
            "ledger_id": outcome.ledger.ledger_id,
            "draft_id": del.draft.draft_id,
            "draft_status": del.draft.status.as_str(),
            "dm_sent": del.dm_sent,
            "suppressed": del.suppressed,
            "empty": empty,
            "empty_reason": empty_reason,
            "item_count": outcome.ledger.ledger.items.len(),
            "blocker_count": outcome.ledger.ledger.open_blockers.len(),
            "item_kinds": item_kinds,
            "item_summaries": item_summaries,
            "preview": outcome.draft_text.chars().take(200).collect::<String>(),
            "confidence": outcome.ledger.ledger.confidence_rollup.as_str(),
            "activity_start": activity_start,
            "activity_end": activity_end,
            "aliases_merged": aliases_merged,
            "acl_empty": outcome.acl_empty,
        }));
    }
    persist_embedded(&st);
    let dms: usize = results
        .iter()
        .filter(|r| r.get("dm_sent").and_then(|v| v.as_bool()) == Some(true))
        .count();
    let with_items: usize = results
        .iter()
        .filter(|r| r.get("item_count").and_then(|v| v.as_u64()).unwrap_or(0) > 0)
        .count();
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "compiled": results.len(),
        "with_items": with_items,
        "dms_sent": dms,
        "force_notify": force,
        "notify_policy": "v1_change_only_daily_cap",
        "status_window_secs": st.cfg.status_window_secs,
        "activity_start": activity_start,
        "activity_end": activity_end,
        "results": results,
        "note": "force_notify=false respects change-only + daily cap. Empty ledgers never DM. Activity uses rolling lookback (STATUS_WINDOW_SECS).",
    })))
}

/// Seed V2 membership for all enabled person twins + gu_* aliases (fix neighborhood 404).
async fn ensure_graph_users(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;
    let mut seeded = Vec::new();
    let mut ids = membership_user_ids_for_twins(&twins);
    ids.push("bridge_reader".into());
    for uid in ids {
        let res = client
            .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
            .json(&json!({
                "global_user_id": uid,
                "groups": ["grp_eng", "grp_default"],
            }))
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => seeded.push(uid),
            Ok(r) => tracing::warn!(user = %uid, status = %r.status(), "v2 seed user non-success"),
            Err(e) => tracing::warn!(user = %uid, error = %e, "v2 seed user failed"),
        }
    }
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "seeded_users": seeded,
        "note": "Ensured grp_eng for subjects + gu_* aliases so multi-identity neighborhoods compile",
    })))
}

/// Machine-readable A1–A7 pilot readiness (no secrets). Use after deploy for A2 go/no-go.
async fn pilot_readiness(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let maps = st
        .store
        .list_slack_maps(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let people: Vec<&Twin> = twins
        .iter()
        .filter(|t| t.enabled && t.twin_kind == TwinKind::Person)
        .collect();
    let mut uniq_slack = std::collections::HashSet::new();
    let mut per_person = Vec::new();
    let mut content_people = 0usize;
    let mut dm_people = 0usize;
    for t in &people {
        let slack = maps
            .iter()
            .find(|m| m.global_user_id == t.subject_id)
            .map(|m| m.slack_user_id.clone())
            .unwrap_or_default();
        if !slack.is_empty() {
            uniq_slack.insert(slack.clone());
        }
        let gu_aliases = twin_alias_list(t)
            .into_iter()
            .filter(|a| a.starts_with("gu_"))
            .count();
        let drafts = st
            .store
            .list_drafts_for_twin(&tenant_id, &t.twin_id)
            .await
            .unwrap_or_default();
        let latest = drafts.first();
        let emptyish = latest
            .map(|d| {
                d.draft_text.contains("nothing invented")
                    || d.draft_text.contains("No code or ticket signals")
            })
            .unwrap_or(true);
        let has_content = latest
            .map(|d| {
                !d.draft_text.contains("nothing invented")
                    && !d.draft_text.contains("No code or ticket signals")
                    && d.draft_text.lines().any(|l| l.trim_start().starts_with('•'))
            })
            .unwrap_or(false);
        let dm_sent = latest.map(|d| !d.slack_dm_ts.is_empty()).unwrap_or(false);
        if has_content {
            content_people += 1;
        }
        if dm_sent {
            dm_people += 1;
        }
        per_person.push(json!({
            "display_name": t.display_name,
            "subject_id": t.subject_id,
            "slack_user_id": slack,
            "slack_mapped": !slack.is_empty(),
            "gu_aliases": gu_aliases,
            "has_content": has_content,
            "empty_placeholder": emptyish,
            "dm_sent": dm_sent,
            "draft_status": latest.map(|d| d.status.as_str()),
        }));
    }
    let multi = uniq_slack.len() >= 2 && people.len() >= 2;
    let a2_live = multi && content_people >= 2;
    let checklist = json!({
        "A1_notify_non_spam": {
            "ok": true,
            "detail": format!(
                "policy=v1 · suppressed={} · sent={}",
                st.metrics.dms_suppressed.load(Ordering::Relaxed),
                st.metrics.drafts_sent.load(Ordering::Relaxed)
            ),
        },
        "A2_multi_person_digests": {
            "ok": a2_live,
            "detail": format!(
                "unique_slack={} content_people={}/2 multi={}",
                uniq_slack.len(),
                content_people,
                multi
            ),
        },
        "A3_graph_durability": {
            "ok": st.embedded_store.is_some() || !st.cfg.is_embedded(),
            "detail": if st.cfg.is_embedded() {
                "embedded twin state path configured"
            } else {
                "cockroach mode"
            },
        },
        "A4_approve_edit_dont_send": { "ok": true, "detail": "product UI language shipped" },
        "A5_install_runbook": { "ok": true, "detail": "Design Partner Install Runbook" },
        "A6_empty_draft_ux": { "ok": true, "detail": "empty banner + no DM" },
        "A7_packaging": { "ok": true, "detail": "one-pager + playbook + soft-outreach checklist" },
    });
    let soft_outreach_ready = a2_live; // hard gate
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "soft_outreach_ready": soft_outreach_ready,
        "multi_person_ready": multi,
        "unique_slack_users": uniq_slack.len(),
        "person_twins": people.len(),
        "content_people": content_people,
        "dm_people": dm_people,
        "status_window_secs": st.cfg.status_window_secs,
        "notify_policy": "v1_change_only_daily_cap",
        "checklist": checklist,
        "members": per_person,
        "note": if soft_outreach_ready {
            "A2 live green — soft outreach allowed"
        } else {
            "Need ≥2 unique Slack + non-empty digests for both humans (deploy + GH activity)"
        },
    })))
}

#[cfg(test)]
mod membership_helper_tests {
    use super::{membership_user_ids_for_twins, merge_aliases_into_twin, twin_alias_list};
    use chrono::Utc;
    use twin_core::ids::person_twin_id;
    use twin_core::model::{Twin, TwinKind};

    fn person(subject: &str, aliases: serde_json::Value) -> Twin {
        let now = Utc::now();
        Twin {
            tenant_id: "ten_t".into(),
            twin_id: person_twin_id(subject),
            twin_kind: TwinKind::Person,
            subject_id: subject.into(),
            display_name: subject.into(),
            timezone: "UTC".into(),
            channel_id: "C1".into(),
            shadow_until: None,
            high_auto_publish: false,
            enabled: true,
            config_json: serde_json::json!({ "provider_aliases": aliases }),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn membership_includes_gu_aliases_only() {
        let t = person(
            "gu_new",
            serde_json::json!(["neeljoshi18", "222674398", "gu_old"]),
        );
        let ids = membership_user_ids_for_twins(&[t]);
        assert!(ids.contains(&"gu_new".to_string()));
        assert!(ids.contains(&"gu_old".to_string()));
        assert!(!ids.iter().any(|x| x == "neeljoshi18"));
    }

    #[test]
    fn merge_aliases_dedupes() {
        let mut t = person("gu_a", serde_json::json!(["gu_old"]));
        merge_aliases_into_twin(
            &mut t,
            ["gu_old".into(), "gu_mid".into(), "login".into()],
        );
        let a = twin_alias_list(&t);
        assert_eq!(a.iter().filter(|x| *x == "gu_old").count(), 1);
        assert!(a.iter().any(|x| x == "gu_mid"));
        assert!(a.iter().any(|x| x == "login"));
    }
}

/// Collapse multiple person twins that share one Slack user (floating graph ghosts).
async fn prune_team_duplicates(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let n = prune_duplicate_slack_twins(st.store.as_ref(), &tenant_id).await;
    persist_embedded(&st);
    let team = st.store.list_twins(&tenant_id).await.map_err(ApiError::from)?;
    let enabled = team
        .iter()
        .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
        .count();
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "pruned": n,
        "enabled_person_twins": enabled,
        "note": "Disabled duplicate twins for the same Slack user. Graph overlay skips disabled.",
    })))
}

/// Proxy V2 intent/conflict seed so product UI can show Team blockers without raw V2 access.
async fn seed_intent_demo_proxy(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let url = format!("{v2}/v2/tenants/{tenant_id}/seed/intent_demo");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;
    let res = client
        .post(&url)
        .send()
        .await
        .map_err(|e| ApiError::from(TwinError::Upstream(format!("v2 seed: {e}"))))?;
    let status = res.status();
    let body: serde_json::Value = res
        .json()
        .await
        .unwrap_or_else(|_| json!({ "error": "bad_json" }));
    if !status.is_success() {
        return Err(ApiError::from(TwinError::Upstream(format!(
            "v2 seed HTTP {status}: {body}"
        ))));
    }
    // Refresh pulse cache so Today blockers update immediately
    let _ = run_thin_monitors(&st).await;
    Ok(Json(body))
}

/// Seed graph activity for every enabled person twin that has no neighborhood (dual digests).
/// Uses real gu_* subjects — not alice/bob — so team/compile fills 2/N when GH is sparse.
async fn seed_dual_digests(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;

    let mut need_seed: Vec<serde_json::Value> = Vec::new();
    let mut already_have = 0usize;
    for t in twins
        .iter()
        .filter(|t| t.enabled && t.twin_kind == TwinKind::Person)
    {
        // Membership first
        let _ = client
            .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
            .json(&json!({
                "global_user_id": t.subject_id,
                "groups": ["grp_eng", "grp_default"],
            }))
            .send()
            .await;
        // Probe neighborhood
        let pn = format!("person:{}", t.subject_id);
        let nb_url = format!(
            "{v2}/v2/tenants/{tenant_id}/neighborhood?user_id={}&node_id={}&hops=2",
            urlencoding_subject(&t.subject_id),
            urlencoding_subject(&pn)
        );
        let empty = match client.get(&nb_url).send().await {
            Ok(r) if r.status().is_success() => {
                let body: serde_json::Value = r.json().await.unwrap_or(json!({}));
                let edges = body
                    .get("edges")
                    .and_then(|e| e.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                edges == 0
            }
            _ => true,
        };
        if empty {
            let provider = t
                .config_json
                .get("provider_aliases")
                .and_then(|a| a.as_array())
                .and_then(|a| a.iter().find_map(|x| x.as_str()))
                .unwrap_or(t.display_name.as_str());
            need_seed.push(json!({
                "global_user_id": t.subject_id,
                "display_name": t.display_name,
                "provider_user_id": provider,
            }));
        } else {
            already_have += 1;
        }
    }

    let mut seed_body = json!({ "seeded": false, "subjects": [] });
    if !need_seed.is_empty() {
        let url = format!("{v2}/v2/tenants/{tenant_id}/seed/team_activity");
        let res = client
            .post(&url)
            .json(&json!({
                "subjects": need_seed,
                "repo": "neeljoshi18/AI-Manager",
                "commits_per_subject": 3,
            }))
            .send()
            .await
            .map_err(|e| ApiError::from(TwinError::Upstream(format!("v2 team seed: {e}"))))?;
        let status = res.status();
        seed_body = res
            .json()
            .await
            .unwrap_or_else(|_| json!({ "error": "bad_json" }));
        if !status.is_success() {
            return Err(ApiError::from(TwinError::Upstream(format!(
                "v2 team seed HTTP {status}: {seed_body}"
            ))));
        }
    }

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "twins_with_edges": already_have,
        "seeded_subjects": need_seed.len(),
        "seed": seed_body,
        "note": "Call team/compile after this for dual digests. Seed only fills empty neighborhoods.",
    })))
}

fn urlencoding_subject(s: &str) -> String {
    // Minimal encode for path/query (gu_ uuid + person: prefix)
    s.replace(':', "%3A")
}

/// Seed a readable dual-person intent/PR story on real team gu_* for the Graph map.
async fn seed_graph_story(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let people: Vec<serde_json::Value> = twins
        .iter()
        .filter(|t| t.enabled && t.twin_kind == TwinKind::Person)
        .take(4)
        .map(|t| {
            let provider = t
                .config_json
                .get("provider_aliases")
                .and_then(|a| a.as_array())
                .and_then(|a| a.iter().find_map(|x| x.as_str()))
                .unwrap_or(t.display_name.as_str());
            json!({
                "global_user_id": t.subject_id,
                "display_name": t.display_name,
                "provider_user_id": provider,
            })
        })
        .collect();
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;
    // membership for each
    for p in &people {
        if let Some(gid) = p.get("global_user_id").and_then(|x| x.as_str()) {
            let _ = client
                .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
                .json(&json!({
                    "global_user_id": gid,
                    "groups": ["grp_eng", "grp_default"],
                }))
                .send()
                .await;
        }
    }
    let url = format!("{v2}/v2/tenants/{tenant_id}/seed/graph_story");
    let res = client
        .post(&url)
        .json(&json!({
            "subjects": people,
            "repo": "neeljoshi18/AI-Manager",
        }))
        .send()
        .await
        .map_err(|e| ApiError::from(TwinError::Upstream(format!("v2 graph story: {e}"))))?;
    let status = res.status();
    let body: serde_json::Value = res
        .json()
        .await
        .unwrap_or_else(|_| json!({ "error": "bad_json" }));
    if !status.is_success() {
        return Err(ApiError::from(TwinError::Upstream(format!(
            "v2 graph story HTTP {status}: {body}"
        ))));
    }
    let _ = run_thin_monitors(&st).await;
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "people_seeded": people.len(),
        "story": body,
        "note": "Open Graph view — people, PR, SHIP/FREEZE intents, BLOCKS. Hide demo stays on.",
    })))
}

#[derive(Deserialize)]
struct PulseQ {
    tenant_id: Option<String>,
    refresh: Option<bool>,
}

async fn get_pulse(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<PulseQ>,
) -> Result<impl IntoResponse, ApiError> {
    let _ = q.tenant_id;
    if q.refresh.unwrap_or(false) {
        let _ = run_thin_monitors(&st).await;
    }
    if let Some(p) = st.last_pulse.lock().get(&tenant_id).cloned() {
        return Ok(Json(p));
    }
    // Lazy one-shot if scheduler has not run
    let _ = run_thin_monitors(&st).await;
    match st.last_pulse.lock().get(&tenant_id).cloned() {
        Some(p) => Ok(Json(p)),
        None => Ok(Json(json!({
            "tenant_id": tenant_id,
            "team": { "multi_person_ready": false, "person_twins": 0, "slack_mapped": 0 },
            "conflicts": { "count": 0, "cards": [] },
            "intents": { "count": 0, "sample": [] },
            "note": "No pulse yet — add team members and ingest GitHub events.",
        }))),
    }
}

#[derive(Deserialize)]
struct ConflictsProxyQ {
    user_id: Option<String>,
    limit: Option<usize>,
}

/// Proxy V2 conflicts for product UI (uses first team member as ACL reader).
async fn get_conflicts_proxy(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<ConflictsProxyQ>,
) -> Result<impl IntoResponse, ApiError> {
    let maps = st
        .store
        .list_slack_maps(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let reader = q
        .user_id
        .filter(|s| !s.is_empty())
        .or_else(|| maps.first().map(|m| m.global_user_id.clone()))
        .or_else(|| {
            twins
                .iter()
                .find(|t| t.twin_kind == TwinKind::Person)
                .map(|t| t.subject_id.clone())
        })
        .unwrap_or_else(|| "bridge_reader".into());
    let limit = q.limit.unwrap_or(30);
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/conflicts?user_id={}&limit={limit}",
        urlencoding_simple(&reader)
    );
    match probe_json(&url).await {
        Some(v) => Ok(Json(v)),
        None => Ok(Json(json!({
            "tenant_id": tenant_id,
            "count": 0,
            "conflicts": [],
            "error": "v2_unreachable_or_empty",
            "reader": reader,
        }))),
    }
}

#[derive(Deserialize)]
struct GraphSnapshotQ {
    user_id: Option<String>,
    node_limit: Option<usize>,
    edge_limit: Option<usize>,
    /// Pass-through to V2; default hide intent_demo seed (alice/bob).
    include_demo: Option<bool>,
    /// Overlay don't-send + pending digests (approved always overlaid when present).
    show_unapproved: Option<bool>,
}


/// Dev insights: activity heat from the live graph (data is currency).
async fn dev_insights(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;
    // Ensure membership then snapshot as bridge_reader
    let _ = client
        .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
        .json(&json!({
            "global_user_id": "bridge_reader",
            "groups": ["grp_eng", "grp_default"],
        }))
        .send()
        .await;
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/snapshot?user_id=bridge_reader&node_limit=800&edge_limit=2000&include_demo=false"
    );
    let snap: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?
        .error_for_status()
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?
        .json()
        .await
        .map_err(|e| ApiError::from(TwinError::Upstream(e.to_string())))?;

    let nodes = snap.get("nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default();
    let edges = snap.get("edges").and_then(|n| n.as_array()).cloned().unwrap_or_default();

    let mut by_type: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut commits: Vec<serde_json::Value> = Vec::new();
    let mut people: Vec<String> = Vec::new();
    for n in &nodes {
        let ty = n.get("type").and_then(|x| x.as_str()).unwrap_or("?").to_string();
        *by_type.entry(ty.clone()).or_insert(0) += 1;
        if ty == "Commit" {
            commits.push(n.clone());
        }
        if ty == "Person" {
            if let Some(lab) = n.get("label").and_then(|x| x.as_str()) {
                people.push(lab.to_string());
            }
        }
    }

    // Hour-of-day / day-of-week from edge valid_from (activity currency)
    let mut hour_hist = vec![0u64; 24];
    let mut dow_hist = vec![0u64; 7]; // Mon=0
    let mut by_day: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut authored_by: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut push_count = 0u64;
    let mut authored_count = 0u64;

    // Map person id -> label
    let mut person_label: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for n in &nodes {
        if n.get("type").and_then(|x| x.as_str()) == Some("Person") {
            if let (Some(id), Some(lab)) = (
                n.get("id").and_then(|x| x.as_str()),
                n.get("label").and_then(|x| x.as_str()),
            ) {
                person_label.insert(id.to_string(), lab.to_string());
            }
        }
    }

    for e in &edges {
        let et = e.get("type").and_then(|x| x.as_str()).unwrap_or("");
        if et == "AUTHORED" {
            authored_count += 1;
            let from = e.get("from").and_then(|x| x.as_str()).unwrap_or("");
            let who = person_label.get(from).cloned().unwrap_or_else(|| from.to_string());
            *authored_by.entry(who).or_insert(0) += 1;
        }
        if et == "PUSHED_TO" {
            push_count += 1;
        }
        if let Some(vf) = e.get("valid_from").and_then(|x| x.as_str()) {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(vf) {
                let utc = dt.with_timezone(&chrono::Utc);
                hour_hist[utc.hour() as usize] += 1;
                // chrono weekday Mon=0..Sun=6 matches our array
                dow_hist[utc.weekday().num_days_from_monday() as usize] += 1;
                let day = utc.format("%Y-%m-%d").to_string();
                *by_day.entry(day).or_insert(0) += 1;
            }
        }
    }

    // Peak hour
    let (peak_hour, peak_hour_n) = hour_hist
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| *n)
        .map(|(h, n)| (h, *n))
        .unwrap_or((0, 0));
    let dow_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let (peak_dow_i, peak_dow_n) = dow_hist
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| *n)
        .map(|(i, n)| (i, *n))
        .unwrap_or((0, 0));

    // Recent commits: prefer message/title from graph properties (data currency)
    let mut recent_commits: Vec<serde_json::Value> = commits
        .iter()
        .map(|n| {
            let msg = n
                .get("message")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| n.get("title").and_then(|x| x.as_str()).filter(|s| !s.is_empty()))
                .unwrap_or("");
            let sha7 = n
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            json!({
                "id": n.get("id"),
                "sha7": sha7,
                "message": msg,
                "title": msg,
                "resource_id": n.get("resource_id"),
            })
        })
        .collect();
    // Prefer commits that have real messages first
    recent_commits.sort_by(|a, b| {
        let am = a.get("message").and_then(|x| x.as_str()).unwrap_or("").len();
        let bm = b.get("message").and_then(|x| x.as_str()).unwrap_or("").len();
        bm.cmp(&am)
    });
    recent_commits.truncate(40);

    // Team digest content signals
    let twins = st.store.list_twins(&tenant_id).await.unwrap_or_default();
    let mut content_people = 0usize;
    let mut person_twins = 0usize;
    for tw in twins.iter().filter(|t| t.enabled && t.twin_kind == TwinKind::Person) {
        person_twins += 1;
        if let Ok(drafts) = st.store.list_drafts_for_twin(&tenant_id, &tw.twin_id).await {
            if drafts.iter().any(|d| {
                !d.draft_text.contains("nothing invented")
                    && d.draft_text.lines().any(|l| l.trim_start().starts_with('•'))
            }) {
                content_people += 1;
            }
        }
    }

    let hour_labels: Vec<String> = (0..24).map(|h| format!("{h:02}:00")).collect();
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "as_of": Utc::now().to_rfc3339(),
        "doctrine": "data_is_currency",
        "graph": {
            "nodes": nodes.len(),
            "edges": edges.len(),
            "by_type": by_type,
            "people": people,
            "commit_nodes": commits.len(),
        },
        "activity": {
            "authored_edges": authored_count,
            "push_edges": push_count,
            "by_author": authored_by,
            "by_day": by_day,
            "hour_of_day_utc": {
                "labels": hour_labels,
                "counts": hour_hist,
                "peak_hour_utc": peak_hour,
                "peak_count": peak_hour_n,
            },
            "day_of_week_utc": {
                "labels": dow_names,
                "counts": dow_hist,
                "peak_day": dow_names[peak_dow_i],
                "peak_count": peak_dow_n,
            },
            "insight": format!(
                "Most active hour (UTC): {:02}:00 ({} events). Peak day: {} ({}).",
                peak_hour, peak_hour_n, dow_names[peak_dow_i], peak_dow_n
            ),
        },
        "digests": {
            "person_twins": person_twins,
            "people_with_content": content_people,
        },
        "recent_commits": recent_commits,
        "note": "Stats derived from ACL-filtered graph edges (valid_from). Commit poller + webhooks fill the graph.",
    })))
}


/// Product Graph map: ACL-safe V2 snapshot + team members for multi-person view.
async fn get_graph_snapshot(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<GraphSnapshotQ>,
) -> Result<impl IntoResponse, ApiError> {
    let maps = st
        .store
        .list_slack_maps(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let twins = st
        .store
        .list_twins(&tenant_id)
        .await
        .map_err(ApiError::from)?;
    let reader = q
        .user_id
        .filter(|s| !s.is_empty())
        .or_else(|| maps.first().map(|m| m.global_user_id.clone()))
        .or_else(|| {
            twins
                .iter()
                .find(|t| t.twin_kind == TwinKind::Person)
                .map(|t| t.subject_id.clone())
        })
        // Prefer eng-seeded bridge reader so private-repo exhaust is visible in Graph.
        .unwrap_or_else(|| "bridge_reader".into());
    let node_limit = q.node_limit.unwrap_or(400);
    let edge_limit = q.edge_limit.unwrap_or(800);
    let include_demo = q.include_demo.unwrap_or(false);
    let show_unapproved = q.show_unapproved.unwrap_or(false);
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let v2_up = probe(&format!("{v2}/healthz")).await;

    // Seed membership for reader (POST) + bridge_reader so snapshots see eng groups
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok();
    if let Some(c) = &client {
        for uid in [&reader, "bridge_reader"] {
            let _ = c
                .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
                .json(&json!({
                    "global_user_id": uid,
                    "groups": ["grp_eng", "grp_default"],
                }))
                .send()
                .await;
        }
    }
    // Prefer bridge_reader for snapshot ACL (grp_eng) when V2 is up.
    let snap_reader = if v2_up {
        "bridge_reader".to_string()
    } else {
        reader.clone()
    };
    let demo_q = if include_demo { "true" } else { "false" };
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/snapshot?user_id={}&node_limit={node_limit}&edge_limit={edge_limit}&include_demo={demo_q}",
        urlencoding_simple(&snap_reader)
    );
    // Snapshot can be larger than health probes — longer timeout than probe_json (2s).
    let snap_raw = if let Some(c) = &client {
        match c.get(&url).send().await {
            Ok(res) if res.status().is_success() => res.json().await.ok(),
            Ok(res) => {
                tracing::debug!(status = %res.status(), "graph snapshot non-success");
                None
            }
            Err(e) => {
                tracing::debug!(error = %e, "graph snapshot request failed");
                None
            }
        }
    } else {
        probe_json(&url).await
    };
    let mut snap = match snap_raw {
        Some(v) => v,
        None => {
            let msg = if !v2_up {
                "V2 graph-api is down or unhealthy. Bridge pauses projections until /healthz recovers; autoheal restarts wedged containers."
            } else {
                "V2 is up but snapshot failed (ACL/empty). Wait for bridge re-project, or check bridge logs."
            };
            return Ok(Json(json!({
                "tenant_id": tenant_id,
                "reader": snap_reader,
                "v2_up": v2_up,
                "status": if v2_up { "empty_or_error" } else { "v2_down" },
                "message": msg,
                "nodes": [],
                "edges": [],
                "totals": { "nodes": 0, "edges": 0 },
                "returned": { "nodes": 0, "edges": 0 },
                "team": { "members": [] },
                "error": if v2_up { "snapshot_failed" } else { "v2_unreachable" },
                "as_of": Utc::now().to_rfc3339(),
                "live": true,
                "include_demo": include_demo,
                "poll_hint_secs": 5,
            })));
        }
    };

    // Belt-and-suspenders: hide demo seed even if V2 predates include_demo.
    if !include_demo {
        filter_demo_from_snapshot_json(&mut snap);
    }

    // Overlay: only *enabled* person twins (disabled = pruned duplicates)
    let mut team_members = Vec::new();
    for t in twins
        .iter()
        .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
    {
        let slack = maps
            .iter()
            .find(|m| m.global_user_id == t.subject_id)
            .map(|m| m.slack_user_id.clone());
        team_members.push(json!({
            "subject_id": t.subject_id,
            "display_name": t.display_name,
            "twin_id": t.twin_id,
            "slack_mapped": slack.is_some(),
            "slack_user_id": slack,
            "person_node_id": format!("person:{}", t.subject_id),
        }));
    }
    if let Some(obj) = snap.as_object_mut() {
        obj.insert("team".into(), json!({ "members": team_members }));
        obj.insert("live".into(), json!(true));
        obj.insert("v2_up".into(), json!(v2_up));
        obj.insert("status".into(), json!("ok"));
        obj.insert("reader".into(), json!(snap_reader));
        obj.insert("include_demo".into(), json!(include_demo));
        obj.insert(
            "poll_hint_secs".into(),
            json!(5),
        );
        // Overlay team people only when no graph Person already represents them
        // (same label / resource_id / subject) — avoids floating duplicate Neels.
        if let Some(nodes) = obj.get_mut("nodes").and_then(|n| n.as_array_mut()) {
            let mut existing_ids: std::collections::HashSet<String> = nodes
                .iter()
                .filter_map(|n| n.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
                .collect();
            let mut existing_labels: std::collections::HashSet<String> = nodes
                .iter()
                .filter(|n| n.get("type").and_then(|t| t.as_str()) == Some("Person"))
                .filter_map(|n| {
                    n.get("label")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_ascii_lowercase())
                })
                .collect();
            let existing_resources: std::collections::HashSet<String> = nodes
                .iter()
                .filter(|n| n.get("type").and_then(|t| t.as_str()) == Some("Person"))
                .filter_map(|n| {
                    n.get("resource_id")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            for t in twins
                .iter()
                .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
            {
                let pid = format!("person:{}", t.subject_id);
                if existing_ids.contains(&pid) {
                    continue;
                }
                let label = if t.display_name.is_empty() {
                    t.subject_id.clone()
                } else {
                    t.display_name.clone()
                };
                let label_l = label.to_ascii_lowercase();
                // Skip if another person already has this login label or provider id
                if existing_labels.contains(&label_l) {
                    continue;
                }
                let aliases: Vec<String> = t
                    .config_json
                    .get("provider_aliases")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                if aliases.iter().any(|a| {
                    existing_resources.contains(a)
                        || existing_labels.contains(&a.to_ascii_lowercase())
                }) {
                    continue;
                }
                if existing_resources.contains(&t.subject_id) {
                    continue;
                }
                nodes.push(json!({
                    "id": pid,
                    "type": "Person",
                    "label": label,
                    "resource_id": t.subject_id,
                    "intent_type": "",
                    "title": "",
                    "is_private": false,
                    "from_team_map": true,
                }));
                existing_ids.insert(format!("person:{}", t.subject_id));
                existing_labels.insert(label_l);
            }
            // Drop disabled team-map ghosts if any leaked earlier (label-only cleanup not possible server-side for V2 store)
            // Filter out Person nodes that are clearly duplicate labels: keep the one with real edges preference client-side.
            // Soft: mark secondary same-label people for UI
            let mut label_first: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for n in nodes.iter_mut() {
                if n.get("type").and_then(|t| t.as_str()) != Some("Person") {
                    continue;
                }
                let lab = n
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if lab.is_empty() {
                    continue;
                }
                let id = n.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                if let Some(first) = label_first.get(&lab) {
                    if first != &id {
                        if let Some(obj) = n.as_object_mut() {
                            obj.insert("duplicate_person".into(), json!(true));
                            obj.insert("duplicate_of".into(), json!(first));
                        }
                    }
                } else {
                    label_first.insert(lab, id);
                }
            }
        }

        // Overlay status digests (twin store) onto graph — after nodes borrow ends.
        // Approved always; pending/don't-send when show_unapproved.
        // Work (commits/PRs) always from GitHub V2 — not gated by Approve.
        let mut digest_nodes: Vec<serde_json::Value> = Vec::new();
        let mut digest_edges: Vec<serde_json::Value> = Vec::new();
        let mut digest_meta = Vec::new();
        let person_nodes: Vec<(String, String, String)> = obj
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|n| n.get("type").and_then(|t| t.as_str()) == Some("Person"))
                    .filter_map(|n| {
                        Some((
                            n.get("id")?.as_str()?.to_string(),
                            n.get("label")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                            n.get("resource_id")
                                .and_then(|x| x.as_str())
                                .unwrap_or("")
                                .to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for t in twins
            .iter()
            .filter(|t| t.twin_kind == TwinKind::Person && t.enabled)
        {
            let drafts = st
                .store
                .list_drafts_for_twin(&tenant_id, &t.twin_id)
                .await
                .unwrap_or_default();
            let Some(d) = drafts.into_iter().next() else {
                continue;
            };
            let st_s = d.status.as_str();
            let is_approved = st_s == "published";
            let is_veto = st_s == "vetoed";
            let is_open = matches!(
                st_s,
                "pending" | "edited" | "force_human" | "publish_queued" | "publish_failed"
            );
            if !is_approved && !show_unapproved {
                digest_meta.push(json!({
                    "twin_id": t.twin_id,
                    "subject_id": t.subject_id,
                    "display_name": t.display_name,
                    "draft_id": d.draft_id,
                    "status": st_s,
                    "hidden": true,
                }));
                continue;
            }
            if !is_approved && !is_veto && !is_open {
                continue;
            }
            let decision = if is_approved {
                "approved"
            } else if is_veto {
                "dont_send"
            } else {
                "unapproved"
            };
            let nid = format!("digest:{}", d.draft_id);
            let preview: String = d.draft_text.chars().take(80).collect();
            digest_nodes.push(json!({
                "id": nid,
                "type": "StatusDigest",
                "label": format!("{decision}: {}", t.display_name),
                "resource_id": d.draft_id,
                "title": preview,
                "intent_type": decision,
                "decision": decision,
                "draft_status": st_s,
                "is_private": is_veto,
                "from_digest_overlay": true,
            }));
            let person_id = format!("person:{}", t.subject_id);
            let from_id = person_nodes
                .iter()
                .find(|(_, lab, res)| {
                    res == &t.subject_id
                        || lab.eq_ignore_ascii_case(&t.display_name)
                        || lab.eq_ignore_ascii_case(&t.subject_id)
                })
                .map(|(id, _, _)| id.clone())
                .unwrap_or(person_id);
            digest_edges.push(json!({
                "id": format!("edge:decided:{}", d.draft_id),
                "from": from_id,
                "to": nid,
                "type": "STATUS_DECISION",
                "from_digest_overlay": true,
            }));
            digest_meta.push(json!({
                "twin_id": t.twin_id,
                "subject_id": t.subject_id,
                "display_name": t.display_name,
                "draft_id": d.draft_id,
                "status": st_s,
                "decision": decision,
                "hidden": false,
            }));
        }
        if let Some(nodes) = obj.get_mut("nodes").and_then(|n| n.as_array_mut()) {
            nodes.extend(digest_nodes);
        }
        if let Some(edges) = obj.get_mut("edges").and_then(|e| e.as_array_mut()) {
            edges.extend(digest_edges);
        } else if !digest_edges.is_empty() {
            obj.insert("edges".into(), json!(digest_edges));
        }
        obj.insert("digest_overlay".into(), json!(digest_meta));
        obj.insert("show_unapproved".into(), json!(show_unapproved));
        obj.insert(
            "digest_note".into(),
            json!(
                "GitHub work always on graph. StatusDigest nodes = Approve / Don't send outcomes from twin store. Check “Show unapproved” for don't-send + pending."
            ),
        );
    }
    Ok(Json(snap))
}

/// Remove intent_demo seed nodes/edges from a V2 snapshot JSON body (product Graph default).
fn filter_demo_from_snapshot_json(snap: &mut serde_json::Value) {
    let Some(obj) = snap.as_object_mut() else {
        return;
    };
    let Some(nodes) = obj.get("nodes").and_then(|n| n.as_array()) else {
        return;
    };
    let mut drop_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in nodes {
        let id = n.get("id").and_then(|x| x.as_str()).unwrap_or("");
        let label = n
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let resource = n
            .get("resource_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let ntype = n.get("type").and_then(|x| x.as_str()).unwrap_or("");
        let id_l = id.to_ascii_lowercase();
        let is_demo = id_l.contains("gu_demo_")
            || id_l.contains("demo-repo")
            || resource.contains("demo-repo")
            || (ntype.eq_ignore_ascii_case("Person")
                && (label == "alice" || label == "bob")
                && (id_l.contains("demo") || resource == "alice" || resource == "bob"));
        if is_demo {
            drop_ids.insert(id.to_string());
        }
    }
    if drop_ids.is_empty() {
        obj.insert("demo_hidden".into(), json!(0));
        return;
    }
    if let Some(nodes) = obj.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        nodes.retain(|n| {
            n.get("id")
                .and_then(|x| x.as_str())
                .map(|id| !drop_ids.contains(id))
                .unwrap_or(true)
        });
    }
    if let Some(edges) = obj.get_mut("edges").and_then(|e| e.as_array_mut()) {
        edges.retain(|e| {
            let from = e.get("from").and_then(|x| x.as_str()).unwrap_or("");
            let to = e.get("to").and_then(|x| x.as_str()).unwrap_or("");
            !drop_ids.contains(from) && !drop_ids.contains(to)
        });
    }
    // Refresh returned counts if present
    let n_count = obj
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let e_count = obj
        .get("edges")
        .and_then(|n| n.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    obj.insert("returned".into(), json!({ "nodes": n_count, "edges": e_count }));
    obj.insert("demo_hidden".into(), json!(drop_ids.len()));
}

async fn upsert_twin(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<UpsertTwinRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if tenant_id.is_empty() {
        return Err(ApiError::bad("tenant_id required"));
    }
    let now = Utc::now();
    let twin_id = body.twin_id.clone().unwrap_or_else(|| match body.twin_kind {
        TwinKind::Person => person_twin_id(&body.subject_id),
        TwinKind::Team => twin_core::ids::team_twin_id(&body.subject_id),
    });
    let existing = st.store.get_twin(&tenant_id, &twin_id).await.map_err(ApiError::from)?;
    let shadow_until = body.shadow_until.or_else(|| {
        existing
            .as_ref()
            .and_then(|t| t.shadow_until)
            .or_else(|| Some(now + Duration::days(st.cfg.shadow_mode_days)))
    });
    // Allow explicit null shadow via config flag shadow_mode_days=0 and high_auto...
    let shadow_until = if st.cfg.shadow_mode_days <= 0 && body.shadow_until.is_none() {
        None
    } else {
        shadow_until
    };

    let twin = Twin {
        tenant_id: tenant_id.clone(),
        twin_id: twin_id.clone(),
        twin_kind: body.twin_kind,
        subject_id: body.subject_id.clone(),
        display_name: body
            .display_name
            .or_else(|| existing.as_ref().map(|t| t.display_name.clone()))
            .unwrap_or_default(),
        timezone: body
            .timezone
            .or_else(|| existing.as_ref().map(|t| t.timezone.clone()))
            .unwrap_or_else(|| "UTC".into()),
        channel_id: body
            .channel_id
            .or_else(|| existing.as_ref().map(|t| t.channel_id.clone()))
            .unwrap_or_default(),
        shadow_until,
        high_auto_publish: body
            .high_auto_publish
            .unwrap_or(existing.as_ref().map(|t| t.high_auto_publish).unwrap_or(st.cfg.high_auto_publish_default)),
        enabled: body
            .enabled
            .unwrap_or(existing.as_ref().map(|t| t.enabled).unwrap_or(true)),
        config_json: body
            .config_json
            .unwrap_or_else(|| existing.as_ref().map(|t| t.config_json.clone()).unwrap_or(json!({}))),
        created_at: existing.as_ref().map(|t| t.created_at).unwrap_or(now),
        updated_at: now,
    };
    st.store.upsert_twin(twin.clone()).await.map_err(ApiError::from)?;

    if let Some(slack_uid) = body.slack_user_id {
        st.store
            .put_slack_map(SlackUserMap {
                tenant_id: tenant_id.clone(),
                global_user_id: body.subject_id,
                slack_user_id: slack_uid,
                slack_team_id: String::new(),
            })
            .await
            .map_err(ApiError::from)?;
    }

    Ok((StatusCode::CREATED, Json(twin)))
}

async fn get_twin(
    State(st): State<AppState>,
    Path((tenant_id, twin_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    match st.store.get_twin(&tenant_id, &twin_id).await.map_err(ApiError::from)? {
        Some(t) => Ok(Json(t)),
        None => Err(ApiError::not_found("twin")),
    }
}

#[derive(Deserialize)]
struct CompileBody {
    period_start: Option<chrono::DateTime<Utc>>,
    period_end: Option<chrono::DateTime<Utc>>,
    hops: Option<usize>,
    /// When true, set shadow_until to past for this compile path only if twin allows.
    skip_shadow: Option<bool>,
    /// Force Slack DM even if within notify_interval (demo / on-demand tool).
    force_notify: Option<bool>,
}

async fn compile_twin(
    State(st): State<AppState>,
    Path((tenant_id, twin_id)): Path<(String, String)>,
    body: Option<Json<CompileBody>>,
) -> Result<impl IntoResponse, ApiError> {
    let mut twin = st
        .store
        .get_twin(&tenant_id, &twin_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("twin"))?;

    let body = body.map(|j| j.0).unwrap_or(CompileBody {
        period_start: None,
        period_end: None,
        hops: None,
        skip_shadow: None,
        force_notify: None,
    });

    if body.skip_shadow == Some(true) {
        twin.shadow_until = None;
        st.store.upsert_twin(twin.clone()).await.map_err(ApiError::from)?;
    }

    let now = Utc::now();
    let (aligned_start, aligned_end) = st.cfg.aligned_period(now);
    let (lookback_start, lookback_end) = st.cfg.activity_lookback(now);
    let start = body.period_start.unwrap_or(aligned_start);
    let end = body.period_end.unwrap_or(aligned_end);
    // Custom body period also drives activity filter; otherwise rolling lookback.
    let (activity_start, activity_end) = if body.period_start.is_some() || body.period_end.is_some()
    {
        (start, end)
    } else {
        (lookback_start, lookback_end)
    };
    let opts = CompileOpts {
        period_start: start,
        period_end: end,
        activity_start,
        activity_end,
        hops: body.hops.unwrap_or(3),
    };

    let outcome = match st.compiler.compile_person(&twin, &opts).await {
        Ok(o) => {
            st.metrics.compile_ok.fetch_add(1, Ordering::Relaxed);
            if o.acl_empty {
                st.metrics.acl_empty.fetch_add(1, Ordering::Relaxed);
            }
            o
        }
        Err(e) => {
            st.metrics.compile_error.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError::from(e));
        }
    };

    let force = body.force_notify.unwrap_or(st.cfg.notify_on_compile_default);
    let key = (tenant_id.clone(), twin.twin_id.clone());
    let allow_notify = if force {
        true
    } else {
        let map = st.last_notify.lock();
        match map.get(&key) {
            None => st.cfg.notify_on_compile_default,
            Some(last) => {
                (now - *last).num_seconds() >= st.cfg.notify_interval_secs
                    && st.cfg.notify_on_compile_default
            }
        }
    };

    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    let del = service
        .start_after_compile_opts(
            &twin,
            &outcome.ledger,
            &outcome.draft_text,
            now,
            allow_notify,
            force, // force_notify from body or NOTIFY_ON_COMPILE
        )
        .await
        .map_err(ApiError::from)?;
    let draft = del.draft;

    if del.dm_sent {
        st.last_notify.lock().insert(key, now);
        st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
    } else if allow_notify {
        if del.suppressed.is_some() {
            st.metrics.dms_suppressed.fetch_add(1, Ordering::Relaxed);
        }
    }
    if draft.status == DraftStatus::Published {
        st.metrics.publish_ok.fetch_add(1, Ordering::Relaxed);
    }

    Ok(Json(json!({
        "run_id": outcome.run_id,
        "ledger_id": outcome.ledger.ledger_id,
        "confidence_rollup": outcome.ledger.confidence_rollup,
        "ledger": outcome.ledger.ledger,
        "draft": draft,
        "acl_empty": outcome.acl_empty,
        "notified": allow_notify && !draft.slack_dm_ts.is_empty(),
        "notify_interval_secs": st.cfg.notify_interval_secs,
        "status_window_secs": st.cfg.status_window_secs,
    })))
}

async fn get_ledger(
    State(st): State<AppState>,
    Path((tenant_id, ledger_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    match st
        .store
        .get_ledger(&tenant_id, &ledger_id)
        .await
        .map_err(ApiError::from)?
    {
        Some(l) => Ok(Json(l)),
        None => Err(ApiError::not_found("ledger")),
    }
}

async fn get_draft(
    State(st): State<AppState>,
    Path((tenant_id, draft_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    match st
        .store
        .get_draft(&tenant_id, &draft_id)
        .await
        .map_err(ApiError::from)?
    {
        Some(d) => Ok(Json(d)),
        None => Err(ApiError::not_found("draft")),
    }
}

async fn veto_draft(
    State(st): State<AppState>,
    Path((tenant_id, draft_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let draft = twin_delivery::veto_draft(st.store.clone(), &tenant_id, &draft_id)
        .await
        .map_err(ApiError::from)?;
    st.metrics.veto_total.fetch_add(1, Ordering::Relaxed);
    st.observer
        .log(
            &tenant_id,
            "dont_send",
            &draft.twin_id,
            json!({
                "draft_id": draft_id,
                "ledger_id": draft.ledger_id,
                "status": draft.status.as_str(),
                "source": "product_ui",
            }),
        )
        .await;
    persist_embedded(&st);
    Ok(Json(json!({
        "draft": draft,
        "outcome": "dont_send",
        "note": "Draft rejected — never posted to channel. Still stored as metadata; enable “Show unapproved digests” on Graph to see it.",
    })))
}

async fn publish_draft(
    State(st): State<AppState>,
    Path((tenant_id, draft_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let draft0 = st
        .store
        .get_draft(&tenant_id, &draft_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("draft"))?;
    let twin = st
        .store
        .get_twin(&tenant_id, &draft0.twin_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("twin"))?;
    // Already published → clear success (UI used to look like “nothing happened”)
    if draft0.status == DraftStatus::Published {
        let existing = st
            .store
            .get_publish_by_ledger(&tenant_id, &draft0.ledger_id)
            .await
            .map_err(ApiError::from)?;
        st.observer
            .log(
                &tenant_id,
                "approve_already",
                &twin.subject_id,
                json!({
                    "draft_id": draft_id,
                    "ledger_id": draft0.ledger_id,
                    "publish": existing,
                    "source": "product_ui",
                }),
            )
            .await;
        return Ok(Json(json!({
            "draft": draft0,
            "publish": existing,
            "outcome": "already_published",
            "note": "Already approved. Status was shared (channel or DM fallback). Compile digests for a new window to approve again.",
            "where_it_went": existing.as_ref().map(|p| p.channel_id.clone()).unwrap_or_default(),
        })));
    }

    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    match service.explicit_publish(&twin, &tenant_id, &draft_id).await {
        Ok((draft, pub_rec)) => {
            if pub_rec.is_some() {
                st.metrics.publish_ok.fetch_add(1, Ordering::Relaxed);
            }
            let where_to = pub_rec
                .as_ref()
                .map(|p| p.channel_id.clone())
                .unwrap_or_else(|| "(no channel record)".into());
            st.observer
                .log(
                    &tenant_id,
                    "approve",
                    &twin.subject_id,
                    json!({
                        "draft_id": draft_id,
                        "ledger_id": draft.ledger_id,
                        "status": draft.status.as_str(),
                        "where": where_to,
                        "publish": pub_rec,
                        "source": "product_ui",
                    }),
                )
                .await;
            persist_embedded(&st);
            Ok(Json(json!({
                "draft": draft,
                "publish": pub_rec,
                "outcome": "published",
                "note": "Approved. Shared to chat (team channel if bot is a member; else DM fallback). Work graph still comes from GitHub — digests show as Status overlay nodes.",
                "where_it_went": where_to,
            })))
        }
        Err(e) => {
            st.metrics.publish_fail.fetch_add(1, Ordering::Relaxed);
            if matches!(e, TwinError::Egress(_)) {
                st.metrics.egress_fail.fetch_add(1, Ordering::Relaxed);
                let detail = e.to_string();
                st.observer
                    .log(
                        &tenant_id,
                        "approve_failed",
                        &twin.subject_id,
                        json!({ "draft_id": draft_id, "error": detail }),
                    )
                    .await;
                return Err(ApiError {
                    status: StatusCode::BAD_GATEWAY,
                    message: format!(
                        "Approve needs Slack egress (channel share). Delivery proxy failed — check egress health + vault SLACK_BOT_TOKEN, then retry. ({detail})"
                    ),
                });
            }
            Err(ApiError::from(e))
        }
    }
}

#[derive(Deserialize)]
struct EditBody {
    text: String,
}

async fn edit_draft(
    State(st): State<AppState>,
    Path((tenant_id, draft_id)): Path<(String, String)>,
    Json(body): Json<EditBody>,
) -> Result<impl IntoResponse, ApiError> {
    let draft = twin_delivery::edit_draft(st.store.clone(), &tenant_id, &draft_id, &body.text)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(draft))
}

async fn silence_draft(
    State(st): State<AppState>,
    Path((tenant_id, draft_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let draft0 = st
        .store
        .get_draft(&tenant_id, &draft_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("draft"))?;
    let twin = st
        .store
        .get_twin(&tenant_id, &draft0.twin_id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("twin"))?;
    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    match service.silence_timeout(&twin, &tenant_id, &draft_id).await {
        Ok((draft, pub_rec)) => {
            if pub_rec.is_some() {
                st.metrics.publish_ok.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Json(json!({ "draft": draft, "publish": pub_rec })))
        }
        Err(e) => Err(ApiError::from(e)),
    }
}

/// Embedded-only: inject ACL-filtered graph fixture for compile (tests/smoke).
async fn set_fixture(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<FixtureBody>,
) -> Result<impl IntoResponse, ApiError> {
    let fixture = st
        .fixture
        .as_ref()
        .ok_or_else(|| ApiError::bad("fixtures only available in embedded mode"))?;
    fixture.set_view(&tenant_id, &body.global_user_id, body.view);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FixtureBody {
    global_user_id: String,
    view: GraphView,
}

async fn slack_interactions(
    State(st): State<AppState>,
    body: String,
) -> Result<impl IntoResponse, ApiError> {
    // Minimal interactivity: parse payload JSON for action_id veto/publish/edit
    // Slack sends application/x-www-form-urlencoded payload=...
    let payload = if let Some(rest) = body.strip_prefix("payload=") {
        urlencoding_decode(rest)
    } else {
        body
    };
    let v: serde_json::Value =
        serde_json::from_str(&payload).unwrap_or_else(|_| json!({}));
    let action = v
        .pointer("/actions/0/action_id")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let draft_id = v
        .pointer("/actions/0/value")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    // Never use Slack team id as product tenant — pilot tenant from env.
    let tenant_id = std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into());

    if draft_id.is_empty() {
        return Ok(Json(json!({ "ok": true, "note": "no draft" })));
    }

    match action {
        "veto" | "dont_send" | "don't_send" => {
            let _ = twin_delivery::veto_draft(st.store.clone(), &tenant_id, draft_id).await;
            st.metrics.veto_total.fetch_add(1, Ordering::Relaxed);
            st.observer
                .log(
                    &tenant_id,
                    "dont_send",
                    draft_id,
                    json!({ "source": "slack_interaction", "action": action }),
                )
                .await;
            persist_embedded(&st);
        }
        "publish" | "approve" => {
            if let Ok(Some(d)) = st.store.get_draft(&tenant_id, draft_id).await {
                if let Ok(Some(twin)) = st.store.get_twin(&tenant_id, &d.twin_id).await {
                    let service =
                        DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
                    let _ = service
                        .explicit_publish(&twin, &tenant_id, draft_id)
                        .await;
                    st.observer
                        .log(
                            &tenant_id,
                            "approve",
                            &twin.subject_id,
                            json!({ "source": "slack_interaction", "draft_id": draft_id }),
                        )
                        .await;
                    persist_embedded(&st);
                }
            }
        }
        _ => {}
    }
    Ok(Json(json!({ "ok": true })))
}

// ─── Person profile + follow-through + Slack channel intent claims ─────────

const SLACK_INTENT_CLAIMS_KV: &str = "slack_intent_claims";
const SLACK_INTENT_CLAIMS_MAX: usize = 100;
const TEXT_PREVIEW_MAX: usize = intent_engine::TEXT_PREVIEW_MAX;

/// Intent Engine classifier (in-house rules — see intent_engine module).
fn classify_slack_intent_text(text: &str) -> (String, f32) {
    let (t, c, _) = intent_engine::classify_text(text);
    (t, c)
}

fn truncate_preview(text: &str, max: usize) -> String {
    intent_engine::truncate_preview(text, max)
}

fn is_slack_channel_message(ev: &serde_json::Value) -> bool {
    let channel_type = ev
        .get("channel_type")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if channel_type == "channel" || channel_type == "group" {
        return true;
    }
    let channel = ev.get("channel").and_then(|c| c.as_str()).unwrap_or("");
    channel.starts_with('C') || channel.starts_with('G')
}

fn is_slack_dm_message(ev: &serde_json::Value) -> bool {
    let channel_type = ev
        .get("channel_type")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if channel_type == "im" || channel_type == "mpim" {
        return true;
    }
    let channel = ev.get("channel").and_then(|c| c.as_str()).unwrap_or("");
    channel.starts_with('D')
}

/// Persist channel/DM intent claim to tenant_kv ring buffer (max 100).
fn push_slack_intent_claim(st: &AppState, tenant_id: &str, claim: serde_json::Value) {
    let Some(store) = &st.embedded_store else {
        return;
    };
    let mut arr = store
        .get_tenant_kv(tenant_id, SLACK_INTENT_CLAIMS_KV)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    arr.push(claim);
    if arr.len() > SLACK_INTENT_CLAIMS_MAX {
        let drop_n = arr.len() - SLACK_INTENT_CLAIMS_MAX;
        arr.drain(0..drop_n);
    }
    store.put_tenant_kv(tenant_id, SLACK_INTENT_CLAIMS_KV, serde_json::Value::Array(arr));
    persist_embedded(st);
}

fn list_slack_intent_claims(st: &AppState, tenant_id: &str) -> Vec<serde_json::Value> {
    st.embedded_store
        .as_ref()
        .and_then(|s| s.get_tenant_kv(tenant_id, SLACK_INTENT_CLAIMS_KV))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

#[derive(Clone, Debug)]
struct ResolvedPerson {
    subject_id: String,
    twin_id: String,
    display_name: String,
    aliases: Vec<String>,
    match_keys: Vec<String>,
}

/// Resolve github login, gu_*, or twin id → person twin flexibly.
/// Prefers **enabled** twins with **exact** display_name / alias / subject match (avoids
/// identity fragmentation where substring hits ghost twins first).
async fn resolve_person(
    st: &AppState,
    tenant_id: &str,
    subject_raw: &str,
) -> Result<ResolvedPerson, ApiError> {
    let raw = urlencoding_decode(subject_raw).trim().to_string();
    if raw.is_empty() {
        return Err(ApiError::bad("subject_id required"));
    }
    let twins = st
        .store
        .list_twins(tenant_id)
        .await
        .map_err(ApiError::from)?;
    let maps = st
        .store
        .list_slack_maps(tenant_id)
        .await
        .map_err(ApiError::from)?;
    let raw_l = raw.to_ascii_lowercase();
    let rest = raw
        .strip_prefix("twin:person:")
        .unwrap_or(raw.as_str())
        .to_string();
    let rest_l = rest.to_ascii_lowercase();

    struct Cand {
        person: ResolvedPerson,
        enabled: bool,
        has_slack: bool,
        score: i32,
    }
    let mut ranked: Vec<Cand> = Vec::new();
    for t in twins.iter().filter(|t| t.twin_kind == TwinKind::Person) {
        let mut aliases: Vec<String> = t
            .config_json
            .get("provider_aliases")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        for m in maps.iter().filter(|m| m.global_user_id == t.subject_id) {
            if !aliases.iter().any(|a| a == &m.global_user_id) {
                aliases.push(m.global_user_id.clone());
            }
        }
        let mut keys = vec![
            t.subject_id.clone(),
            t.twin_id.clone(),
            t.display_name.clone(),
        ];
        keys.extend(aliases.iter().cloned());
        let person = ResolvedPerson {
            subject_id: t.subject_id.clone(),
            twin_id: t.twin_id.clone(),
            display_name: t.display_name.clone(),
            aliases: aliases.clone(),
            match_keys: keys.clone(),
        };
        let has_slack = maps.iter().any(|m| m.global_user_id == t.subject_id);
        let exact = keys
            .iter()
            .any(|k| k.eq_ignore_ascii_case(&raw) || k.eq_ignore_ascii_case(&rest));
        let exact_display = t.display_name.eq_ignore_ascii_case(&raw)
            || t.display_name.eq_ignore_ascii_case(&rest);
        let exact_subject = t.subject_id.eq_ignore_ascii_case(&raw)
            || t.subject_id.eq_ignore_ascii_case(&rest)
            || t.twin_id.eq_ignore_ascii_case(&raw);
        let substr = !exact
            && keys.iter().any(|k| {
                let kl = k.to_ascii_lowercase();
                // Only allow substring on keys longer than 4 to avoid gu_ noise
                (kl.len() >= 4 && (kl.contains(&raw_l) || raw_l.contains(&kl)))
                    || (rest_l.len() >= 4 && (kl.contains(&rest_l) || rest_l.contains(&kl)))
            });
        if !exact && !substr {
            continue;
        }
        let mut score = 0i32;
        if exact_display {
            score += 100;
        }
        if exact_subject {
            score += 90;
        }
        if exact {
            score += 50;
        }
        if substr {
            score += 5;
        }
        if t.enabled {
            score += 40;
        }
        if has_slack {
            score += 25;
        }
        ranked.push(Cand {
            person,
            enabled: t.enabled,
            has_slack,
            score,
        });
    }
    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.enabled.cmp(&a.enabled))
            .then_with(|| b.has_slack.cmp(&a.has_slack))
    });
    if let Some(best) = ranked.into_iter().next() {
        return Ok(best.person);
    }
    // Fabricate a synthetic person from the raw id (profile still useful for graph-only people)
    let twin_id = if raw.starts_with("twin:") {
        raw.clone()
    } else {
        person_twin_id(&raw)
    };
    Ok(ResolvedPerson {
        subject_id: raw.clone(),
        twin_id,
        display_name: raw.clone(),
        aliases: vec![raw.clone()],
        match_keys: vec![raw],
    })
}

fn person_matches_keys(person: &ResolvedPerson, hay: &str) -> bool {
    if hay.is_empty() {
        return false;
    }
    let h = hay.to_ascii_lowercase();
    person.match_keys.iter().any(|k| {
        let k = k.to_ascii_lowercase();
        !k.is_empty() && (h == k || h.contains(&k) || k.contains(&h))
    })
}

async fn fetch_v2_snapshot(
    st: &AppState,
    tenant_id: &str,
    node_limit: usize,
    edge_limit: usize,
) -> Option<serde_json::Value> {
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    if v2.is_empty() {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok()?;
    let _ = client
        .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
        .json(&json!({
            "global_user_id": "bridge_reader",
            "groups": ["grp_eng", "grp_default"],
        }))
        .send()
        .await;
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/snapshot?user_id=bridge_reader&node_limit={node_limit}&edge_limit={edge_limit}&include_demo=false"
    );
    let res = client.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    res.json().await.ok()
}

async fn fetch_v2_intents(st: &AppState, tenant_id: &str, reader: &str) -> Vec<serde_json::Value> {
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    if v2.is_empty() {
        return Vec::new();
    }
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/intents?user_id={}&limit=80",
        urlencoding_simple(reader)
    );
    probe_json(&url)
        .await
        .and_then(|v| v.get("intents").and_then(|i| i.as_array()).cloned())
        .unwrap_or_default()
}

fn intent_owner_matches(intent: &serde_json::Value, person: &ResolvedPerson) -> bool {
    let props = intent.get("properties").cloned().unwrap_or(json!({}));
    let owners = [
        props
            .get("owner_node_id")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
        intent
            .get("owner_node_id")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
        intent
            .get("owner")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
        intent
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
        intent
            .get("display_name")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
        intent.get("id").and_then(|x| x.as_str()).unwrap_or(""),
        // graph node resource_id sometimes embeds login
        intent
            .get("resource_id")
            .and_then(|x| x.as_str())
            .unwrap_or(""),
    ];
    owners.iter().any(|o| person_matches_keys(person, o))
        || json_looks_like_person_blob(intent, person)
}

fn json_looks_like_person_blob(v: &serde_json::Value, person: &ResolvedPerson) -> bool {
    let blob = v.to_string().to_ascii_lowercase();
    person
        .match_keys
        .iter()
        .filter(|k| k.len() >= 3)
        .any(|k| blob.contains(&k.to_ascii_lowercase()))
}

fn conflict_touches_person(c: &serde_json::Value, person: &ResolvedPerson) -> bool {
    json_looks_like_person_blob(c, person)
}

fn parse_time_flex(s: &str) -> Option<chrono::DateTime<Utc>> {
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    // unix seconds
    if let Ok(secs) = s.parse::<i64>() {
        return chrono::DateTime::from_timestamp(secs, 0);
    }
    None
}

fn compute_follow_through_items(
    person: &ResolvedPerson,
    intents: &[serde_json::Value],
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> (Vec<serde_json::Value>, String) {
    let now = Utc::now();
    let min_age = Duration::hours(24);
    let abandon_age = Duration::hours(72);
    let note = "Best-effort: non-demo intents older than ~24h; later AUTHORED/commit activity on about_node or matching resource → supported; FREEZE+later commits → contradicted; no signal after ~72h → abandoned; else unknown.";

    // Person node ids from graph
    let mut person_node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in nodes {
        if n.get("type").and_then(|x| x.as_str()) != Some("Person")
            && n.get("node_type").and_then(|x| x.as_str()) != Some("Person")
        {
            continue;
        }
        let id = n
            .get("id")
            .or_else(|| n.get("node_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let lab = n
            .get("label")
            .or_else(|| n.get("display_name"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if person_matches_keys(person, id) || person_matches_keys(person, lab) {
            if !id.is_empty() {
                person_node_ids.insert(id.to_string());
            }
        }
    }

    // Authored commit targets + times for this person
    let mut authored_targets: Vec<(String, Option<chrono::DateTime<Utc>>)> = Vec::new();
    for e in edges {
        let et = e
            .get("type")
            .or_else(|| e.get("edge_type"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if et != "AUTHORED" && et != "PUSHED_TO" && et != "OPENED" && et != "MERGED" {
            continue;
        }
        let from = e.get("from").or_else(|| e.get("from_node_id")).and_then(|x| x.as_str()).unwrap_or("");
        let to = e.get("to").or_else(|| e.get("to_node_id")).and_then(|x| x.as_str()).unwrap_or("");
        if !person_node_ids.contains(from) && !person_matches_keys(person, from) {
            continue;
        }
        let t = e
            .get("valid_from")
            .and_then(|x| x.as_str())
            .and_then(parse_time_flex);
        authored_targets.push((to.to_string(), t));
    }

    // Commit messages for keyword reinforcement
    let commit_msgs: Vec<String> = nodes
        .iter()
        .filter(|n| {
            n.get("type").and_then(|x| x.as_str()) == Some("Commit")
                || n.get("node_type").and_then(|x| x.as_str()) == Some("Commit")
        })
        .filter_map(|n| {
            n.get("message")
                .or_else(|| n.get("title"))
                .or_else(|| n.pointer("/properties/message"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_ascii_lowercase())
        })
        .collect();

    let mut items = Vec::new();
    for intent in intents {
        let is_demo = json_looks_like_demo_seed(intent)
            || intent.get("is_demo").and_then(|x| x.as_bool()).unwrap_or(false);
        if is_demo {
            continue;
        }
        if !intent_owner_matches(intent, person) {
            continue;
        }
        let props = intent.get("properties").cloned().unwrap_or(json!({}));
        let itype = props
            .get("intent_type")
            .or_else(|| intent.get("intent_type"))
            .and_then(|x| x.as_str())
            .unwrap_or("OTHER");
        let summary = intent
            .get("display_name")
            .or_else(|| intent.get("label"))
            .or_else(|| intent.get("title"))
            .or_else(|| props.get("summary"))
            .and_then(|x| x.as_str())
            .unwrap_or(itype)
            .to_string();
        let about = props
            .get("about_node_id")
            .or_else(|| intent.get("about_node_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let intent_at = props
            .get("stated_at")
            .or_else(|| props.get("created_at"))
            .or_else(|| intent.get("created_at"))
            .or_else(|| intent.get("valid_from"))
            .and_then(|x| x.as_str())
            .and_then(parse_time_flex);
        // Require ~24h age when we have a timestamp; if unknown, still evaluate as unknown-capable
        if let Some(at) = intent_at {
            if now.signed_duration_since(at) < min_age {
                continue; // too fresh
            }
        }
        // Later activity signals
        let later: Vec<&(String, Option<chrono::DateTime<Utc>>)> = authored_targets
            .iter()
            .filter(|(tgt, t)| {
                let about_hit = !about.is_empty()
                    && (tgt == &about || tgt.contains(&about) || about.contains(tgt.as_str()));
                let time_ok = match (intent_at, t) {
                    (Some(ia), Some(tt)) => *tt > ia,
                    (Some(_), None) => true,
                    (None, _) => true,
                };
                time_ok && (about_hit || !about.is_empty() && about_hit)
            })
            .collect();
        // Broader: any later authored by person after intent
        let any_later = authored_targets.iter().any(|(_, t)| match (intent_at, t) {
            (Some(ia), Some(tt)) => *tt > ia,
            _ => false,
        });
        let summary_l = summary.to_ascii_lowercase();
        let msg_hit = commit_msgs.iter().any(|m| {
            summary_l
                .split_whitespace()
                .filter(|w| w.len() > 4)
                .take(4)
                .any(|w| m.contains(w))
        });

        let status = match itype {
            "SHIP" | "FIX" | "EXPLORE" => {
                if !later.is_empty() || msg_hit {
                    "supported"
                } else if intent_at
                    .map(|at| now.signed_duration_since(at) >= abandon_age)
                    .unwrap_or(false)
                    && !any_later
                {
                    "abandoned"
                } else if any_later {
                    "supported"
                } else {
                    "unknown"
                }
            }
            "FREEZE" => {
                if !later.is_empty() || (any_later && msg_hit) {
                    "contradicted"
                } else if intent_at
                    .map(|at| now.signed_duration_since(at) >= abandon_age)
                    .unwrap_or(false)
                {
                    "supported" // freeze held — no contradictory commits found
                } else {
                    "unknown"
                }
            }
            "BLOCKED" => {
                if any_later || msg_hit {
                    "supported" // activity after blocked claim (unblocking work)
                } else if intent_at
                    .map(|at| now.signed_duration_since(at) >= abandon_age)
                    .unwrap_or(false)
                {
                    "abandoned"
                } else {
                    "unknown"
                }
            }
            _ => "unknown",
        };

        items.push(json!({
            "intent_id": intent.get("id").or_else(|| intent.get("node_id")),
            "intent_type": itype,
            "said_or_implied": summary,
            "about_node_id": if about.is_empty() { serde_json::Value::Null } else { json!(about) },
            "intent_at": intent_at.map(|t| t.to_rfc3339()),
            "later_signal": {
                "authored_hits": later.len(),
                "any_later_activity": any_later,
                "commit_message_overlap": msg_hit,
            },
            "status": status,
            "is_demo": false,
            "gap": match status {
                "unknown" => "Insufficient graph linkage between claim and later work",
                "abandoned" => "No supporting activity found after claim aged out",
                "contradicted" => "Later activity conflicts with FREEZE/hold claim",
                "supported" => "Later commits/PRs align with claim",
                _ => "",
            },
        }));
    }
    items.truncate(40);
    (items, note.to_string())
}

async fn build_follow_through(
    st: &AppState,
    tenant_id: &str,
    person: &ResolvedPerson,
) -> serde_json::Value {
    let reader = person.subject_id.clone();
    let mut intents = fetch_v2_intents(st, tenant_id, &reader).await;
    let snap = fetch_v2_snapshot(st, tenant_id, 800, 2000).await;
    let nodes = snap
        .as_ref()
        .and_then(|s| s.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = snap
        .as_ref()
        .and_then(|s| s.get("edges"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    if intents.is_empty() {
        // Fallback: Intent nodes from snapshot owned by person
        intents = nodes
            .iter()
            .filter(|n| {
                n.get("type").and_then(|x| x.as_str()) == Some("Intent")
                    || n.get("node_type").and_then(|x| x.as_str()) == Some("Intent")
            })
            .filter(|n| intent_owner_matches(n, person))
            .cloned()
            .collect();
    }
    // Also fold in slack_intent_claims older than 24h as soft claims
    let claims = list_slack_intent_claims(st, tenant_id);
    for c in claims {
        let sub = c.get("subject").and_then(|x| x.as_str()).unwrap_or("");
        if !person_matches_keys(person, sub) {
            continue;
        }
        let at = c.get("at").and_then(|x| x.as_str()).unwrap_or("");
        if let Some(dt) = parse_time_flex(at) {
            if Utc::now().signed_duration_since(dt) < Duration::hours(24) {
                continue;
            }
        }
        intents.push(json!({
            "id": format!("slack_claim:{}", c.get("ts").and_then(|x| x.as_str()).unwrap_or("?")),
            "display_name": c.get("text_preview").cloned().unwrap_or(json!("")),
            "intent_type": c.get("intent_type").cloned().unwrap_or(json!("OTHER")),
            "properties": {
                "intent_type": c.get("intent_type"),
                "stated_at": c.get("at"),
                "source": "slack_intent_claim",
                "confidence": c.get("confidence"),
            },
            "is_demo": false,
        }));
    }
    let (items, note) = compute_follow_through_items(person, &intents, &nodes, &edges);
    json!({
        "tenant_id": tenant_id,
        "subject_id": person.subject_id,
        "twin_id": person.twin_id,
        "count": items.len(),
        "items": items,
        "note": note,
    })
}

async fn get_follow_through(
    State(st): State<AppState>,
    Path((tenant_id, subject_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let person = resolve_person(&st, &tenant_id, &subject_id).await?;
    Ok(Json(build_follow_through(&st, &tenant_id, &person).await))
}

// ─── Intent Engine HTTP (in-house claim ledger) ─────────────────────────────

#[derive(Deserialize)]
struct IntentLedgerQ {
    include_demo: Option<bool>,
    open_only: Option<bool>,
    limit: Option<usize>,
}

fn list_explicit_claims(st: &AppState, tenant_id: &str) -> Vec<serde_json::Value> {
    st.embedded_store
        .as_ref()
        .and_then(|s| s.get_tenant_kv(tenant_id, intent_engine::EXPLICIT_CLAIMS_KV))
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
}

fn push_explicit_claim(st: &AppState, tenant_id: &str, claim: serde_json::Value) {
    let Some(store) = &st.embedded_store else {
        return;
    };
    let mut arr = store
        .get_tenant_kv(tenant_id, intent_engine::EXPLICIT_CLAIMS_KV)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    arr.push(claim);
    if arr.len() > intent_engine::EXPLICIT_CLAIMS_MAX {
        let drop_n = arr.len() - intent_engine::EXPLICIT_CLAIMS_MAX;
        arr.drain(0..drop_n);
    }
    store.put_tenant_kv(
        tenant_id,
        intent_engine::EXPLICIT_CLAIMS_KV,
        serde_json::Value::Array(arr),
    );
    persist_embedded(st);
}

async fn collect_intent_ledger(
    st: &AppState,
    tenant_id: &str,
    include_demo: bool,
    open_only: bool,
) -> Vec<intent_engine::IntentClaimRecord> {
    let mut graph_raw = fetch_v2_intents(st, tenant_id, "bridge_reader").await;
    if graph_raw.is_empty() {
        if let Some(snap) = fetch_v2_snapshot(st, tenant_id, 800, 2000).await {
            graph_raw = snap
                .get("nodes")
                .and_then(|n| n.as_array())
                .map(|a| {
                    a.iter()
                        .filter(|n| {
                            n.get("type").and_then(|x| x.as_str()) == Some("Intent")
                                || n.get("node_type").and_then(|x| x.as_str()) == Some("Intent")
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
        }
    }
    let graph: Vec<_> = graph_raw
        .iter()
        .map(|i| {
            let tagged = with_is_demo_tag(i.clone());
            intent_engine::claim_from_graph_intent(tenant_id, &tagged)
        })
        .collect();
    let slack: Vec<_> = list_slack_intent_claims(st, tenant_id)
        .iter()
        .map(|c| intent_engine::claim_from_slack_kv(tenant_id, c))
        .collect();
    let explicit: Vec<_> = list_explicit_claims(st, tenant_id)
        .iter()
        .filter_map(|c| serde_json::from_value::<intent_engine::IntentClaimRecord>(c.clone()).ok())
        .collect();
    intent_engine::merge_ledger(graph, slack, explicit, include_demo, open_only)
}

async fn intent_engine_status(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    let mut manifest = intent_engine::engine_manifest();
    // Live counts (include demo for honesty stats; live_claims separate)
    let all = collect_intent_ledger(&st, &tenant_id, true, false).await;
    let live_only: Vec<_> = all.iter().filter(|c| !c.is_demo).cloned().collect();
    let stats_all = intent_engine::ledger_stats(&all);
    let stats_live = intent_engine::ledger_stats(&live_only);
    // Conflicts from pulse (primary exec surface)
    let pulse = st.last_pulse.lock().get(&tenant_id).cloned();
    let conflict_n = pulse
        .as_ref()
        .and_then(|p| p.pointer("/conflicts/cards"))
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if let Some(obj) = manifest.as_object_mut() {
        obj.insert("tenant_id".into(), json!(tenant_id));
        obj.insert("ledger_all".into(), stats_all);
        obj.insert("ledger_live".into(), stats_live);
        obj.insert(
            "conflicts_cached".into(),
            json!({
                "count": conflict_n,
                "note": "Conflicts-first surface via /pulse; demo cards tagged is_demo",
            }),
        );
        obj.insert(
            "endpoints".into(),
            json!({
                "ledger": "GET /v3/tenants/{tenant}/intent/ledger?include_demo=false&open_only=true",
                "capture": "POST /v3/tenants/{tenant}/intent/claims",
                "supersede": "POST /v3/tenants/{tenant}/intent/claims/{id}/supersede",
                "profile": "GET /v3/tenants/{tenant}/people/{subject}/profile",
                "follow_through": "GET /v3/tenants/{tenant}/people/{subject}/follow_through",
                "pulse": "GET /v3/tenants/{tenant}/pulse",
            }),
        );
        obj.insert(
            "adequacy_note".into(),
            json!(
                "Trajectory (commits/graph) is strong; live purpose claims are still sparse until organic PRs + channel/bot capture fill L1. Demo seeds are tagged and excluded from live ledger by default."
            ),
        );
    }
    Json(manifest)
}

async fn intent_ledger(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<IntentLedgerQ>,
) -> impl IntoResponse {
    let include_demo = q.include_demo.unwrap_or(false);
    let open_only = q.open_only.unwrap_or(true);
    let limit = q.limit.unwrap_or(100).min(500);
    let mut claims = collect_intent_ledger(&st, &tenant_id, include_demo, open_only).await;
    let stats = intent_engine::ledger_stats(&claims);
    claims.truncate(limit);
    let rows: Vec<serde_json::Value> = claims.iter().map(|c| c.to_json()).collect();
    Json(json!({
        "tenant_id": tenant_id,
        "include_demo": include_demo,
        "open_only": open_only,
        "stats": stats,
        "count": rows.len(),
        "claims": rows,
        "principles": intent_engine::PRINCIPLES.iter().map(|(id, t)| json!({"id": id, "text": t})).collect::<Vec<_>>(),
        "note": "Unified claim ledger: V2 graph intents + slack channel/DM extracts + explicit captures. Default excludes demo seeds. Conflicts via /pulse.",
    }))
}

#[derive(Deserialize)]
struct IntentClaimBody {
    intent_type: Option<String>,
    summary: Option<String>,
    /// Free text — classified if intent_type omitted
    text: Option<String>,
    owner_subject: Option<String>,
    about_node_id: Option<String>,
    evidence: Option<Vec<String>>,
    channel: Option<String>,
}

async fn intent_claim_create(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<IntentClaimBody>,
) -> Result<impl IntoResponse, ApiError> {
    let text = body.text.clone().unwrap_or_default();
    let (itype, _conf, ev_tag) = if let Some(ref t) = body.intent_type {
        if intent_engine::IntentType::parse(t).is_some() {
            (
                t.to_ascii_uppercase(),
                0.95_f32,
                "source:explicit".to_string(),
            )
        } else {
            intent_engine::classify_text(&text)
        }
    } else if !text.trim().is_empty() {
        intent_engine::classify_text(&text)
    } else {
        return Err(ApiError::bad(
            "Provide intent_type and/or text (e.g. \"blocked on security\")",
        ));
    };
    let summary = body
        .summary
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.clone())
            }
        })
        .unwrap_or_else(|| format!("{itype} (explicit)"));
    let mut evidence = body.evidence.clone().unwrap_or_default();
    if !ev_tag.is_empty() && !evidence.iter().any(|e| e == &ev_tag) {
        evidence.push(ev_tag);
    }
    evidence.push("capture:explicit_api".into());
    let claim = intent_engine::build_explicit_claim(
        &tenant_id,
        &itype,
        &summary,
        body.owner_subject.as_deref(),
        body.about_node_id.as_deref(),
        evidence,
        body.channel.as_deref(),
    );
    let claim_json = claim.to_json();
    push_explicit_claim(&st, &tenant_id, claim_json.clone());
    st.observer
        .log(
            &tenant_id,
            "intent_claim_explicit",
            claim.intent_type.as_str(),
            json!({
                "claim_id": claim.claim_id,
                "owner": claim.owner_subject,
                "confidence": claim.confidence,
            }),
        )
        .await;
    Ok(Json(json!({
        "ok": true,
        "claim": claim_json,
        "note": "Explicit claim stored in tenant ledger (high trust). Conflicts still computed on graph-attached intents via V2; this claim appears in /intent/ledger and person profile slack/explicit fold.",
    })))
}

async fn intent_claim_supersede(
    State(st): State<AppState>,
    Path((tenant_id, claim_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let Some(store) = &st.embedded_store else {
        return Err(ApiError::bad("embedded store required for claim supersede"));
    };
    let mut arr = store
        .get_tenant_kv(&tenant_id, intent_engine::EXPLICIT_CLAIMS_KV)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let mut found = false;
    for c in &mut arr {
        if c.get("claim_id").and_then(|x| x.as_str()) == Some(claim_id.as_str()) {
            if let Some(obj) = c.as_object_mut() {
                obj.insert("lifecycle".into(), json!("superseded"));
                obj.insert("superseded_at".into(), json!(Utc::now().to_rfc3339()));
            }
            found = true;
        }
    }
    // Also mark slack claims if matching claim_id / slack:ts form
    let mut slack = store
        .get_tenant_kv(&tenant_id, SLACK_INTENT_CLAIMS_KV)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    for c in &mut slack {
        let id = c
            .get("claim_id")
            .or_else(|| c.get("ts"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if claim_id == id || claim_id == format!("slack:{id}") {
            if let Some(obj) = c.as_object_mut() {
                obj.insert("lifecycle".into(), json!("superseded"));
            }
            found = true;
        }
    }
    if !found {
        return Err(ApiError::bad(
            "claim_id not found in explicit or slack ledgers (graph intents supersede via V2 lifecycle later)",
        ));
    }
    store.put_tenant_kv(
        &tenant_id,
        intent_engine::EXPLICIT_CLAIMS_KV,
        serde_json::Value::Array(arr),
    );
    store.put_tenant_kv(
        &tenant_id,
        SLACK_INTENT_CLAIMS_KV,
        serde_json::Value::Array(slack),
    );
    persist_embedded(&st);
    st.observer
        .log(
            &tenant_id,
            "intent_claim_supersede",
            &claim_id,
            json!({ "ok": true }),
        )
        .await;
    Ok(Json(json!({
        "ok": true,
        "claim_id": claim_id,
        "lifecycle": "superseded",
        "note": "Human gate: claim no longer open in ledger (open_only=true).",
    })))
}

async fn get_person_profile(
    State(st): State<AppState>,
    Path((tenant_id, subject_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let person = resolve_person(&st, &tenant_id, &subject_id).await?;
    let as_of = Utc::now();

    // Digests for twin
    let drafts = st
        .store
        .list_drafts_for_twin(&tenant_id, &person.twin_id)
        .await
        .unwrap_or_default();
    let digests: Vec<serde_json::Value> = drafts
        .iter()
        .take(5)
        .map(|d| {
            json!({
                "draft_id": d.draft_id,
                "ledger_id": d.ledger_id,
                "status": d.status.as_str(),
                "updated_at": d.updated_at,
                "created_at": d.created_at,
                "preview": d.draft_text.chars().take(400).collect::<String>(),
                "draft_text": d.draft_text,
            })
        })
        .collect();

    // Insights (work surface + cadence) — filter to person
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    let snap = fetch_v2_snapshot(&st, &tenant_id, 800, 2000).await;
    let nodes = snap
        .as_ref()
        .and_then(|s| s.get("nodes"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = snap
        .as_ref()
        .and_then(|s| s.get("edges"))
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();

    let mut person_node_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in &nodes {
        let ty = n
            .get("type")
            .or_else(|| n.get("node_type"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if ty != "Person" {
            continue;
        }
        let id = n
            .get("id")
            .or_else(|| n.get("node_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let lab = n
            .get("label")
            .or_else(|| n.get("display_name"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if person_matches_keys(&person, id) || person_matches_keys(&person, lab) {
            if !id.is_empty() {
                person_node_ids.insert(id.to_string());
            }
        }
    }

    // Repos via resource_id / repo labels on commits authored by person
    let authored_commit_ids: std::collections::HashSet<String> = edges
        .iter()
        .filter(|e| {
            e.get("type").or_else(|| e.get("edge_type")).and_then(|x| x.as_str()) == Some("AUTHORED")
        })
        .filter(|e| {
            let from = e
                .get("from")
                .or_else(|| e.get("from_node_id"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            person_node_ids.contains(from) || person_matches_keys(&person, from)
        })
        .filter_map(|e| {
            e.get("to")
                .or_else(|| e.get("to_node_id"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
        .collect();

    let mut repos: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let mut commit_sample: Vec<serde_json::Value> = Vec::new();
    let mut hour_hist = vec![0u64; 24];
    for e in &edges {
        let et = e
            .get("type")
            .or_else(|| e.get("edge_type"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if et != "AUTHORED" && et != "PUSHED_TO" {
            continue;
        }
        let from = e
            .get("from")
            .or_else(|| e.get("from_node_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if !(person_node_ids.contains(from) || person_matches_keys(&person, from)) {
            continue;
        }
        if let Some(vf) = e.get("valid_from").and_then(|x| x.as_str()) {
            if let Some(dt) = parse_time_flex(vf) {
                hour_hist[dt.hour() as usize] += 1;
            }
        }
    }
    for n in &nodes {
        let ty = n
            .get("type")
            .or_else(|| n.get("node_type"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let id = n
            .get("id")
            .or_else(|| n.get("node_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if ty == "Commit" && (authored_commit_ids.contains(id) || authored_commit_ids.is_empty()) {
            // If we have authored set, require membership; if empty person nodes, fall back to label match in blob
            if !authored_commit_ids.is_empty() && !authored_commit_ids.contains(id) {
                continue;
            }
            if authored_commit_ids.is_empty() && !json_looks_like_person_blob(n, &person) {
                continue;
            }
            // Repo from id `commit:owner/repo:sha`, resource_id, or properties
            let mut repo_guess = String::new();
            if let Some(id_s) = n.get("id").or_else(|| n.get("node_id")).and_then(|x| x.as_str()) {
                // commit:org/repo:sha40
                if let Some(rest) = id_s.strip_prefix("commit:") {
                    if let Some((repo, _)) = rest.rsplit_once(':') {
                        if repo.contains('/') {
                            repo_guess = repo.to_string();
                        }
                    }
                }
            }
            if repo_guess.is_empty() {
                let rid = n
                    .get("resource_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if !rid.is_empty() {
                    let repo = rid.split('@').next().unwrap_or(rid);
                    if repo.contains('/') && !repo.chars().all(|c| c.is_ascii_hexdigit()) {
                        repo_guess = repo.to_string();
                    }
                }
            }
            if !repo_guess.is_empty() {
                *repos.entry(repo_guess).or_insert(0) += 1;
            }
            let msg = n
                .get("message")
                .or_else(|| n.get("title"))
                .or_else(|| n.pointer("/properties/message"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            commit_sample.push(json!({
                "id": id,
                "sha7": n.get("label").or_else(|| n.get("display_name")),
                "message": msg,
                "resource_id": n.get("resource_id"),
            }));
        }
        if ty == "Repository" || ty == "Repo" {
            let lab = n
                .get("label")
                .or_else(|| n.get("display_name"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if !lab.is_empty() && repos.contains_key(lab) {
                // already counted
            }
        }
    }
    // Prefer commits with messages
    commit_sample.sort_by(|a, b| {
        let am = a.get("message").and_then(|x| x.as_str()).unwrap_or("").len();
        let bm = b.get("message").and_then(|x| x.as_str()).unwrap_or("").len();
        bm.cmp(&am)
    });
    commit_sample.truncate(25);
    let repo_list: Vec<serde_json::Value> = repos
        .iter()
        .map(|(k, v)| json!({ "repo": k, "commit_touches": v }))
        .collect();
    let (peak_hour, peak_n) = hour_hist
        .iter()
        .enumerate()
        .max_by_key(|(_, n)| *n)
        .map(|(h, n)| (h, *n))
        .unwrap_or((0, 0));

    // Intents
    let reader = person.subject_id.as_str();
    let mut intents_raw = fetch_v2_intents(&st, &tenant_id, reader).await;
    if intents_raw.is_empty() {
        intents_raw = nodes
            .iter()
            .filter(|n| {
                n.get("type").and_then(|x| x.as_str()) == Some("Intent")
                    || n.get("node_type").and_then(|x| x.as_str()) == Some("Intent")
            })
            .cloned()
            .collect();
    }
    let intents: Vec<serde_json::Value> = intents_raw
        .into_iter()
        .filter(|i| intent_owner_matches(i, &person) || json_looks_like_person_blob(i, &person))
        .map(|i| {
            let mut tagged = with_is_demo_tag(i);
            if let Some(obj) = tagged.as_object_mut() {
                let itype = obj
                    .get("properties")
                    .and_then(|p| p.get("intent_type"))
                    .or_else(|| obj.get("intent_type"))
                    .cloned()
                    .unwrap_or(json!("OTHER"));
                obj.insert("intent_type".into(), itype);
            }
            tagged
        })
        .collect();

    // Conflicts from pulse cache
    let pulse = st.last_pulse.lock().get(&tenant_id).cloned();
    if pulse.is_none() {
        let _ = run_thin_monitors(&st).await;
    }
    let pulse = st
        .last_pulse
        .lock()
        .get(&tenant_id)
        .cloned()
        .unwrap_or(json!({}));
    let mut conflict_cards: Vec<serde_json::Value> = pulse
        .pointer("/conflicts/cards")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let demo_cards: Vec<serde_json::Value> = pulse
        .pointer("/conflicts/demo_cards")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    conflict_cards.extend(demo_cards);
    let conflicts_touching: Vec<serde_json::Value> = conflict_cards
        .into_iter()
        .filter(|c| conflict_touches_person(c, &person))
        .map(with_is_demo_tag)
        .collect();

    let follow = build_follow_through(&st, &tenant_id, &person).await;

    // Slack intent claims for this person
    let slack_claims: Vec<serde_json::Value> = list_slack_intent_claims(&st, &tenant_id)
        .into_iter()
        .filter(|c| {
            let sub = c.get("subject").and_then(|x| x.as_str()).unwrap_or("");
            person_matches_keys(&person, sub)
                || c.get("slack_user")
                    .and_then(|x| x.as_str())
                    .map(|s| !s.is_empty() && person_matches_keys(&person, s))
                    .unwrap_or(false)
        })
        .rev()
        .take(30)
        .collect();

    // confidence heuristic
    let mut conf = 0.15_f64;
    if !person_node_ids.is_empty() {
        conf += 0.15;
    }
    if !commit_sample.is_empty() {
        conf += 0.2;
    }
    if !digests.is_empty() {
        conf += 0.15;
    }
    if intents.iter().any(|i| i.get("is_demo") != Some(&json!(true))) {
        conf += 0.15;
    }
    if !slack_claims.is_empty() {
        conf += 0.15;
    }
    if peak_n > 0 {
        conf += 0.05;
    }
    conf = conf.min(0.95);

    let what_we_cannot_know = json!([
        "Private 1:1 DMs (no silent wiretap — only bot DMs the person initiates / digest replies)",
        "Full Slack channel history unless messages were ingested while the bot was a channel member",
        "Linear/Jira/ticket systems (not connected in this pilot)",
        "Code review private notes and offline conversations",
        "Calendar / meeting content",
        "True goals or preferences not stated as claims or visible as work exhaust",
        "Productivity rankings or LOC-based performance scores (doctrine: never)",
    ]);

    let _ = v2; // reserved for future V2 project post

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "subject": {
            "subject_id": person.subject_id,
            "twin_id": person.twin_id,
            "display_name": person.display_name,
            "aliases": person.aliases,
            "resolved_from": subject_id,
            "graph_person_node_ids": person_node_ids.iter().cloned().collect::<Vec<_>>(),
        },
        "as_of": as_of.to_rfc3339(),
        "work_surface": {
            "repos": repo_list,
            "commit_sample": commit_sample,
            "authored_commit_count": authored_commit_ids.len(),
        },
        "cadence": {
            "peak_hour_utc": peak_hour,
            "peak_count": peak_n,
            "hour_of_day_utc": hour_hist,
            "notes": if peak_n > 0 {
                format!("Most active hour (UTC) for this person: {peak_hour:02}:00 ({peak_n} edge events). Not a ranking.")
            } else {
                "Insufficient person-scoped activity edges for cadence.".into()
            },
        },
        "digests": {
            "count": digests.len(),
            "latest": digests,
        },
        "intents": intents,
        "conflicts_touching": conflicts_touching,
        "follow_through": follow,
        "slack_intent_claims": slack_claims,
        "confidence_overall": conf,
        "what_we_cannot_know": what_we_cannot_know,
        "doctrine": "Slack = delivery + opt-in team channel truth (bot must be invited). GitHub = work. Intent = typed claim with evidence, not chat archive. No LOC rankings. No silent 1:1 DM wiretap.",
        "note": "Developer-evaluator profile from live V3 digests + V2 graph/intents/pulse + slack_intent_claims. Demo intents/conflicts tagged is_demo.",
    })))
}

async fn slack_events(
    State(st): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // URL verification challenge
    if body.get("type").and_then(|t| t.as_str()) == Some("url_verification") {
        return Json(json!({ "challenge": body.get("challenge") }));
    }
    // Plain-text DM: approve / don't send; channel/DM free text → intent claims
    if body.get("type").and_then(|t| t.as_str()) == Some("event_callback") {
        let ev = body.get("event").cloned().unwrap_or(json!({}));
        let et = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let subtype = ev.get("subtype").and_then(|t| t.as_str()).unwrap_or("");
        // Ignore bot messages + message edits/joins (prefer plain messages)
        let ignore_sub = !subtype.is_empty()
            && subtype != "file_share"
            && subtype != "thread_broadcast";
        if et == "message" && !ignore_sub && ev.get("bot_id").is_none() {
            let text_raw = ev
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let text = text_raw.to_ascii_lowercase();
            let slack_uid = ev.get("user").and_then(|u| u.as_str()).unwrap_or("");
            let channel = ev.get("channel").and_then(|c| c.as_str()).unwrap_or("");
            let ts = ev.get("ts").and_then(|t| t.as_str()).unwrap_or("");
            let tenant_id =
                std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into());
            let is_dm = is_slack_dm_message(&ev);
            let is_channel = is_slack_channel_message(&ev);

            // ── Approve / don't send (DM path — keep working) ──
            let action = if text == "approve"
                || text == "a"
                || text.starts_with("approve ")
                || text == "✅"
            {
                Some("approve")
            } else if text == "don't send"
                || text == "dont send"
                || text == "veto"
                || text == "reject"
                || text == "no"
            {
                Some("dont_send")
            } else {
                None
            };
            if let Some(act) = action {
                // Prefer DM; still honor if typed elsewhere for back-compat
                if let Ok(maps) = st.store.list_slack_maps(&tenant_id).await {
                    if let Some(m) = maps.iter().find(|m| m.slack_user_id == slack_uid) {
                        let twin_id = person_twin_id(&m.global_user_id);
                        if let Ok(Some(twin)) = st.store.get_twin(&tenant_id, &twin_id).await {
                            if let Ok(drafts) =
                                st.store.list_drafts_for_twin(&tenant_id, &twin.twin_id).await
                            {
                                if let Some(d) = drafts.into_iter().next() {
                                    match act {
                                        "dont_send" => {
                                            let _ = twin_delivery::veto_draft(
                                                st.store.clone(),
                                                &tenant_id,
                                                &d.draft_id,
                                            )
                                            .await;
                                            st.metrics.veto_total.fetch_add(1, Ordering::Relaxed);
                                        }
                                        "approve" => {
                                            let service = DeliveryService::new(
                                                st.store.clone(),
                                                st.slack.clone(),
                                                st.policy.clone(),
                                            );
                                            let _ = service
                                                .explicit_publish(
                                                    &twin,
                                                    &tenant_id,
                                                    &d.draft_id,
                                                )
                                                .await;
                                        }
                                        _ => {}
                                    }
                                    st.observer
                                        .log(
                                            &tenant_id,
                                            act,
                                            &twin.subject_id,
                                            json!({
                                                "source": "slack_text",
                                                "text": text,
                                                "draft_id": d.draft_id,
                                                "slack_user": slack_uid,
                                            }),
                                        )
                                        .await;
                                    persist_embedded(&st);
                                }
                            }
                        }
                    }
                }
            } else if is_channel || is_dm {
                // ── Intent claim capture (channels where bot is member; DM free-text) ──
                let (itype, conf) = classify_slack_intent_text(&text_raw);
                let keyword_hit = conf >= 0.7
                    || text.contains("blocked on")
                    || text.contains("working on")
                    || text.contains("freeze")
                    || text.contains("ready to ship")
                    || text.contains("do not merge")
                    || text.contains("don't merge");
                if keyword_hit {
                    let maps = st.store.list_slack_maps(&tenant_id).await.unwrap_or_default();
                    let subject = maps
                        .iter()
                        .find(|m| m.slack_user_id == slack_uid)
                        .map(|m| m.global_user_id.clone())
                        .unwrap_or_else(|| {
                            if slack_uid.is_empty() {
                                "unknown".into()
                            } else {
                                format!("slack:{slack_uid}")
                            }
                        });
                    let preview = truncate_preview(&text_raw, TEXT_PREVIEW_MAX);
                    let channel_label = if is_dm {
                        "dm".to_string()
                    } else {
                        channel.to_string()
                    };
                    let claim = json!({
                        "at": Utc::now().to_rfc3339(),
                        "slack_user": slack_uid,
                        "subject": subject,
                        "text_preview": preview,
                        "intent_type": itype.clone(),
                        "channel": channel_label,
                        "ts": ts,
                        "confidence": conf,
                        "source": if is_dm { "slack_dm" } else { "slack_channel" },
                    });
                    st.observer
                        .log(
                            &tenant_id,
                            "slack_intent",
                            &subject,
                            claim.clone(),
                        )
                        .await;
                    push_slack_intent_claim(&st, &tenant_id, claim);
                    // Optional lightweight V2 project (best-effort; KV is source of truth for profile)
                    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
                    if !v2.is_empty() && conf >= 0.75 {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(5))
                            .build();
                        if let Ok(client) = client {
                            let event_id = format!(
                                "slack-intent-{}-{}",
                                channel_label,
                                ts.replace('.', "_")
                            );
                            let intent_node_id = format!("intent:slack:{event_id}");
                            let _ = client
                                .post(format!("{v2}/v2/project"))
                                .json(&json!({
                                    "tenant_id": tenant_id,
                                    "event_id": event_id,
                                    "event_type": "intent.stated",
                                    "provider": "slack",
                                    "occurred_at": Utc::now().to_rfc3339(),
                                    "actor": { "provider_user_id": subject },
                                    "resource": {
                                        "kind": "intent",
                                        "provider_id": intent_node_id,
                                        "title": format!("{itype}: {preview}"),
                                    },
                                    "payload": {
                                        "intent_type": itype,
                                        "confidence": conf,
                                        "source": "slack_channel_claim",
                                        "text_preview": preview,
                                        "channel": channel_label,
                                    },
                                }))
                                .send()
                                .await;
                        }
                    }
                }
            }
        }
    }
    Json(json!({ "ok": true }))
}

#[derive(Deserialize)]
struct EventsQ {
    limit: Option<usize>,
}

async fn list_events(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<EventsQ>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).min(500);
    let mut events = st.observer.list_embedded(&tenant_id, limit);
    if let Some(pg) = st.observer.list_pg(&tenant_id, limit as i64).await {
        // Prefer Postgres when connected (authoritative external view)
        events = pg;
    }
    Json(json!({
        "tenant_id": tenant_id,
        "external_db": st.observer.external_connected(),
        "count": events.len(),
        "events": events,
        "note": if st.observer.external_connected() {
            "Events from OBSERVE_DATABASE_URL (Neon). SELECT * FROM twin_events ORDER BY at DESC;"
        } else {
            "Embedded event log only. Set OBSERVE_DATABASE_URL to a Neon Postgres URL for external live SQL."
        },
    }))
}

/// Mirror embedded twin state (Docker volume JSON) → Neon tables.
async fn sync_to_db(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !st.observer.external_connected() {
        return Err(ApiError::bad(
            "OBSERVE_DATABASE_URL not set or connect failed. Add Neon URL to deploy/.env.staging and restart twin-api.",
        ));
    }
    let Some(store) = st.embedded_store.as_ref() else {
        return Err(ApiError::bad(
            "sync_to_db requires embedded twin store (staging mode)",
        ));
    };
    // Disk first, then explicit full upsert (idempotent — no delete races)
    if let Some(path) = &st.twin_persist_path {
        let _ = store.save_to_path(path);
    }
    match st.observer.sync_store(&tenant_id, store.as_ref()).await {
        Ok(body) => Ok(Json(body)),
        Err(e) => Err(ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("sync_to_db failed: {e}"),
        }),
    }
}

/// Fetch V2 ACL snapshot and upsert nodes+edges into Neon graph_* tables.
async fn sync_graph_to_db(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !st.observer.external_connected() {
        return Err(ApiError::bad(
            "OBSERVE_DATABASE_URL not set or connect failed. Add Neon URL to deploy/.env.staging and restart twin-api.",
        ));
    }
    match export_graph_to_neon(&st, Some(tenant_id.as_str())).await {
        Ok(body) => Ok(Json(body)),
        Err(e) => Err(ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: format!("sync_graph_to_db failed: {e}"),
        }),
    }
}

/// Single-flight gate: bulk export must not stack (Neon pool + twin dual-write).
fn graph_export_gate() -> &'static tokio::sync::Mutex<()> {
    static GATE: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    GATE.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Shared path for on-demand + periodic V2 → Neon graph export.
/// `tenant_override` None → DEFAULT_TENANT_ID / SEED_TEAM_TENANT / ten_github.
/// `wait_for_lock`: true for HTTP (wait); false for background tick (skip if busy).
async fn export_graph_to_neon(
    st: &AppState,
    tenant_override: Option<&str>,
) -> Result<serde_json::Value, String> {
    export_graph_to_neon_inner(st, tenant_override, true).await
}

async fn export_graph_to_neon_inner(
    st: &AppState,
    tenant_override: Option<&str>,
    wait_for_lock: bool,
) -> Result<serde_json::Value, String> {
    if !st.observer.external_connected() {
        return Err("OBSERVE_DATABASE_URL not connected".into());
    }
    let _guard = if wait_for_lock {
        graph_export_gate().lock().await
    } else {
        match graph_export_gate().try_lock() {
            Ok(g) => g,
            Err(_) => {
                return Err("graph export already in progress (skipped tick)".into());
            }
        }
    };
    let tenant_id = tenant_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::env::var("DEFAULT_TENANT_ID")
                .or_else(|_| std::env::var("SEED_TEAM_TENANT"))
                .unwrap_or_else(|_| "ten_github".into())
        });
    let v2 = st.cfg.v2_base_url.trim_end_matches('/');
    if v2.is_empty() {
        return Err("V2 base URL not configured".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    // Ensure bridge_reader membership (same pattern as dev_insights / get_graph_snapshot)
    let _ = client
        .post(format!("{v2}/v2/tenants/{tenant_id}/users"))
        .json(&json!({
            "global_user_id": "bridge_reader",
            "groups": ["grp_eng", "grp_default"],
        }))
        .send()
        .await;
    let url = format!(
        "{v2}/v2/tenants/{tenant_id}/snapshot?user_id=bridge_reader&node_limit=2000&edge_limit=5000&include_demo=false"
    );
    let snap: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("v2 snapshot request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("v2 snapshot status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("v2 snapshot json: {e}"))?;
    let nodes = snap
        .get("nodes")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = snap
        .get("edges")
        .and_then(|n| n.as_array())
        .cloned()
        .unwrap_or_default();
    let body = st
        .observer
        .sync_graph_snapshot(&tenant_id, &nodes, &edges)
        .await?;
    let n = body.get("nodes").and_then(|v| v.as_i64()).unwrap_or(0);
    let e = body.get("edges").and_then(|v| v.as_i64()).unwrap_or(0);
    tracing::info!(%tenant_id, nodes = n, edges = e, "graph neon export ok");
    let _ = st
        .observer
        .log(
            &tenant_id,
            "sync_graph_to_db",
            "v2_snapshot",
            json!({ "nodes": n, "edges": e }),
        )
        .await;
    Ok(body)
}

async fn observe_status(State(st): State<AppState>) -> impl IntoResponse {
    let external = st.observer.external_connected();
    Json(json!({
        "external_db": external,
        "env_url_set": std::env::var("OBSERVE_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")).map(|s| !s.trim().is_empty()).unwrap_or(false),
        "continuous_mirror": external,
        "graph_mirror": external,
        "tables": [
            "twin_events",
            "twin_snapshot_json",
            "twin_twins",
            "twin_slack_maps",
            "twin_drafts",
            "twin_tenant_kv",
            "graph_nodes",
            "graph_edges",
            "graph_export_meta"
        ],
        "sync_endpoint": "POST /v3/tenants/{tenant_id}/sync_to_db",
        "graph_export_endpoint": "POST /v3/tenants/{tenant_id}/sync_graph_to_db",
        "events_endpoint": "GET /v3/tenants/{tenant_id}/events",
        "note": if external {
            "Neon connected. Twin dual-write continuous; V2 graph export is periodic (GRAPH_NEON_EXPORT_INTERVAL_SECS, default 900) + on-demand via sync_graph_to_db. Graph UI remains primary."
        } else {
            "Not connected. Set OBSERVE_DATABASE_URL on droplet deploy/.env.staging and restart twin-api."
        },
    }))
}

/// Bot Framework messaging endpoint — Adaptive Card Action.Submit → Approve / Edit / Don't send.
async fn teams_messages(
    State(st): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ApiError> {
    // Activity type invoke (Adaptive Card submit) or message
    let activity_type = body.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if activity_type == "invoke"
        || body
            .pointer("/value/action")
            .and_then(|v| v.as_str())
            .is_some()
    {
        let action = body
            .pointer("/value/action")
            .or_else(|| body.pointer("/value/data/action"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let draft_id = body
            .pointer("/value/draft_id")
            .or_else(|| body.pointer("/value/data/draft_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tenant_id = body
            .pointer("/channelData/tenant/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                std::env::var("DEFAULT_TENANT_ID").unwrap_or_else(|_| "ten_github".into())
            });
        if !draft_id.is_empty() {
            match action {
                "veto" => {
                    let _ = twin_delivery::veto_draft(st.store.clone(), &tenant_id, draft_id).await;
                    st.metrics.veto_total.fetch_add(1, Ordering::Relaxed);
                }
                "publish" => {
                    if let Ok(Some(d)) = st.store.get_draft(&tenant_id, draft_id).await {
                        if let Ok(Some(twin)) = st.store.get_twin(&tenant_id, &d.twin_id).await {
                            let service = DeliveryService::new(
                                st.store.clone(),
                                st.slack.clone(),
                                st.policy.clone(),
                            );
                            let _ = service.explicit_publish(&twin, &tenant_id, draft_id).await;
                        }
                    }
                }
                "edit" => {
                    // Edit requires new text from the card form; acknowledge for product UI path.
                    tracing::info!(%draft_id, "teams edit action — use product UI My status for freeform edit");
                }
                _ => {}
            }
        }
        // Bot Framework expects 200 + body for invoke
        return Ok(Json(json!({
            "statusCode": 200,
            "type": "application/vnd.microsoft.card.adaptive",
            "value": {
                "type": "AdaptiveCard",
                "version": "1.4",
                "body": [{ "type": "TextBlock", "text": format!("Recorded: {action}"), "wrap": true }]
            }
        })));
    }
    // ConversationUpdate / ping — ok
    Ok(Json(json!({ "ok": true })))
}

// ─── Roles (champion vs member) ─────────────────────────────────────────────

fn default_roles_json() -> serde_json::Value {
    json!({
        "champions": [],
        "default_role": "champion",
        "note": "Pilot: all seats act as champion until SSO. Set champions[] subject_ids for member-gated cockpit writes."
    })
}

async fn get_roles(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    let roles = st
        .embedded_store
        .as_ref()
        .and_then(|s| s.get_tenant_kv(&tenant_id, "roles"))
        .unwrap_or_else(default_roles_json);
    Json(json!({
        "tenant_id": tenant_id,
        "roles": roles,
        "delivery_adapter": st.delivery_adapter,
    }))
}

#[derive(Deserialize)]
struct RolesBody {
    champions: Option<Vec<String>>,
    default_role: Option<String>,
}

async fn put_roles(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<RolesBody>,
) -> Result<impl IntoResponse, ApiError> {
    let champions = body.champions.unwrap_or_default();
    let default_role = body
        .default_role
        .unwrap_or_else(|| "champion".into());
    let default_role = if default_role.eq_ignore_ascii_case("member") {
        "member"
    } else {
        "champion"
    };
    let value = json!({
        "champions": champions,
        "default_role": default_role,
        "updated_at": Utc::now().to_rfc3339(),
    });
    if let Some(store) = &st.embedded_store {
        store.put_tenant_kv(&tenant_id, "roles", value.clone());
        persist_embedded(&st);
    } else {
        return Err(ApiError::bad(
            "roles persist requires embedded twin store (staging)",
        ));
    }
    Ok(Json(json!({ "ok": true, "tenant_id": tenant_id, "roles": value })))
}

// ─── Tomorrow focus (persist assignments) ───────────────────────────────────

async fn get_tomorrow_focus(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> impl IntoResponse {
    let focus = st
        .embedded_store
        .as_ref()
        .and_then(|s| s.get_tenant_kv(&tenant_id, "tomorrow_focus"))
        .unwrap_or_else(|| {
            json!({
                "items": [],
                "note": "Empty — cockpit suggestions can be pinned here."
            })
        });
    Json(json!({ "tenant_id": tenant_id, "focus": focus }))
}

#[derive(Deserialize)]
struct TomorrowFocusBody {
    items: Vec<serde_json::Value>,
    note: Option<String>,
}

async fn put_tomorrow_focus(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<TomorrowFocusBody>,
) -> Result<impl IntoResponse, ApiError> {
    let value = json!({
        "items": body.items,
        "note": body.note.unwrap_or_else(|| "Pinned by champion".into()),
        "updated_at": Utc::now().to_rfc3339(),
    });
    if let Some(store) = &st.embedded_store {
        store.put_tenant_kv(&tenant_id, "tomorrow_focus", value.clone());
        persist_embedded(&st);
    } else {
        return Err(ApiError::bad(
            "tomorrow_focus persist requires embedded twin store (staging)",
        ));
    }
    Ok(Json(json!({ "ok": true, "tenant_id": tenant_id, "focus": value })))
}

fn urlencoding_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(m: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: m.into(),
        }
    }
    fn not_found(m: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: m.into(),
        }
    }
}

impl From<TwinError> for ApiError {
    fn from(e: TwinError) -> Self {
        let status = match &e {
            TwinError::NotFound(_) => StatusCode::NOT_FOUND,
            TwinError::Validation(_) => StatusCode::BAD_REQUEST,
            TwinError::Conflict(_) => StatusCode::CONFLICT,
            TwinError::AclDenied(_) => StatusCode::FORBIDDEN,
            TwinError::Egress(_) => StatusCode::BAD_GATEWAY,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
