//! HTTP reverse proxy: inject credentials for allowlisted tools/hosts.

use crate::redact::redact_secrets;
use crate::registry::ToolRegistry;
use crate::secrets::SecretsStore;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use std::sync::Arc;
use tracing::{info, warn};

/// Shared proxy state.
#[derive(Clone)]
pub struct ProxyState {
    pub registry: Arc<ToolRegistry>,
    pub secrets: Arc<SecretsStore>,
    pub http: reqwest::Client,
    /// When true, scan response body for known secret values and redact.
    pub redact_responses: bool,
}

impl ProxyState {
    pub fn new(registry: ToolRegistry, secrets: SecretsStore) -> Self {
        Self {
            registry: Arc::new(registry),
            secrets: Arc::new(secrets),
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("reqwest client"),
            redact_responses: true,
        }
    }
}

/// Build the axum router for the egress proxy.
pub fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        // Absolute-URL form: /https://api.github.com/user  or  /http://...
        .route("/{*path}", any(proxy_handler))
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Header clients must send so the proxy knows which tool/secret to use.
pub const TOOL_HEADER: &str = "x-ai-manager-tool";

async fn proxy_handler(State(state): State<ProxyState>, req: Request) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();
    let uri = req.uri().clone();

    let tool = match headers
        .get(TOOL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(t) => t,
        None => {
            warn!(path = %uri.path(), "missing X-AI-Manager-Tool header");
            return (
                StatusCode::BAD_REQUEST,
                "missing X-AI-Manager-Tool header",
            )
                .into_response();
        }
    };

    let target = match resolve_target_url(&uri) {
        Ok(u) => u,
        Err(msg) => {
            warn!(path = %uri.path(), %msg, "bad target URL");
            return (StatusCode::BAD_REQUEST, msg).into_response();
        }
    };

    let host = match target.host_str() {
        Some(h) => h.to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "target URL missing host").into_response();
        }
    };

    // Fail closed: unknown tool or host not allowlisted.
    let tool_cfg = match state.registry.get(&tool) {
        Some(c) => c,
        None => {
            warn!(%tool, %host, "unknown tool");
            return audit_deny(&tool, &host, StatusCode::FORBIDDEN, "unknown tool");
        }
    };

    if !state.registry.host_allowed(&tool, &host) {
        warn!(%tool, %host, "host not allowlisted for tool");
        return audit_deny(
            &tool,
            &host,
            StatusCode::FORBIDDEN,
            "host not allowlisted for tool",
        );
    }

    let secret_value = match state.secrets.get(&tool_cfg.secret) {
        Some(v) => v,
        None => {
            warn!(%tool, secret = %tool_cfg.secret, "secret not found (fail-closed)");
            return audit_deny(
                &tool,
                &host,
                StatusCode::SERVICE_UNAVAILABLE,
                "secret not available",
            );
        }
    };

    // Build upstream request: strip hop-by-hop + client auth; inject secret.
    let body_bytes = match axum::body::to_bytes(req.into_body(), 16 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("failed to read body: {e}"),
            )
                .into_response();
        }
    };

    let mut upstream = state
        .http
        .request(method_to_reqwest(&method), target.as_str());

    // Forward safe headers (skip host, auth, connection, content-length, tool header).
    let mut forwarded = HeaderMap::new();
    for (name, value) in headers.iter() {
        let n = name.as_str();
        if is_hop_by_hop(n) || n == TOOL_HEADER || n == "authorization" || n == "host" {
            continue;
        }
        forwarded.insert(name.clone(), value.clone());
    }
    // Inject credential — never log secret_value.
    let inject = format!("{}{}", tool_cfg.prefix, secret_value);
    if let (Ok(hn), Ok(hv)) = (
        HeaderName::from_bytes(tool_cfg.header.as_bytes()),
        HeaderValue::from_str(&inject),
    ) {
        forwarded.insert(hn, hv);
    } else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid header config for tool",
        )
            .into_response();
    }

    for (k, v) in forwarded.iter() {
        upstream = upstream.header(k.as_str(), v.as_bytes());
    }
    if !body_bytes.is_empty() {
        upstream = upstream.body(body_bytes.to_vec());
    }

    let upstream_resp = match upstream.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!(%tool, %host, error = %e, "upstream request failed");
            info!(
                tool = %tool,
                host = %host,
                status = 502,
                "egress audit"
            );
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream error: {e}"),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
    let mut resp_headers = HeaderMap::new();
    for (name, value) in upstream_resp.headers().iter() {
        let n = name.as_str();
        if is_hop_by_hop(n) || n == "transfer-encoding" {
            continue;
        }
        if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
            resp_headers.insert(name.clone(), v);
        }
    }

    let resp_body = match upstream_resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("upstream body error: {e}"),
            )
                .into_response();
        }
    };

    let final_body = if state.redact_responses {
        match std::str::from_utf8(&resp_body) {
            Ok(text) => {
                let redacted = redact_secrets(text, state.secrets.values());
                Body::from(redacted)
            }
            Err(_) => Body::from(resp_body),
        }
    } else {
        Body::from(resp_body)
    };

    // Structured audit — never log secret values.
    info!(
        tool = %tool,
        host = %host,
        status = status.as_u16(),
        "egress audit"
    );

    let mut response = Response::new(final_body);
    *response.status_mut() = status;
    *response.headers_mut() = resp_headers;
    // Ensure content-type if missing for redacted text paths is fine as-is.
    let _ = header::CONTENT_TYPE;
    response
}

fn audit_deny(tool: &str, host: &str, status: StatusCode, msg: &'static str) -> Response {
    info!(
        tool = %tool,
        host = %host,
        status = status.as_u16(),
        reason = msg,
        "egress audit"
    );
    (status, msg).into_response()
}

/// Parse target from path:
/// - `/https://api.github.com/user` → https://api.github.com/user
/// - `/http://example.com/x` → http://example.com/x
/// - `/proxy/api.github.com/user` → https://api.github.com/user
/// - `/proxy/https://api.github.com/user` → https://api.github.com/user
fn resolve_target_url(uri: &Uri) -> Result<url::Url, &'static str> {
    let path = uri.path();
    // Drop leading slash.
    let rest = path.strip_prefix('/').unwrap_or(path);
    if rest.is_empty() || rest == "healthz" {
        return Err("missing target URL in path");
    }

    let candidate = if let Some(after) = rest.strip_prefix("proxy/") {
        if after.starts_with("http://") || after.starts_with("https://") {
            after.to_string()
        } else {
            // host/path → assume https
            format!("https://{after}")
        }
    } else if rest.starts_with("http://") || rest.starts_with("https://") {
        rest.to_string()
    } else if rest.starts_with("http:/") || rest.starts_with("https:/") {
        // axum may collapse // → / in path: https:/api.github.com/user
        fix_collapsed_scheme(rest)
    } else {
        return Err("path must be /https://host/... or /proxy/host/...");
    };

    // Preserve query string from original request.
    let with_query = if let Some(q) = uri.query() {
        if candidate.contains('?') {
            candidate
        } else {
            format!("{candidate}?{q}")
        }
    } else {
        candidate
    };

    url::Url::parse(&with_query).map_err(|_| "invalid target URL")
}

fn fix_collapsed_scheme(rest: &str) -> String {
    // https:/host/path → https://host/path
    if let Some(after) = rest.strip_prefix("https:/") {
        if after.starts_with('/') {
            format!("https:/{after}") // already https://
        } else {
            format!("https://{after}")
        }
    } else if let Some(after) = rest.strip_prefix("http:/") {
        if after.starts_with('/') {
            format!("http:/{after}")
        } else {
            format!("http://{after}")
        }
    } else {
        rest.to_string()
    }
}

fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
    )
}

fn method_to_reqwest(m: &Method) -> reqwest::Method {
    reqwest::Method::from_bytes(m.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolRegistry;
    use crate::secrets::SecretsStore;
    use axum::body::Body;
    use axum::http::Request;
    use std::collections::HashMap;
    use tower::ServiceExt;

    fn sample_registry() -> ToolRegistry {
        ToolRegistry::from_yaml_str(
            r#"
tools:
  github_api:
    hosts: ["api.github.com", "127.0.0.1"]
    secret: GITHUB_TOKEN
    header: Authorization
    prefix: "Bearer "
"#,
        )
        .unwrap()
    }

    fn sample_secrets() -> SecretsStore {
        let mut m = HashMap::new();
        m.insert("GITHUB_TOKEN".into(), "ghp_injected_secret".into());
        SecretsStore::new(m)
    }

    /// TC-S02: proxy injects Authorization header for allowlisted host.
    #[tokio::test]
    async fn tc_s02_injects_credential() {
        // Mock upstream that echoes Authorization.
        let upstream = Router::new().route(
            "/user",
            get(|headers: HeaderMap| async move {
                let auth = headers
                    .get(header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                (StatusCode::OK, auth)
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, upstream).await.unwrap();
        });

        // Registry must allow 127.0.0.1 for this test.
        let reg = ToolRegistry::from_yaml_str(&format!(
            r#"
tools:
  github_api:
    hosts: ["127.0.0.1"]
    secret: GITHUB_TOKEN
    header: Authorization
    prefix: "Bearer "
"#
        ))
        .unwrap();
        let state = ProxyState::new(reg, sample_secrets());
        let app = build_router(state);

        let path = format!("/http://127.0.0.1:{}/user", addr.port());
        let req = Request::builder()
            .method("GET")
            .uri(&path)
            .header(TOOL_HEADER, "github_api")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        // Redaction may replace the secret in the response body — either full inject
        // was applied upstream and redacted, or we see Bearer prefix + redacted.
        assert!(
            text == "Bearer ghp_injected_secret" || text == "Bearer [REDACTED]",
            "unexpected body: {text}"
        );
    }

    /// TC-S03: unknown / non-allowlisted host denied with 403.
    #[tokio::test]
    async fn tc_s03_unknown_host_denied() {
        let state = ProxyState::new(sample_registry(), sample_secrets());
        let app = build_router(state);

        let req = Request::builder()
            .method("GET")
            .uri("/https://evil.example.com/steal")
            .header(TOOL_HEADER, "github_api")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unknown_tool_denied() {
        let state = ProxyState::new(sample_registry(), sample_secrets());
        let app = build_router(state);

        let req = Request::builder()
            .method("GET")
            .uri("/https://api.github.com/user")
            .header(TOOL_HEADER, "not_a_tool")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_tool_header() {
        let state = ProxyState::new(sample_registry(), sample_secrets());
        let app = build_router(state);

        let req = Request::builder()
            .method("GET")
            .uri("/https://api.github.com/user")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolve_url_forms() {
        let u: Uri = "/https://api.github.com/user".parse().unwrap();
        // Note: path may collapse // 
        let resolved = resolve_target_url(&u);
        // Depending on URI parse, path might be /https://... or /https:/...
        assert!(resolved.is_ok(), "{resolved:?}");
        let url = resolved.unwrap();
        assert_eq!(url.host_str(), Some("api.github.com"));

        let u: Uri = "/proxy/api.github.com/repos/x".parse().unwrap();
        let url = resolve_target_url(&u).unwrap();
        assert_eq!(url.as_str(), "https://api.github.com/repos/x");
    }
}
