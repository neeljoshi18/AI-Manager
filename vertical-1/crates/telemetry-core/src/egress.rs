//! Outbound HTTP client that routes through the credential egress proxy.
//!
//! When `EGRESS_PROXY_URL` is set, requests are rewritten to:
//!   `{EGRESS_PROXY_URL}/proxy/{host}{path}?{query}`
//! with header `X-AI-Manager-Tool: <tool>`.
//!
//! The client **must not** attach real `Authorization` secrets — the proxy injects them.
//!
//! Fail-closed policy (`EGRESS_ENFORCE=true`):
//! - Proxy URL required for outbound tool calls
//! - No silent fallback to env-held API tokens
//! - Errors surface to the caller instead of dialing upstream directly

use crate::error::{CoreError, CoreResult};
use reqwest::{Client, Method, Response};
use std::time::Duration;

/// Header the egress proxy expects for tool/secret lookup.
pub const TOOL_HEADER: &str = "X-AI-Manager-Tool";

/// Configuration for outbound egress.
#[derive(Debug, Clone)]
pub struct EgressConfig {
    /// Base URL of the egress proxy, e.g. `http://127.0.0.1:18090`.
    pub proxy_url: Option<String>,
    /// When true, refuse direct outbound calls if proxy is unset (fail closed).
    pub enforce: bool,
}

impl Default for EgressConfig {
    fn default() -> Self {
        Self {
            proxy_url: None,
            enforce: false,
        }
    }
}

impl EgressConfig {
    pub fn from_env() -> Self {
        let proxy_url = std::env::var("EGRESS_PROXY_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let enforce = std::env::var("EGRESS_ENFORCE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self { proxy_url, enforce }
    }

    pub fn proxy_enabled(&self) -> bool {
        self.proxy_url.is_some()
    }
}

/// reqwest wrapper that rewrites outbound URLs through the egress proxy.
#[derive(Debug, Clone)]
pub struct EgressClient {
    http: Client,
    config: EgressConfig,
}

impl EgressClient {
    pub fn new(config: EgressConfig) -> CoreResult<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| CoreError::Internal(format!("http client: {e}")))?;
        Ok(Self { http, config })
    }

    pub fn from_env() -> CoreResult<Self> {
        Self::new(EgressConfig::from_env())
    }

    pub fn config(&self) -> &EgressConfig {
        &self.config
    }

    /// Build the proxied URL for a target absolute URL.
    ///
    /// `https://api.github.com/user` + proxy `http://127.0.0.1:18090`
    /// → `http://127.0.0.1:18090/proxy/api.github.com/user`
    pub fn rewrite_url(&self, target: &str) -> CoreResult<String> {
        let proxy = match &self.config.proxy_url {
            Some(p) => p.trim_end_matches('/'),
            None => {
                if self.config.enforce {
                    return Err(CoreError::Internal(
                        "EGRESS_ENFORCE=true but EGRESS_PROXY_URL is unset (fail-closed)".into(),
                    ));
                }
                return Ok(target.to_string());
            }
        };

        let url = reqwest::Url::parse(target)
            .map_err(|e| CoreError::Validation(format!("invalid target URL: {e}")))?;
        let host = url
            .host_str()
            .ok_or_else(|| CoreError::Validation("target URL missing host".into()))?;
        let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
        let path = url.path();
        let query = url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();

        Ok(format!("{proxy}/proxy/{host}{port}{path}{query}"))
    }

    /// GET via proxy (or direct if proxy unset and not enforce).
    pub async fn get(&self, tool: &str, target_url: &str) -> CoreResult<Response> {
        self.request(Method::GET, tool, target_url, None, &[]).await
    }

    /// POST JSON via proxy.
    pub async fn post_json(
        &self,
        tool: &str,
        target_url: &str,
        body: &serde_json::Value,
    ) -> CoreResult<Response> {
        let bytes = serde_json::to_vec(body)
            .map_err(|e| CoreError::Internal(format!("json encode: {e}")))?;
        self.request(
            Method::POST,
            tool,
            target_url,
            Some(bytes),
            &[("content-type", "application/json")],
        )
        .await
    }

    /// Generic request. Does **not** attach Authorization — proxy injects it.
    pub async fn request(
        &self,
        method: Method,
        tool: &str,
        target_url: &str,
        body: Option<Vec<u8>>,
        extra_headers: &[(&str, &str)],
    ) -> CoreResult<Response> {
        if tool.is_empty() {
            return Err(CoreError::Validation("tool name required for egress".into()));
        }

        // Fail closed: never use env tokens as a silent fallback.
        if self.config.enforce && self.config.proxy_url.is_none() {
            return Err(CoreError::Internal(
                "EGRESS_ENFORCE=true: outbound API calls require EGRESS_PROXY_URL (no env secret fallback)"
                    .into(),
            ));
        }

        let url = self.rewrite_url(target_url)?;
        let mut req = self.http.request(method, &url);

        if self.config.proxy_enabled() {
            req = req.header(TOOL_HEADER, tool);
        } else {
            // Direct mode only when enforce=false — still no secret injection here.
            // Callers must not rely on this for production tool credentials.
            tracing::warn!(
                tool,
                target = target_url,
                "egress proxy unset; direct request without credential injection"
            );
        }

        for (k, v) in extra_headers {
            // Strip any attempt to smuggle Authorization past the proxy policy.
            if k.eq_ignore_ascii_case("authorization") {
                continue;
            }
            req = req.header(*k, *v);
        }
        if let Some(b) = body {
            req = req.body(b);
        }

        req.send()
            .await
            .map_err(|e| CoreError::Internal(format!("egress request failed: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_via_proxy() {
        let client = EgressClient::new(EgressConfig {
            proxy_url: Some("http://127.0.0.1:18090".into()),
            enforce: true,
        })
        .unwrap();
        let u = client
            .rewrite_url("https://api.github.com/user")
            .unwrap();
        assert_eq!(u, "http://127.0.0.1:18090/proxy/api.github.com/user");
    }

    #[test]
    fn rewrite_preserves_query() {
        let client = EgressClient::new(EgressConfig {
            proxy_url: Some("http://proxy:18090".into()),
            enforce: false,
        })
        .unwrap();
        let u = client
            .rewrite_url("https://api.github.com/search/issues?q=a")
            .unwrap();
        assert_eq!(
            u,
            "http://proxy:18090/proxy/api.github.com/search/issues?q=a"
        );
    }

    #[test]
    fn enforce_without_proxy_fails() {
        let client = EgressClient::new(EgressConfig {
            proxy_url: None,
            enforce: true,
        })
        .unwrap();
        let err = client.rewrite_url("https://api.github.com/user").unwrap_err();
        assert!(err.to_string().contains("fail-closed") || err.to_string().contains("EGRESS"));
    }

    #[test]
    fn direct_when_not_enforced() {
        let client = EgressClient::new(EgressConfig {
            proxy_url: None,
            enforce: false,
        })
        .unwrap();
        let u = client
            .rewrite_url("https://api.github.com/user")
            .unwrap();
        assert_eq!(u, "https://api.github.com/user");
    }

    #[test]
    fn config_from_env_defaults() {
        // Don't assert on ambient env; just ensure constructor works.
        let _ = EgressConfig::from_env();
    }
}
