//! Vertical 3 twin-api — status twins, ledgers, veto-first delivery (:18083).
//! Demo console at `/demo/` for founder/lead visibility (M4 Sew & Show).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
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
    DeliveryPolicy, DeliveryService, EgressSlackClient, MockSlackClient, SlackClient,
};

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
    slack: Arc<dyn SlackClient>,
    policy: DeliveryPolicy,
    mode: String,
    /// "mock" | "egress"
    slack_mode: String,
    metrics: Arc<Metrics>,
    cfg: TwinConfig,
    /// Last demo simulation snapshot per tenant (for console).
    last_demo: Arc<Mutex<std::collections::HashMap<String, serde_json::Value>>>,
    /// Last Slack notify time per (tenant, twin_id) for debounce.
    last_notify: Arc<Mutex<std::collections::HashMap<(String, String), chrono::DateTime<Utc>>>>,
    /// Cached team pulse (conflicts + intent counts) per tenant from thin monitor.
    last_pulse: Arc<Mutex<std::collections::HashMap<String, serde_json::Value>>>,
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
        .route("/v3/oauth/slack/start", get(oauth_slack_start))
        .route("/v3/oauth/github/start", get(oauth_github_start))
        .route(
            "/v3/tenants/{tenant_id}/twins",
            get(list_twins_route).post(upsert_twin),
        )
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
        // Mock Slack by default in embedded; USE_EGRESS_SLACK=true for real proxy DMs
        let force_egress = std::env::var("USE_EGRESS_SLACK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let (slack, slack_mode): (Arc<dyn SlackClient>, String) = if force_egress {
            let egress = twin_core::EgressClient::new(twin_core::EgressConfig {
                proxy_url: cfg.egress_proxy_url.clone(),
                enforce: cfg.egress_enforce,
            })?;
            info!("Slack delivery via egress proxy (USE_EGRESS_SLACK=true)");
            (Arc::new(EgressSlackClient::new(egress)), "egress".into())
        } else {
            info!("Slack delivery: mock (set USE_EGRESS_SLACK=true for real DMs)");
            (MockSlackClient::new(), "mock".into())
        };
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
            metrics,
            cfg,
            last_demo,
            last_notify: last_notify.clone(),
            last_pulse: last_pulse.clone(),
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
    let (slack, slack_mode): (Arc<dyn SlackClient>, String) =
        if std::env::var("FORCE_MOCK_SLACK")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            (MockSlackClient::new(), "mock".into())
        } else {
            let egress = twin_core::EgressClient::new(twin_core::EgressConfig {
                proxy_url: cfg.egress_proxy_url.clone(),
                enforce: cfg.egress_enforce,
            })?;
            (Arc::new(EgressSlackClient::new(egress)), "egress".into())
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
        metrics,
        cfg,
        last_demo,
        last_notify,
        last_pulse,
    })
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
            .collect();
        let demo_conflicts: Vec<serde_json::Value> = all_conflict_cards
            .iter()
            .filter(|c| json_looks_like_demo_seed(c))
            .cloned()
            .collect();
        let live_intents: Vec<serde_json::Value> = all_intent_sample
            .iter()
            .filter(|i| !json_looks_like_demo_seed(i))
            .cloned()
            .collect();
        let demo_intents: Vec<serde_json::Value> = all_intent_sample
            .iter()
            .filter(|i| json_looks_like_demo_seed(i))
            .cloned()
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
                "note": "Primary cards exclude intent_demo seed; demo_* fields keep Load intent demo visible",
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
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
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
        .timeout(std::time::Duration::from_secs(2))
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

async fn oauth_slack_start() -> impl IntoResponse {
    if !env_present("SLACK_CLIENT_ID") || !env_present("SLACK_CLIENT_SECRET") {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({
                "error": "slack_oauth_not_configured",
                "message": "Set SLACK_CLIENT_ID and SLACK_CLIENT_SECRET (human secrets). Manifest: deploy/oauth/slack-app-manifest.json. Until then use vault SLACK_BOT_TOKEN via egress.",
                "manual_path": "vertical-security/secrets/dev_secrets.json"
            })),
        );
    }
    // Full authorize redirect when credentials exist (next slice after human secrets).
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
            "note": "Open authorize_url in browser; callback store bot token in egress vault only."
        })),
    )
}

async fn oauth_github_start() -> impl IntoResponse {
    if !env_present("GITHUB_APP_ID") {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({
                "error": "github_app_not_configured",
                "message": "Set GITHUB_APP_ID (and related secrets). Manifest: deploy/oauth/github-app-manifest.yml. Manual webhooks to V1 still work.",
                "webhook_path": "/v1/tenants/{tenant_id}/webhooks/github"
            })),
        );
    }
    let app_slug = std::env::var("GITHUB_APP_SLUG").unwrap_or_else(|_| "ai-manager".into());
    let url = format!("https://github.com/apps/{app_slug}/installations/new");
    (
        StatusCode::OK,
        Json(json!({
            "ready": true,
            "install_url": url
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
        members.push(json!({
            "twin_id": t.twin_id,
            "subject_id": t.subject_id,
            "display_name": t.display_name,
            "enabled": t.enabled,
            "channel_id": t.channel_id,
            "slack_user_id": slack.map(|s| s.slack_user_id.clone()),
            "slack_mapped": slack.is_some(),
            "provider_aliases": aliases,
            "shadow_until": t.shadow_until,
            "last_digest": last_digest,
        }));
    }
    // Do not list alias-only slack map rows (login/numeric keys) as extra members —
    // they were creating empty "ghost" rows and inflated multi-person noise.
    let mapped = members
        .iter()
        .filter(|m| m.get("slack_mapped").and_then(|v| v.as_bool()) == Some(true))
        .count();
    // Unique Slack user IDs among *enabled person twins* (not alias-only map rows).
    // Same human mapped thrice under one Slack must not count as multi-person.
    let mut uniq_slack: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut enabled_person_twins = 0usize;
    for m in &members {
        let enabled = m.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let has_twin = m.get("twin_id").and_then(|v| v.as_str()).is_some();
        if enabled && has_twin {
            enabled_person_twins += 1;
            if let Some(s) = m.get("slack_user_id").and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    uniq_slack.insert(s.to_string());
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
    slack_user_id: String,
    channel_id: Option<String>,
    /// GitHub login / provider ids that should resolve to this Slack user (bridge map).
    provider_aliases: Option<Vec<String>>,
    enabled: Option<bool>,
    skip_shadow: Option<bool>,
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
    if body.slack_user_id.trim().is_empty() {
        return Err(ApiError::bad("slack_user_id required"));
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
    st.store
        .put_slack_map(SlackUserMap {
            tenant_id: tenant_id.clone(),
            global_user_id: body.subject_id.clone(),
            slack_user_id: body.slack_user_id.clone(),
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
                    slack_user_id: body.slack_user_id.clone(),
                    slack_team_id: String::new(),
                })
                .await;
        }
    }
    let _ = prune_duplicate_slack_twins(st.store.as_ref(), &tenant_id).await;
    persist_embedded(&st);
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "twin": twin,
            "slack_user_id": body.slack_user_id,
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
    Ok(Json(draft))
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
    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    match service.explicit_publish(&twin, &tenant_id, &draft_id).await {
        Ok((draft, pub_rec)) => {
            if pub_rec.is_some() {
                st.metrics.publish_ok.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Json(json!({ "draft": draft, "publish": pub_rec })))
        }
        Err(e) => {
            st.metrics.publish_fail.fetch_add(1, Ordering::Relaxed);
            if matches!(e, TwinError::Egress(_)) {
                st.metrics.egress_fail.fetch_add(1, Ordering::Relaxed);
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
    let tenant_id = v
        .pointer("/team/id")
        .and_then(|x| x.as_str())
        .unwrap_or("ten_unknown");

    if draft_id.is_empty() {
        return Ok(Json(json!({ "ok": true, "note": "no draft" })));
    }

    match action {
        "veto" => {
            let _ = twin_delivery::veto_draft(st.store.clone(), tenant_id, draft_id).await;
            st.metrics.veto_total.fetch_add(1, Ordering::Relaxed);
        }
        "publish" => {
            if let Ok(Some(d)) = st.store.get_draft(tenant_id, draft_id).await {
                if let Ok(Some(twin)) = st.store.get_twin(tenant_id, &d.twin_id).await {
                    let service =
                        DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
                    let _ = service.explicit_publish(&twin, tenant_id, draft_id).await;
                }
            }
        }
        _ => {}
    }
    Ok(Json(json!({ "ok": true })))
}

async fn slack_events(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    // URL verification challenge
    if body.get("type").and_then(|t| t.as_str()) == Some("url_verification") {
        return Json(json!({ "challenge": body.get("challenge") }));
    }
    Json(json!({ "ok": true }))
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
