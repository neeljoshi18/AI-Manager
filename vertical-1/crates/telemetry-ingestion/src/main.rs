//! Vertical 1 — Webhook Ingestion Edge (Rust / Axum).
//!
//! Accepts provider webhooks, authenticates, rate-limits, deduplicates,
//! normalizes to Protobuf-aligned canonical events, and durably enqueues
//! to the streaming bus before returning HTTP 200.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use telemetry_core::acl::seed_membership;
use telemetry_core::error::CoreError;
use telemetry_core::model::{EventQuery, QueryContext, TenantConfig};
use telemetry_core::pipeline::{IngestHeaders, IngestRequest};
use telemetry_core::store::derive_pr_state;
use telemetry_core::wiring::{build_from_env, Vertical1Runtime};
use telemetry_proto::{EventCategory, SourceProvider};
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use chrono::{DateTime, Utc};

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
    let bind = rt.config.http_bind.clone();
    let mode = rt.config.runtime_mode.clone();
    let state = AppState { rt };

    // In embedded mode the ingestion process also exposes ACL-filtered query
    // routes so a single binary is enough for local demos (shared in-memory store).
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics_handler))
        .route("/v1/tenants", post(upsert_tenant))
        .route("/v1/tenants/{tenant_id}/webhooks/{provider}", post(ingest_webhook))
        .route("/v1/ingest/{tenant_id}/{provider}", post(ingest_webhook))
        .route("/v1/tenants/{tenant_id}/events", get(query_events))
        .route("/v1/tenants/{tenant_id}/resource-state", get(resource_state))
        .route("/v1/tenants/{tenant_id}/users", post(seed_user))
        .route(
            "/v1/tenants/{tenant_id}/users/{user_id}/groups",
            get(get_groups).post(set_groups),
        )
        .route(
            "/v1/tenants/{tenant_id}/users/{user_id}/groups/{group_id}",
            axum::routing::delete(remove_group).put(add_group),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = bind.parse()?;
    info!(%addr, %mode, "telemetry-ingestion starting");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let snap = state.rt.metrics.snapshot();
    Json(json!({
        "status": "ok",
        "service": "telemetry-ingestion",
        "vertical": 1,
        "accepted": snap.accepted,
        "last_accepted_unix": snap.last_accepted_unix,
    }))
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "status": "ready",
        "runtime_mode": state.rt.config.runtime_mode,
        "metrics": state.rt.metrics.snapshot(),
    }))
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.rt.metrics.snapshot())
}

#[derive(Debug, Deserialize)]
struct UpsertTenantBody {
    tenant_id: String,
    github_webhook_secret: Option<String>,
    gitlab_webhook_secret: Option<String>,
    jira_webhook_secret: Option<String>,
    linear_webhook_secret: Option<String>,
    slack_signing_secret: Option<String>,
    teams_webhook_secret: Option<String>,
    zendesk_webhook_secret: Option<String>,
    default_group_ids: Option<Vec<String>>,
}

async fn upsert_tenant(
    State(state): State<AppState>,
    Json(body): Json<UpsertTenantBody>,
) -> Result<impl IntoResponse, ApiError> {
    let cfg = TenantConfig {
        tenant_id: body.tenant_id.clone(),
        github_webhook_secret: body.github_webhook_secret,
        gitlab_webhook_secret: body.gitlab_webhook_secret,
        jira_webhook_secret: body.jira_webhook_secret,
        linear_webhook_secret: body.linear_webhook_secret,
        slack_signing_secret: body.slack_signing_secret,
        teams_webhook_secret: body.teams_webhook_secret,
        zendesk_webhook_secret: body.zendesk_webhook_secret,
        default_group_ids: body.default_group_ids.unwrap_or_else(|| vec!["grp_default".into()]),
    };
    state.rt.tenants.upsert(cfg).await.map_err(ApiError::from)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "tenant_id": body.tenant_id, "status": "upserted" })),
    ))
}

async fn ingest_webhook(
    State(state): State<AppState>,
    Path((tenant_id, provider)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let provider = SourceProvider::from_str_name_lower(&provider).ok_or_else(|| {
        ApiError::bad_request(format!("unknown provider: {provider}"))
    })?;

    // Slack URL verification
    if provider == SourceProvider::Slack {
        if let Ok(v) = serde_json::from_slice::<Value>(&body) {
            if v.get("type").and_then(|t| t.as_str()) == Some("url_verification") {
                let challenge = v.get("challenge").cloned().unwrap_or(Value::Null);
                return Ok(Json(json!({ "challenge": challenge })).into_response());
            }
        }
    }

    let req = IngestRequest {
        tenant_id,
        provider,
        body: body.to_vec(),
        headers: extract_headers(&headers),
        is_backfill: headers
            .get("x-backfill")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    };

    match state.rt.pipeline.ingest(req).await {
        Ok(outcome) => {
            let status = match outcome.status {
                telemetry_core::model::IngestStatus::Accepted => StatusCode::OK,
                telemetry_core::model::IngestStatus::Duplicate => StatusCode::OK,
                telemetry_core::model::IngestStatus::DeadLettered => StatusCode::ACCEPTED,
            };
            Ok((
                status,
                Json(json!({
                    "event_id": outcome.event_id,
                    "status": outcome.status,
                    "latency_ms": outcome.latency_ms,
                })),
            )
                .into_response())
        }
        Err(CoreError::RateLimited { retry_after_secs }) => Ok((
            StatusCode::TOO_MANY_REQUESTS,
            [
                (
                    axum::http::header::RETRY_AFTER,
                    retry_after_secs.to_string(),
                ),
            ],
            Json(json!({ "error": "rate_limited", "retry_after_secs": retry_after_secs })),
        )
            .into_response()),
        Err(CoreError::Auth(msg)) => Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized", "message": msg })),
        )
            .into_response()),
        Err(CoreError::NotFound(msg)) => Ok((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "not_found", "message": msg })),
        )
            .into_response()),
        Err(CoreError::Validation(msg)) if msg == "slack_url_verification" => {
            Ok(StatusCode::BAD_REQUEST.into_response())
        }
        Err(e) => {
            error!(error = %e, "ingest failed");
            Err(ApiError::from(e))
        }
    }
}

fn extract_headers(headers: &HeaderMap) -> IngestHeaders {
    let get = |name: &str| -> Option<String> {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    };
    IngestHeaders {
        signature_256: get("x-hub-signature-256"),
        signature: get("x-hub-signature"),
        gitlab_token: get("x-gitlab-token"),
        shared_secret: get("x-webhook-secret")
            .or_else(|| get("x-linear-signature"))
            .or_else(|| get("authorization")),
        linear_signature: get("linear-signature").or_else(|| get("x-linear-signature")),
        slack_signature: get("x-slack-signature"),
        slack_timestamp: get("x-slack-request-timestamp"),
        delivery_id: get("x-github-delivery")
            .or_else(|| get("x-gitlab-event-uuid"))
            .or_else(|| get("x-request-id"))
            .or_else(|| get("x-delivery-id")),
        event_name: get("x-github-event")
            .or_else(|| get("x-gitlab-event"))
            .or_else(|| get("x-event-type")),
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        let status = if e.is_client_error() {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self {
            status,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": self.message })),
        )
            .into_response()
    }
}

// ─── ACL-filtered query surface (also on telemetry-query binary) ─────────────

#[derive(Debug, Deserialize)]
struct QueryParams {
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
    axum::extract::Query(params): axum::extract::Query<QueryParams>,
) -> Result<impl IntoResponse, ApiError> {
    let groups = state
        .rt
        .acl
        .get_user_groups(&tenant_id, &params.user_id)
        .await
        .map_err(ApiError::from)?;
    let ctx = QueryContext {
        tenant_id: tenant_id.clone(),
        global_user_id: params.user_id,
        group_ids: groups,
    };
    let filter = EventQuery {
        tenant_id,
        categories: params
            .category
            .as_deref()
            .and_then(parse_category)
            .into_iter()
            .collect(),
        providers: params
            .provider
            .as_deref()
            .and_then(SourceProvider::from_str_name_lower)
            .into_iter()
            .collect(),
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
    Ok(Json(json!({ "count": events.len(), "events": events })))
}

#[derive(Debug, Deserialize)]
struct ResourceStateParams {
    user_id: String,
    resource_id: String,
}

async fn resource_state(
    State(state): State<AppState>,
    Path(tenant_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<ResourceStateParams>,
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
    let state_label = latest.as_ref().map(|e| derive_pr_state(&e.event_type));
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
