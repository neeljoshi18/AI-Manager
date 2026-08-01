//! Vertical 2 Graph API — ACL-safe multi-hop context queries + projection ingest.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use graph_core::config::GraphConfig;
use graph_core::ids::pr_node_id;
use graph_core::membership::{InMemoryMembership, MembershipStore};
use graph_core::model::{ProjectOutcome, ProjectStatus, QueryContext};
use graph_core::project::ProjectEngine;
use graph_core::store::{GraphStore, InMemoryGraphStore};
use graph_core::v1_event::{V1AclRevocation, V1CanonicalEvent};
use graph_core::GraphError;
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Default)]
struct Metrics {
    projects_applied: AtomicU64,
    projects_duplicate: AtomicU64,
    projects_skipped: AtomicU64,
    projects_error: AtomicU64,
    projects_timeout: AtomicU64,
    projects_busy: AtomicU64,
    acl_revocations: AtomicU64,
}

#[derive(Clone)]
struct AppState {
    engine: Arc<ProjectEngine>,
    store: Arc<dyn GraphStore>,
    membership: Arc<dyn MembershipStore>,
    max_hops: usize,
    default_hops: usize,
    mode: String,
    metrics: Arc<Metrics>,
    /// Cap concurrent projections so healthz stays responsive on small VPS.
    project_sem: Arc<Semaphore>,
    project_timeout: Duration,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cfg = GraphConfig::from_env();
    let state = build_state(cfg.clone()).await?;

    // Health routes stay outside heavy work paths (always fast).
    let health = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(state.clone());

    let api = Router::new()
        .route("/metrics", get(metrics))
        .route("/v2/project", post(project_event))
        .route("/v2/project/acl", post(project_acl))
        .route("/v2/tenants/{tenant_id}/users", post(seed_user))
        .route(
            "/v2/tenants/{tenant_id}/users/{user_id}/groups/{group_id}",
            axum::routing::put(add_group).delete(remove_group),
        )
        .route("/v2/tenants/{tenant_id}/node", get(get_node))
        .route("/v2/tenants/{tenant_id}/neighborhood", get(neighborhood))
        .route("/v2/tenants/{tenant_id}/path", get(path_query))
        .route("/v2/tenants/{tenant_id}/state", get(state_query))
        .route("/v2/tenants/{tenant_id}/blockers", get(blockers))
        .route("/v2/tenants/{tenant_id}/intents", get(list_intents))
        .route("/v2/tenants/{tenant_id}/conflicts", get(list_conflicts))
        .route(
            "/v2/tenants/{tenant_id}/seed/intent_demo",
            post(seed_intent_demo),
        )
        .route("/v2/tenants/{tenant_id}/snapshot", get(graph_snapshot))
        .route("/v2/tenants/{tenant_id}/stats", get(stats))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let app = health.merge(api);

    let addr: SocketAddr = cfg.http_bind.parse()?;
    info!(%addr, mode = %cfg.runtime_mode, "graph-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn build_state(cfg: GraphConfig) -> anyhow::Result<AppState> {
    let metrics = Arc::new(Metrics::default());
    // 2 concurrent projects max keeps /healthz snappy on 2vCPU staging.
    let project_limit: usize = std::env::var("GRAPH_PROJECT_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
        .clamp(1, 8);
    let project_timeout_secs: u64 = std::env::var("GRAPH_PROJECT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10)
        .clamp(3, 60);
    let project_sem = Arc::new(Semaphore::new(project_limit));
    let project_timeout = Duration::from_secs(project_timeout_secs);
    info!(
        project_limit,
        project_timeout_secs, "graph project concurrency limits"
    );

    if cfg.is_embedded() {
        info!("runtime mode=embedded");
        let mem = InMemoryGraphStore::new();
        if let Ok(p) = std::env::var("GRAPH_EMBEDDED_STATE_PATH") {
            let path = std::path::PathBuf::from(p);
            match mem.load_from_path(&path) {
                Ok(true) => info!(path = %path.display(), "restored embedded graph state"),
                Ok(false) => info!(path = %path.display(), "no graph state file yet"),
                Err(e) => tracing::warn!(error = %e, "graph state load failed"),
            }
            mem.set_persist_path(Some(path));
        }
        let store: Arc<dyn GraphStore> = mem;
        let membership: Arc<dyn MembershipStore> = InMemoryMembership::new();
        let engine = Arc::new(ProjectEngine::new(store.clone(), membership.clone()));
        return Ok(AppState {
            engine,
            store,
            membership,
            max_hops: cfg.max_hops,
            default_hops: cfg.default_hops,
            mode: "embedded".into(),
            metrics,
            project_sem,
            project_timeout,
        });
    }

    use graph_core::membership_v1::HybridMembership;
    use graph_core::store_crdb::{CrdbGraphStore, CrdbMembership};
    let url = cfg
        .cockroach_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("COCKROACH_URL required for production mode"))?;
    info!(%url, "connecting cockroach context_graph");
    let store: Arc<dyn GraphStore> = CrdbGraphStore::connect(&url).await?;
    let local_mem: Arc<dyn MembershipStore> = CrdbMembership::connect(&url).await?;
    // Production always uses HybridMembership; V1 identity when V1_COCKROACH_URL is set.
    let membership: Arc<dyn MembershipStore> =
        if let Some(v1_url) = cfg.v1_cockroach_url.as_deref() {
            info!(%v1_url, "live ACL groups from Vertical 1 identity tables");
            HybridMembership::with_v1_identity(local_mem, v1_url).await?
        } else {
            HybridMembership::local_only(local_mem)
        };
    let engine = Arc::new(ProjectEngine::new(store.clone(), membership.clone()));
    Ok(AppState {
        engine,
        store,
        membership,
        max_hops: cfg.max_hops,
        default_hops: cfg.default_hops,
        mode: "production".into(),
        metrics,
        project_sem,
        project_timeout,
    })
}

fn record_project_outcome(m: &Metrics, out: &ProjectOutcome) {
    match out.status {
        ProjectStatus::Applied => {
            m.projects_applied.fetch_add(1, Ordering::Relaxed);
        }
        ProjectStatus::Duplicate => {
            m.projects_duplicate.fetch_add(1, Ordering::Relaxed);
        }
        ProjectStatus::Skipped => {
            m.projects_skipped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok", "service": "graph-api", "vertical": 2 }))
}

async fn readyz(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({ "status": "ready", "mode": st.mode }))
}

async fn metrics(State(st): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "service": "graph-api",
        "mode": st.mode,
        "projects_applied": st.metrics.projects_applied.load(Ordering::Relaxed),
        "projects_duplicate": st.metrics.projects_duplicate.load(Ordering::Relaxed),
        "projects_skipped": st.metrics.projects_skipped.load(Ordering::Relaxed),
        "projects_error": st.metrics.projects_error.load(Ordering::Relaxed),
        "projects_timeout": st.metrics.projects_timeout.load(Ordering::Relaxed),
        "projects_busy": st.metrics.projects_busy.load(Ordering::Relaxed),
        "project_permits_available": st.project_sem.available_permits(),
        "acl_revocations": st.metrics.acl_revocations.load(Ordering::Relaxed),
    }))
}

async fn project_event(
    State(st): State<AppState>,
    Json(event): Json<V1CanonicalEvent>,
) -> Result<Json<ProjectOutcome>, ApiError> {
    // Non-blocking try: if saturated, return 503 so bridge backs off instead of wedging.
    let permit = match st.project_sem.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            st.metrics.projects_busy.fetch_add(1, Ordering::Relaxed);
            return Err(ApiError {
                status: StatusCode::SERVICE_UNAVAILABLE,
                message: "graph project concurrency limit; retry".into(),
            });
        }
    };
    let result = tokio::time::timeout(st.project_timeout, st.engine.project_event(&event)).await;
    drop(permit);
    match result {
        Ok(Ok(out)) => {
            record_project_outcome(&st.metrics, &out);
            Ok(Json(out))
        }
        Ok(Err(e)) => {
            st.metrics.projects_error.fetch_add(1, Ordering::Relaxed);
            Err(ApiError::from(e))
        }
        Err(_elapsed) => {
            st.metrics.projects_timeout.fetch_add(1, Ordering::Relaxed);
            Err(ApiError {
                status: StatusCode::GATEWAY_TIMEOUT,
                message: format!(
                    "project timed out after {}s",
                    st.project_timeout.as_secs()
                ),
            })
        }
    }
}

async fn project_acl(
    State(st): State<AppState>,
    Json(rev): Json<V1AclRevocation>,
) -> Result<Json<ProjectOutcome>, ApiError> {
    match st.engine.project_acl_revocation(&rev).await {
        Ok(out) => {
            st.metrics.acl_revocations.fetch_add(1, Ordering::Relaxed);
            record_project_outcome(&st.metrics, &out);
            Ok(Json(out))
        }
        Err(e) => {
            st.metrics.projects_error.fetch_add(1, Ordering::Relaxed);
            Err(ApiError::from(e))
        }
    }
}

#[derive(Deserialize)]
struct SeedUser {
    global_user_id: String,
    groups: Vec<String>,
}

async fn seed_user(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<SeedUser>,
) -> Result<impl IntoResponse, ApiError> {
    st.membership
        .set_groups(&tenant_id, &body.global_user_id, &body.groups)
        .await
        .map_err(ApiError::from)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "global_user_id": body.global_user_id, "groups": body.groups })),
    ))
}

async fn add_group(
    State(st): State<AppState>,
    Path((tenant_id, user_id, group_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    st.membership
        .add_group(&tenant_id, &user_id, &group_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "added": group_id })))
}

async fn remove_group(
    State(st): State<AppState>,
    Path((tenant_id, user_id, group_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    st.membership
        .remove_group(&tenant_id, &user_id, &group_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "removed": group_id })))
}

#[derive(Deserialize)]
struct NodeQ {
    user_id: String,
    node_id: String,
}

async fn get_node(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<NodeQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let node = st
        .store
        .get_node(&ctx, &q.node_id)
        .await
        .map_err(ApiError::from)?;
    match node {
        Some(n) => Ok(Json(json!({ "node": n }))),
        None => Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "not found or acl denied".into(),
        }),
    }
}

#[derive(Deserialize)]
struct NeighborhoodQ {
    user_id: String,
    node_id: String,
    hops: Option<usize>,
}

async fn neighborhood(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<NeighborhoodQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let hops = q.hops.unwrap_or(st.default_hops).min(st.max_hops);
    let nb = st
        .store
        .neighborhood(&ctx, &q.node_id, hops)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(nb))
}

#[derive(Deserialize)]
struct PathQ {
    user_id: String,
    from: String,
    to: String,
    max_hops: Option<usize>,
}

async fn path_query(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<PathQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let max_hops = q.max_hops.unwrap_or(st.default_hops).min(st.max_hops);
    let path = st
        .store
        .path(&ctx, &q.from, &q.to, max_hops)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "path": path })))
}

#[derive(Deserialize)]
struct StateQ {
    user_id: String,
    node_id: String,
    state_key: Option<String>,
}

async fn state_query(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<StateQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let key = q.state_key.unwrap_or_else(|| "lifecycle".into());
    // Convenience: accept bare resource id for PRs
    let node_id = if q.node_id.starts_with("pr:") || q.node_id.starts_with("person:") {
        q.node_id
    } else if q.node_id.contains("/pr/") {
        pr_node_id(&q.node_id)
    } else {
        q.node_id
    };
    let stt = st
        .store
        .get_state(&ctx, &node_id, &key)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "state": stt })))
}

#[derive(Deserialize)]
struct BlockersQ {
    user_id: String,
    node_id: String,
}

async fn blockers(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<BlockersQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let edges = st
        .store
        .blockers(&ctx, &q.node_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "blockers": edges })))
}

#[derive(Deserialize)]
struct IntentsQ {
    user_id: String,
    limit: Option<usize>,
}

/// List Intent nodes visible to user (rules-classified claims).
async fn list_intents(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<IntentsQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let limit = q.limit.unwrap_or(100);
    let intents = st
        .store
        .list_nodes_by_type(&ctx, "Intent", limit)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "count": intents.len(),
        "intents": intents,
    })))
}

#[derive(Deserialize)]
struct ConflictsQ {
    user_id: String,
    limit: Option<usize>,
}

/// Conflict detector v0: dual owners, SHIP vs FREEZE, BLOCKS, open BLOCKED intents.
async fn list_conflicts(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<ConflictsQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    // Gather intent + work neighborhood via type lists + BLOCKS edges
    let intents = st
        .store
        .list_nodes_by_type(&ctx, "Intent", 300)
        .await
        .map_err(ApiError::from)?;
    let blocks = st
        .store
        .list_edges_by_type(&ctx, "BLOCKS", 300)
        .await
        .map_err(ApiError::from)?;
    let about = st
        .store
        .list_edges_by_type(&ctx, "ABOUT", 300)
        .await
        .map_err(ApiError::from)?;
    let claims = st
        .store
        .list_edges_by_type(&ctx, "CLAIMS", 300)
        .await
        .map_err(ApiError::from)?;
    let mut nodes = intents;
    // Pull endpoints of BLOCKS for context (best-effort)
    for e in blocks.iter().chain(about.iter()) {
        for nid in [&e.from_node_id, &e.to_node_id] {
            if nodes.iter().any(|n| &n.node_id == nid) {
                continue;
            }
            if let Ok(Some(n)) = st.store.get_node(&ctx, nid).await {
                nodes.push(n);
            }
        }
    }
    let mut edges = blocks;
    edges.extend(about);
    edges.extend(claims);
    let mut cards = graph_core::intent::detect_conflicts(&tenant_id, &nodes, &edges, chrono::Utc::now());
    cards.truncate(limit);
    Ok(Json(json!({
        "tenant_id": tenant_id,
        "count": cards.len(),
        "conflicts": cards,
        "engine": "rules_v0",
    })))
}

async fn stats(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let nodes = st.store.count_nodes(&tenant_id).await.map_err(ApiError::from)?;
    let edges = st.store.count_edges(&tenant_id).await.map_err(ApiError::from)?;
    Ok(Json(json!({ "nodes": nodes, "edges": edges })))
}

/// Seed multi-person intent + conflict cards for pilot UI (rules_v0 proof).
/// Idempotent-ish: stable node/edge ids overwrite same entities.
async fn seed_intent_demo(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    use graph_core::ids::stable_edge_id;
    use graph_core::model::{GraphEdge, GraphMutation, GraphNode};
    let now = chrono::Utc::now();
    let groups = vec!["grp_eng".to_string(), "grp_default".to_string()];
    let p1 = format!("person:gu_demo_alice");
    let p2 = format!("person:gu_demo_bob");
    let pr = format!("pr:{tenant_id}/demo-repo/pr/42");
    let repo = format!("repo:{tenant_id}/demo-repo");
    let i_ship = format!("intent:{p1}:{pr}");
    let i_freeze = format!("intent:{p2}:{pr}");
    let i_blocked = format!("intent:{p1}:{pr}:blocked");

    // Ensure membership for ACL readers
    let group_ids = vec!["grp_eng".to_string(), "grp_default".to_string()];
    for uid in ["gu_demo_alice", "gu_demo_bob", "bridge_reader"] {
        let _ = st
            .membership
            .set_groups(&tenant_id, uid, &group_ids)
            .await;
    }

    let mk_person = |id: &str, name: &str| GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: id.into(),
        node_type: "Person".into(),
        display_name: name.into(),
        resource_id: name.into(),
        properties: json!({ "seed": "intent_demo" }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };
    let pr_node = GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: pr.clone(),
        node_type: "PullRequest".into(),
        display_name: "Ship release — DO NOT MERGE until freeze lifts".into(),
        resource_id: format!("{tenant_id}/demo-repo/pr/42"),
        properties: json!({ "title": "Ship release — DO NOT MERGE until freeze lifts", "state": "OPEN", "seed": "intent_demo" }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };
    let repo_node = GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: repo.clone(),
        node_type: "Repo".into(),
        display_name: format!("{tenant_id}/demo-repo"),
        resource_id: format!("{tenant_id}/demo-repo"),
        properties: json!({ "seed": "intent_demo" }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };
    let intent_ship = GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: i_ship.clone(),
        node_type: "Intent".into(),
        display_name: "SHIP: ready to ship release".into(),
        resource_id: pr_node.resource_id.clone(),
        properties: json!({
            "intent_type": "SHIP",
            "confidence": 0.9,
            "evidence": ["text:ready to ship", "seed:intent_demo"],
            "about_node_id": pr,
            "owner_node_id": p1,
            "classified_by": "rules_v0",
        }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };
    let intent_freeze = GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: i_freeze.clone(),
        node_type: "Intent".into(),
        display_name: "FREEZE: code freeze — do not merge".into(),
        resource_id: pr_node.resource_id.clone(),
        properties: json!({
            "intent_type": "FREEZE",
            "confidence": 0.95,
            "evidence": ["text:do not merge", "label:freeze", "seed:intent_demo"],
            "about_node_id": pr,
            "owner_node_id": p2,
            "classified_by": "rules_v0",
        }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };
    let intent_blocked = GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: i_blocked.clone(),
        node_type: "Intent".into(),
        display_name: "BLOCKED: waiting on security review".into(),
        resource_id: pr_node.resource_id.clone(),
        properties: json!({
            "intent_type": "BLOCKED",
            "confidence": 0.9,
            "evidence": ["text:blocked on", "seed:intent_demo"],
            "about_node_id": pr,
            "owner_node_id": p1,
            "classified_by": "rules_v0",
        }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };

    let edge = |etype: &str, from: &str, to: &str| GraphEdge {
        tenant_id: tenant_id.clone(),
        edge_id: stable_edge_id(&tenant_id, etype, from, to),
        edge_type: etype.into(),
        from_node_id: from.into(),
        to_node_id: to.into(),
        valid_from: now,
        valid_to: None,
        event_id: format!("seed-intent-demo-{etype}"),
        properties: json!({ "seed": "intent_demo" }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    };

    let mut m = GraphMutation {
        nodes: vec![
            mk_person(&p1, "alice"),
            mk_person(&p2, "bob"),
            repo_node,
            pr_node,
            intent_ship,
            intent_freeze,
            intent_blocked,
        ],
        edges: vec![
            edge("AUTHORED", &p1, &pr),
            edge("BELONGS_TO", &pr, &repo),
            edge("CLAIMS", &p1, &i_ship),
            edge("CLAIMS", &p2, &i_freeze),
            edge("CLAIMS", &p1, &i_blocked),
            edge("ABOUT", &i_ship, &pr),
            edge("ABOUT", &i_freeze, &pr),
            edge("ABOUT", &i_blocked, &pr),
            // open blocker edge for dual-blocks / open blocker surface
            edge("BLOCKS", &pr, &format!("pr:{tenant_id}/demo-repo/pr/7")),
        ],
        states: vec![],
    };
    // Second PR blocked by first
    m.nodes.push(GraphNode {
        tenant_id: tenant_id.clone(),
        node_id: format!("pr:{tenant_id}/demo-repo/pr/7"),
        node_type: "PullRequest".into(),
        display_name: "Depends on release PR".into(),
        resource_id: format!("{tenant_id}/demo-repo/pr/7"),
        properties: json!({ "title": "Depends on release PR", "seed": "intent_demo" }),
        is_private: false,
        allowed_group_ids: groups.clone(),
        acl_version: 1,
    });

    st.store
        .apply_mutation(m)
        .await
        .map_err(ApiError::from)?;

    // Verify conflicts visible
    let ctx = ctx_for(&st, &tenant_id, "gu_demo_alice").await?;
    let intents = st
        .store
        .list_nodes_by_type(&ctx, "Intent", 50)
        .await
        .map_err(ApiError::from)?;
    let blocks = st
        .store
        .list_edges_by_type(&ctx, "BLOCKS", 50)
        .await
        .map_err(ApiError::from)?;
    let about = st
        .store
        .list_edges_by_type(&ctx, "ABOUT", 50)
        .await
        .map_err(ApiError::from)?;
    let claims = st
        .store
        .list_edges_by_type(&ctx, "CLAIMS", 50)
        .await
        .map_err(ApiError::from)?;
    let mut nodes = intents.clone();
    for e in blocks.iter().chain(about.iter()) {
        for nid in [&e.from_node_id, &e.to_node_id] {
            if nodes.iter().any(|n| &n.node_id == nid) {
                continue;
            }
            if let Ok(Some(n)) = st.store.get_node(&ctx, nid).await {
                nodes.push(n);
            }
        }
    }
    let mut edges = blocks;
    edges.extend(about);
    edges.extend(claims);
    let cards =
        graph_core::intent::detect_conflicts(&tenant_id, &nodes, &edges, chrono::Utc::now());

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "seeded": true,
        "intent_count": intents.len(),
        "conflict_count": cards.len(),
        "conflicts": cards,
        "engine": "rules_v0",
        "note": "SHIP vs FREEZE dual owners + BLOCKED + BLOCKS edge for Team blockers UI",
    })))
}

#[derive(Deserialize)]
struct SnapshotQ {
    user_id: String,
    node_limit: Option<usize>,
    edge_limit: Option<usize>,
    /// When true, include `intent_demo` alice/bob seed graph. Default **false** for pilot Graph UI.
    include_demo: Option<bool>,
}

/// Intent-demo seed nodes (alice/bob, demo-repo PR, seed intents) — not real humans.
fn is_demo_seed_node(n: &graph_core::model::GraphNode) -> bool {
    let id = n.node_id.to_ascii_lowercase();
    let lab = n.display_name.to_ascii_lowercase();
    let resource = n.resource_id.to_ascii_lowercase();
    if n.properties
        .get("seed")
        .and_then(|v| v.as_str())
        .map(|s| s == "intent_demo")
        .unwrap_or(false)
    {
        return true;
    }
    if id.contains("gu_demo_")
        || id.contains("demo-repo")
        || id.contains("/demo-repo/")
        || resource.contains("demo-repo")
    {
        return true;
    }
    // Seed people only (not real humans named Alice)
    if n.node_type.eq_ignore_ascii_case("Person")
        && (lab == "alice" || lab == "bob")
        && (id.contains("demo") || resource == "alice" || resource == "bob")
    {
        return true;
    }
    false
}

/// ACL-safe full-ish graph for product Graph UI (live map of ingested signals).
async fn graph_snapshot(
    State(st): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(q): Query<SnapshotQ>,
) -> Result<impl IntoResponse, ApiError> {
    let ctx = ctx_for(&st, &tenant_id, &q.user_id).await?;
    let node_limit = q.node_limit.unwrap_or(400);
    let edge_limit = q.edge_limit.unwrap_or(800);
    let include_demo = q.include_demo.unwrap_or(false);
    let (nodes_raw, edges_raw) = st
        .store
        .snapshot(&ctx, node_limit, edge_limit)
        .await
        .map_err(ApiError::from)?;
    let total_nodes = st.store.count_nodes(&tenant_id).await.map_err(ApiError::from)?;
    let total_edges = st.store.count_edges(&tenant_id).await.map_err(ApiError::from)?;

    // Server-side hide demo seed by default (product Graph is pilot-real, not theater).
    let demo_ids: std::collections::HashSet<String> = if include_demo {
        std::collections::HashSet::new()
    } else {
        nodes_raw
            .iter()
            .filter(|n| is_demo_seed_node(n))
            .map(|n| n.node_id.clone())
            .collect()
    };
    let nodes: Vec<_> = nodes_raw
        .into_iter()
        .filter(|n| !demo_ids.contains(&n.node_id))
        .collect();
    let edges: Vec<_> = edges_raw
        .into_iter()
        .filter(|e| {
            !demo_ids.contains(&e.from_node_id) && !demo_ids.contains(&e.to_node_id)
        })
        .collect();
    let demo_hidden = demo_ids.len();

    let mut by_type: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for n in &nodes {
        *by_type.entry(n.node_type.clone()).or_insert(0) += 1;
    }
    let mut edge_by_type: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for e in &edges {
        *edge_by_type.entry(e.edge_type.clone()).or_insert(0) += 1;
    }

    // Collapse Person duplicates that share the same display_name (e.g. multiple gu_*
    // after embedded identity resets). Prefer person with highest edge degree, then
    // numeric GitHub resource_id.
    let mut degree: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in &edges {
        *degree.entry(e.from_node_id.clone()).or_insert(0) += 1;
        *degree.entry(e.to_node_id.clone()).or_insert(0) += 1;
    }
    let mut by_label: std::collections::HashMap<String, Vec<&graph_core::model::GraphNode>> =
        std::collections::HashMap::new();
    for n in &nodes {
        if !n.node_type.eq_ignore_ascii_case("Person") {
            continue;
        }
        let lab = if n.display_name.is_empty() {
            n.node_id.to_ascii_lowercase()
        } else {
            n.display_name.to_ascii_lowercase()
        };
        by_label.entry(lab).or_default().push(n);
    }
    let mut person_alias: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut drop_persons: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_lab, group) in by_label {
        if group.len() <= 1 {
            continue;
        }
        let mut ranked = group;
        ranked.sort_by(|a, b| {
            let score = |n: &graph_core::model::GraphNode| {
                let deg = *degree.get(&n.node_id).unwrap_or(&0);
                let numeric = if n.resource_id.chars().all(|c| c.is_ascii_digit()) {
                    1usize
                } else {
                    0
                };
                let not_seed = if n.node_id.contains("gu_seed_") { 0usize } else { 1 };
                (deg, numeric, not_seed)
            };
            score(b).cmp(&score(a))
        });
        let canonical = ranked[0].node_id.clone();
        for n in ranked.into_iter().skip(1) {
            person_alias.insert(n.node_id.clone(), canonical.clone());
            drop_persons.insert(n.node_id.clone());
        }
    }

    // Light view models for UI (trim heavy properties)
    let node_views: Vec<serde_json::Value> = nodes
        .iter()
        .filter(|n| !drop_persons.contains(&n.node_id))
        .map(|n| {
            let intent_type = n
                .properties
                .get("intent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = n
                .properties
                .get("title")
                .or_else(|| n.properties.get("summary"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            json!({
                "id": n.node_id,
                "type": n.node_type,
                "label": if n.display_name.is_empty() { n.node_id.clone() } else { n.display_name.clone() },
                "resource_id": n.resource_id,
                "intent_type": intent_type,
                "title": title,
                "is_private": n.is_private,
            })
        })
        .collect();
    let edge_views: Vec<serde_json::Value> = edges
        .iter()
        .map(|e| {
            let from = person_alias
                .get(&e.from_node_id)
                .cloned()
                .unwrap_or_else(|| e.from_node_id.clone());
            let to = person_alias
                .get(&e.to_node_id)
                .cloned()
                .unwrap_or_else(|| e.to_node_id.clone());
            json!({
                "id": e.edge_id,
                "type": e.edge_type,
                "from": from,
                "to": to,
                "event_id": e.event_id,
                "valid_from": e.valid_from,
            })
        })
        .collect();

    Ok(Json(json!({
        "tenant_id": tenant_id,
        "reader": q.user_id,
        "as_of": chrono::Utc::now().to_rfc3339(),
        "totals": { "nodes": total_nodes, "edges": total_edges },
        "returned": { "nodes": node_views.len(), "edges": edge_views.len() },
        "by_type": by_type,
        "edge_by_type": edge_by_type,
        "nodes": node_views,
        "edges": edge_views,
        "person_aliases_collapsed": person_alias.len(),
        "demo_hidden": demo_hidden,
        "include_demo": include_demo,
        "truncated": total_nodes as usize > node_views.len() + demo_hidden
            || total_edges as usize > edge_views.len(),
        "engine": "acl_snapshot_v0",
    })))
}

#[cfg(test)]
mod demo_filter_tests {
    use super::is_demo_seed_node;
    use graph_core::model::GraphNode;
    use serde_json::json;

    fn node(id: &str, ntype: &str, name: &str, resource: &str, seed: bool) -> GraphNode {
        GraphNode {
            tenant_id: "ten_t".into(),
            node_id: id.into(),
            node_type: ntype.into(),
            display_name: name.into(),
            resource_id: resource.into(),
            properties: if seed {
                json!({ "seed": "intent_demo" })
            } else {
                json!({})
            },
            is_private: false,
            allowed_group_ids: vec![],
            acl_version: 1,
        }
    }

    #[test]
    fn flags_seed_alice_and_demo_pr() {
        assert!(is_demo_seed_node(&node(
            "person:gu_demo_alice",
            "Person",
            "alice",
            "alice",
            true
        )));
        assert!(is_demo_seed_node(&node(
            "pr:ten_github/demo-repo/pr/42",
            "PullRequest",
            "Ship",
            "ten_github/demo-repo/pr/42",
            true
        )));
        assert!(!is_demo_seed_node(&node(
            "person:gu_real",
            "Person",
            "neeljoshi18",
            "222674398",
            false
        )));
    }
}

async fn ctx_for(
    st: &AppState,
    tenant_id: &str,
    user_id: &str,
) -> Result<QueryContext, ApiError> {
    let groups = st
        .membership
        .get_groups(tenant_id, user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(QueryContext {
        tenant_id: tenant_id.to_string(),
        global_user_id: user_id.to_string(),
        group_ids: groups,
    })
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<GraphError> for ApiError {
    fn from(e: GraphError) -> Self {
        let status = match &e {
            GraphError::NotFound(_) => StatusCode::NOT_FOUND,
            GraphError::AclDenied(_) => StatusCode::FORBIDDEN,
            GraphError::Validation(_) => StatusCode::BAD_REQUEST,
            GraphError::DuplicateEvent(_) => StatusCode::OK,
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
