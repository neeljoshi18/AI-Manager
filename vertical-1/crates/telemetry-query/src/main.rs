//! Vertical 1 — ACL-filtered query API for Vertical 2+.
//!
//! Every read enforces query-time group membership (Invariant #2).
//! No downstream agent may access telemetry without satisfying mirrored ACLs.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use telemetry_core::acl::seed_membership;
use telemetry_core::model::{EventQuery, QueryContext};
use telemetry_core::store::derive_pr_state;
use telemetry_core::wiring::{build_from_env, Vertical1Runtime};
use telemetry_proto::{EventCategory, SourceProvider};
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Shared process state. In embedded mode, ingestion and query must share the
/// same runtime instance — use the all-in-one binary or the verify harness.
/// For standalone query in production, both talk to CockroachDB + ClickHouse.
#[derive(Clone)]
struct AppState {
    rt: Arc<Vertical1Runtime>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let rt = Arc::new(build_from_env());
    let bind = rt.config.query_bind.clone();
    let state = AppState { rt };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/tenants/{tenant_id}/events", get(query_events))
        .route("/v1/tenants/{tenant_id}/resource-state", get(resource_state))
        .route("/v1/tenants/{tenant_id}/users", post(seed_user))
        .route("/v1/tenants/{tenant_id}/users/{user_id}/groups", get(get_groups).post(set_groups))
        .route(
            "/v1/tenants/{tenant_id}/users/{user_id}/groups/{group_id}",
            axum::routing::delete(remove_group).put(add_group),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = bind.parse()?;
    info!(%addr, "telemetry-query listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    Json(json!({ "status": "ok", "service": "telemetry-query", "vertical": 1 }))
}

#[derive(Debug, Deserialize)]
struct QueryParams {
    /// Acting user's global_user_id (required for ACL).
    user_id: String,
    resource_id: Option<String>,
    parent_resource_id: Option<String>,
    event_type: Option<String>,
    category: Option<String>,
    provider: Option<String>,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    limit: Option<usize>,
}

async fn query_events(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(params): Query<QueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let groups = state
        .rt
        .acl
        .get_user_groups(&tenant_id, &params.user_id)
        .await
        .map_err(ApiError::from)?;

    let ctx = QueryContext {
        tenant_id: tenant_id.clone(),
        global_user_id: params.user_id.clone(),
        group_ids: groups,
    };

    let categories = params
        .category
        .as_deref()
        .and_then(parse_category)
        .into_iter()
        .collect();
    let providers = params
        .provider
        .as_deref()
        .and_then(SourceProvider::from_str_name_lower)
        .into_iter()
        .collect();

    let filter = EventQuery {
        tenant_id,
        categories,
        providers,
        resource_id: params.resource_id,
        parent_resource_id: params.parent_resource_id,
        event_type: params.event_type,
        since: params.since,
        until: params.until,
        limit: params.limit.unwrap_or(100),
    };

    let events = state
        .rt
        .store
        .query(&ctx, &filter)
        .await
        .map_err(ApiError::from)?;

    Ok(Json(json!({
        "count": events.len(),
        "events": events,
    })))
}

#[derive(Debug, Deserialize)]
struct ResourceStateParams {
    user_id: String,
    resource_id: String,
}

async fn resource_state(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    Query(params): Query<ResourceStateParams>,
) -> Result<impl IntoResponse, ApiError> {
    let groups = state
        .rt
        .acl
        .get_user_groups(&tenant_id, &params.user_id)
        .await
        .map_err(ApiError::from)?;
    let ctx = QueryContext {
        tenant_id,
        global_user_id: params.user_id,
        group_ids: groups,
    };
    let latest = state
        .rt
        .store
        .latest_state_for_resource(&ctx, &params.resource_id)
        .await
        .map_err(ApiError::from)?;

    let state_label = latest
        .as_ref()
        .map(|e| derive_pr_state(&e.event_type));

    Ok(Json(json!({
        "resource_id": params.resource_id,
        "state": state_label,
        "latest_event": latest,
    })))
}

#[derive(Debug, Deserialize)]
struct SeedUserBody {
    provider_user_id: String,
    email: Option<String>,
    display_name: Option<String>,
    groups: Option<Vec<String>>,
}

async fn seed_user(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    Json(body): Json<SeedUserBody>,
) -> Result<impl IntoResponse, ApiError> {
    let groups: Vec<&str> = body
        .groups
        .as_ref()
        .map(|g| g.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();
    let gid = seed_membership(
        state.rt.acl.as_ref(),
        &tenant_id,
        &body.provider_user_id,
        body.email.as_deref().unwrap_or(""),
        body.display_name.as_deref().unwrap_or(""),
        &groups,
    )
    .await
    .map_err(ApiError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "global_user_id": gid, "tenant_id": tenant_id })),
    ))
}

#[derive(Debug, Deserialize)]
struct GroupsBody {
    groups: Vec<String>,
}

async fn get_groups(
    State(state): State<AppState>,
    Path((tenant_id, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let groups = state
        .rt
        .acl
        .get_user_groups(&tenant_id, &user_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "global_user_id": user_id, "groups": groups })))
}

async fn set_groups(
    State(state): State<AppState>,
    Path((tenant_id, user_id)): Path<(String, String)>,
    Json(body): Json<GroupsBody>,
) -> Result<impl IntoResponse, ApiError> {
    let version = state
        .rt
        .acl
        .set_user_groups(&tenant_id, &user_id, &body.groups)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "acl_version": version, "groups": body.groups })))
}

async fn add_group(
    State(state): State<AppState>,
    Path((tenant_id, user_id, group_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let version = state
        .rt
        .acl
        .add_user_to_group(&tenant_id, &user_id, &group_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "acl_version": version, "added": group_id })))
}

async fn remove_group(
    State(state): State<AppState>,
    Path((tenant_id, user_id, group_id)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let version = state
        .rt
        .acl
        .remove_user_from_group(&tenant_id, &user_id, &group_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({ "acl_version": version, "removed": group_id })))
}

fn parse_category(s: &str) -> Option<EventCategory> {
    match s.to_ascii_lowercase().as_str() {
        "code" => Some(EventCategory::Code),
        "work_item" | "work-item" | "workitem" => Some(EventCategory::WorkItem),
        "communication" | "comm" => Some(EventCategory::Communication),
        "identity" => Some(EventCategory::Identity),
        _ => None,
    }
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<telemetry_core::error::CoreError> for ApiError {
    fn from(e: telemetry_core::error::CoreError) -> Self {
        use telemetry_core::error::CoreError;
        let status = match &e {
            CoreError::AclDenied(_) => StatusCode::FORBIDDEN,
            CoreError::NotFound(_) => StatusCode::NOT_FOUND,
            CoreError::Validation(_) => StatusCode::BAD_REQUEST,
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
