//! Outbound HTTP via credential egress proxy (ADR-012).
//! Twin processes never hold SLACK_BOT_TOKEN.

use crate::error::{TwinError, TwinResult};
use reqwest::{Client, Method, Response};
use std::time::Duration;

pub const TOOL_HEADER: &str = "X-AI-Manager-Tool";
pub const SLACK_TOOL: &str = "slack_api";

#[derive(Debug, Clone)]
pub struct EgressConfig {
    pub proxy_url: Option<String>,
    pub enforce: bool,
}

impl Default for EgressConfig {
    fn default() -> Self {
        Self {
            proxy_url: None,
            enforce: true,
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
            .unwrap_or(true);
        Self { proxy_url, enforce }
    }
}

#[derive(Debug, Clone)]
pub struct EgressClient {
    http: Client,
    config: EgressConfig,
}

impl EgressClient {
    pub fn new(config: EgressConfig) -> TwinResult<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| TwinError::Internal(format!("http client: {e}")))?;
        Ok(Self { http, config })
    }

    pub fn from_env() -> TwinResult<Self> {
        Self::new(EgressConfig::from_env())
    }

    pub fn config(&self) -> &EgressConfig {
        &self.config
    }

    pub fn rewrite_url(&self, target: &str) -> TwinResult<String> {
        let proxy = match &self.config.proxy_url {
            Some(p) => p.trim_end_matches('/'),
            None => {
                if self.config.enforce {
                    return Err(TwinError::Egress(
                        "EGRESS_ENFORCE=true but EGRESS_PROXY_URL is unset (fail-closed)".into(),
                    ));
                }
                return Ok(target.to_string());
            }
        };

        let url = reqwest::Url::parse(target)
            .map_err(|e| TwinError::Validation(format!("invalid target URL: {e}")))?;
        let host = url
            .host_str()
            .ok_or_else(|| TwinError::Validation("target URL missing host".into()))?;
        let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
        let path = url.path();
        let query = url
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();

        Ok(format!("{proxy}/proxy/{host}{port}{path}{query}"))
    }

    pub async fn post_json(
        &self,
        tool: &str,
        target_url: &str,
        body: &serde_json::Value,
    ) -> TwinResult<Response> {
        let bytes = serde_json::to_vec(body)
            .map_err(|e| TwinError::Internal(format!("json encode: {e}")))?;
        self.request(
            Method::POST,
            tool,
            target_url,
            Some(bytes),
            &[("content-type", "application/json")],
        )
        .await
    }

    pub async fn request(
        &self,
        method: Method,
        tool: &str,
        target_url: &str,
        body: Option<Vec<u8>>,
        extra_headers: &[(&str, &str)],
    ) -> TwinResult<Response> {
        if tool.is_empty() {
            return Err(TwinError::Validation(
                "tool name required for egress".into(),
            ));
        }

        // Fail closed: never attach Authorization; never read SLACK_BOT_TOKEN from env.
        if std::env::var("SLACK_BOT_TOKEN").is_ok() {
            tracing::warn!(
                "SLACK_BOT_TOKEN present in process env — twin must not use it; egress inject only"
            );
        }

        let url = self.rewrite_url(target_url)?;
        let mut req = self.http.request(method, &url).header(TOOL_HEADER, tool);
        for (k, v) in extra_headers {
            req = req.header(*k, *v);
        }
        if let Some(b) = body {
            req = req.body(b);
        }
        req.send()
            .await
            .map_err(|e| TwinError::Egress(format!("request failed: {e}")))
    }
}

/// Scan process environment for forbidden Slack secrets (TC-T07).
pub fn env_has_slack_token() -> bool {
    std::env::var("SLACK_BOT_TOKEN")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
        || std::env::var("SLACK_TOKEN")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
}
