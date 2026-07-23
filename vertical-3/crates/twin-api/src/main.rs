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
}

#[derive(Clone)]
struct AppState {
    store: Arc<dyn TwinStore>,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cfg = TwinConfig::from_env();
    let state = build_state(cfg.clone()).await?;

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
        .route("/v3/tenants/{tenant_id}/twins", post(upsert_twin))
        .route(
            "/v3/tenants/{tenant_id}/twins/{twin_id}",
            get(get_twin),
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
    let policy = DeliveryPolicy {
        medium_veto_window_secs: cfg.medium_veto_window_secs,
        blocker_veto_window_secs: cfg.blocker_veto_window_secs,
    };

    let last_demo = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let last_notify = Arc::new(Mutex::new(std::collections::HashMap::new()));

    if cfg.is_embedded() {
        info!("runtime mode=embedded");
        let store: Arc<dyn TwinStore> = InMemoryTwinStore::new();
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
    })
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
    for tenant in tenants {
        let twins = st.store.list_twins(&tenant).await.unwrap_or_default();
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
            let allow_notify = should_notify && !empty;

            let service =
                DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
            let draft = service
                .start_after_compile_opts(
                    &twin,
                    &outcome.ledger,
                    &outcome.draft_text,
                    now,
                    allow_notify,
                )
                .await?;
            if allow_notify && !draft.slack_dm_ts.is_empty() {
                st.last_notify.lock().insert(key, now);
                st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
                info!(
                    twin = %twin.twin_id,
                    ledger = %outcome.ledger.ledger_id,
                    "scheduled status DM sent"
                );
            }
        }
    }
    Ok(())
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
    probe_json(url).await.is_some()
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
    res.json().await.ok()
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
    let opts = CompileOpts {
        period_start,
        period_end,
        hops: 2,
    };
    let outcome = st
        .compiler
        .compile_person(&twin, &opts)
        .await
        .map_err(ApiError::from)?;
    st.metrics.compile_ok.fetch_add(1, Ordering::Relaxed);

    // Demo button is an explicit on-demand notify tool
    let service = DeliveryService::new(st.store.clone(), st.slack.clone(), st.policy.clone());
    let draft = service
        .start_after_compile_opts(&twin, &outcome.ledger, &outcome.draft_text, now, true)
        .await
        .map_err(ApiError::from)?;
    if !draft.slack_dm_ts.is_empty() {
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
    Json(json!({
        "service": "twin-api",
        "mode": st.mode,
        "twin_compile_total_ok": st.metrics.compile_ok.load(Ordering::Relaxed),
        "twin_compile_total_error": st.metrics.compile_error.load(Ordering::Relaxed),
        "twin_drafts_sent_total": st.metrics.drafts_sent.load(Ordering::Relaxed),
        "twin_veto_total": st.metrics.veto_total.load(Ordering::Relaxed),
        "twin_publish_total_ok": st.metrics.publish_ok.load(Ordering::Relaxed),
        "twin_publish_total_fail": st.metrics.publish_fail.load(Ordering::Relaxed),
        "twin_acl_empty_total": st.metrics.acl_empty.load(Ordering::Relaxed),
        "twin_egress_fail_total": st.metrics.egress_fail.load(Ordering::Relaxed),
    }))
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
    let start = body.period_start.unwrap_or(aligned_start);
    let end = body.period_end.unwrap_or(aligned_end);
    let opts = CompileOpts {
        period_start: start,
        period_end: end,
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
    let draft = service
        .start_after_compile_opts(
            &twin,
            &outcome.ledger,
            &outcome.draft_text,
            now,
            allow_notify,
        )
        .await
        .map_err(ApiError::from)?;

    if allow_notify && !draft.slack_dm_ts.is_empty() {
        st.last_notify.lock().insert(key, now);
        st.metrics.drafts_sent.fetch_add(1, Ordering::Relaxed);
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
