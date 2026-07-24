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
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Default)]
struct Metrics {
    projects_applied: AtomicU64,
    projects_duplicate: AtomicU64,
    projects_skipped: AtomicU64,
    projects_error: AtomicU64,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cfg = GraphConfig::from_env();
    let state = build_state(cfg.clone()).await?;

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
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
        .route("/v2/tenants/{tenant_id}/snapshot", get(graph_snapshot))
        .route("/v2/tenants/{tenant_id}/stats", get(stats))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = cfg.http_bind.parse()?;
    info!(%addr, mode = %cfg.runtime_mode, "graph-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn build_state(cfg: GraphConfig) -> anyhow::Result<AppState> {
    let metrics = Arc::new(Metrics::default());
    if cfg.is_embedded() {
        info!("runtime mode=embedded");
        let store: Arc<dyn GraphStore> = InMemoryGraphStore::new();
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
        "acl_revocations": st.metrics.acl_revocations.load(Ordering::Relaxed),
    }))
}

async fn project_event(
    State(st): State<AppState>,
    Json(event): Json<V1CanonicalEvent>,
) -> Result<Json<ProjectOutcome>, ApiError> {
    match st.engine.project_event(&event).await {
        Ok(out) => {
            record_project_outcome(&st.metrics, &out);
            Ok(Json(out))
        }
        Err(e) => {
            st.metrics.projects_error.fetch_add(1, Ordering::Relaxed);
            Err(ApiError::from(e))
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

#[derive(Deserialize)]
struct SnapshotQ {
    user_id: String,
    node_limit: Option<usize>,
    edge_limit: Option<usize>,
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
    let (nodes, edges) = st
        .store
        .snapshot(&ctx, node_limit, edge_limit)
        .await
        .map_err(ApiError::from)?;
    let total_nodes = st.store.count_nodes(&tenant_id).await.map_err(ApiError::from)?;
    let total_edges = st.store.count_edges(&tenant_id).await.map_err(ApiError::from)?;

    let mut by_type: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for n in &nodes {
        *by_type.entry(n.node_type.clone()).or_insert(0) += 1;
    }
    let mut edge_by_type: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for e in &edges {
        *edge_by_type.entry(e.edge_type.clone()).or_insert(0) += 1;
    }

    // Light view models for UI (trim heavy properties)
    let node_views: Vec<serde_json::Value> = nodes
        .iter()
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
            json!({
                "id": e.edge_id,
                "type": e.edge_type,
                "from": e.from_node_id,
                "to": e.to_node_id,
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
        "truncated": total_nodes as usize > node_views.len() || total_edges as usize > edge_views.len(),
        "engine": "acl_snapshot_v0",
    })))
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
