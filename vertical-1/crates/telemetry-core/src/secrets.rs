//! File-backed secrets loading for Vertical 1 (dev + simple deploy overlays).
//!
//! Used for:
//! - Optional tenant webhook secret overlays (`WEBHOOK_SECRET_<tenant_id>`)
//! - Shared secret name→value map consumed by egress tooling
//!
//! **Inbound** webhook HMAC still runs in-process. This module does not route
//! inbound traffic through the egress proxy.
//!
//! When `EGRESS_ENFORCE=true`, callers must not fall back to process env for
//! long-lived API tokens — use the egress proxy instead.

use crate::error::{CoreError, CoreResult};
use crate::model::TenantConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// In-memory secret name → value map. Never log values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsMap {
    #[serde(flatten)]
    map: HashMap<String, String>,
}

impl SecretsMap {
    pub fn new(map: HashMap<String, String>) -> Self {
        Self { map }
    }

    pub fn empty() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.map.get(name).map(|s| s.as_str())
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.map.insert(name.into(), value.into());
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(|k| k.as_str())
    }

    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.map
    }
}

/// Load secrets from a JSON file: `{ "NAME": "value", ... }`.
pub fn load_secrets_file(path: impl AsRef<Path>) -> CoreResult<SecretsMap> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|e| {
        CoreError::Internal(format!(
            "failed to read secrets file {}: {e}",
            path.display()
        ))
    })?;
    let map: HashMap<String, String> = serde_json::from_str(&raw)
        .map_err(|e| CoreError::Internal(format!("failed to parse secrets JSON: {e}")))?;
    Ok(SecretsMap::new(map))
}

/// Load from `SECRETS_FILE` env if set; otherwise `None` (not an error).
pub fn load_secrets_from_env() -> CoreResult<Option<SecretsMap>> {
    match std::env::var("SECRETS_FILE") {
        Ok(path) if !path.is_empty() => Ok(Some(load_secrets_file(path)?)),
        _ => Ok(None),
    }
}

/// Helper for overlaying file secrets onto tenant webhook configuration.
///
/// Convention (document + implement):
/// - `WEBHOOK_SECRET_<tenant_id>` → GitHub webhook secret for that tenant
/// - `WEBHOOK_SECRET_<tenant_id>_GITHUB` → GitHub
/// - `WEBHOOK_SECRET_<tenant_id>_GITLAB` → GitLab
/// - `WEBHOOK_SECRET_<tenant_id>_SLACK` → Slack signing secret
/// - similarly `_JIRA`, `_LINEAR`, `_TEAMS`, `_ZENDESK`
///
/// Short form `WEBHOOK_SECRET_<tenant>` fills GitHub only (most common smoke path).
#[derive(Debug, Clone)]
pub struct TenantSecrets {
    pub map: SecretsMap,
}

impl TenantSecrets {
    pub fn from_file(path: impl AsRef<Path>) -> CoreResult<Self> {
        Ok(Self {
            map: load_secrets_file(path)?,
        })
    }

    pub fn from_map(map: SecretsMap) -> Self {
        Self { map }
    }

    /// Build or overlay a `TenantConfig` from keys matching `tenant_id`.
    pub fn apply_to_tenant(&self, mut config: TenantConfig) -> TenantConfig {
        let tid = &config.tenant_id;
        let prefix = format!("WEBHOOK_SECRET_{tid}");

        if let Some(v) = self.map.get(&prefix) {
            config.github_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_GITHUB")) {
            config.github_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_GITLAB")) {
            config.gitlab_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_JIRA")) {
            config.jira_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_LINEAR")) {
            config.linear_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_SLACK")) {
            config.slack_signing_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_TEAMS")) {
            config.teams_webhook_secret = Some(v.to_string());
        }
        if let Some(v) = self.map.get(&format!("{prefix}_ZENDESK")) {
            config.zendesk_webhook_secret = Some(v.to_string());
        }
        config
    }

    /// Construct a minimal tenant config from file keys alone.
    pub fn tenant_config(&self, tenant_id: &str, default_group_ids: Vec<String>) -> TenantConfig {
        let base = TenantConfig {
            tenant_id: tenant_id.to_string(),
            github_webhook_secret: None,
            gitlab_webhook_secret: None,
            jira_webhook_secret: None,
            linear_webhook_secret: None,
            slack_signing_secret: None,
            teams_webhook_secret: None,
            zendesk_webhook_secret: None,
            default_group_ids,
        };
        self.apply_to_tenant(base)
    }
}

/// Minimal tool registry parse (YAML) for client-side validation / tests.
/// Full enforcement lives in `vertical-security` egress-proxy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolEntry {
    pub hosts: Vec<String>,
    pub secret: String,
    #[serde(default = "default_auth_header")]
    pub header: String,
    #[serde(default)]
    pub prefix: String,
}

fn default_auth_header() -> String {
    "Authorization".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ToolRegistryFile {
    #[serde(default)]
    tools: HashMap<String, ToolEntry>,
}

/// Parse tool registry YAML (same schema as vertical-security).
pub fn parse_tool_registry_yaml(yaml: &str) -> CoreResult<HashMap<String, ToolEntry>> {
    let file: ToolRegistryFile = serde_yaml::from_str(yaml)
        .map_err(|e| CoreError::Validation(format!("tool registry YAML: {e}")))?;
    Ok(file.tools)
}

pub fn load_tool_registry(path: impl AsRef<Path>) -> CoreResult<HashMap<String, ToolEntry>> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).map_err(|e| {
        CoreError::Internal(format!(
            "failed to read tool registry {}: {e}",
            path.display()
        ))
    })?;
    parse_tool_registry_yaml(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_secrets_file_ok() {
        let f = tempfile_or_std();
        let path = f.path().to_path_buf();
        std::fs::write(
            &path,
            r#"{"GITHUB_TOKEN":"ghp_x","WEBHOOK_SECRET_ten_demo":"whsec_demo"}"#,
        )
        .unwrap();
        let map = load_secrets_file(&path).unwrap();
        assert_eq!(map.get("GITHUB_TOKEN"), Some("ghp_x"));
        assert_eq!(map.get("WEBHOOK_SECRET_ten_demo"), Some("whsec_demo"));
        let _ = f;
    }

    #[test]
    fn tenant_secrets_overlay() {
        let mut map = SecretsMap::empty();
        map.insert("WEBHOOK_SECRET_acme", "whsec_acme");
        map.insert("WEBHOOK_SECRET_acme_SLACK", "slack_sig");
        let ts = TenantSecrets::from_map(map);
        let cfg = ts.tenant_config("acme", vec!["grp_eng".into()]);
        assert_eq!(cfg.github_webhook_secret.as_deref(), Some("whsec_acme"));
        assert_eq!(cfg.slack_signing_secret.as_deref(), Some("slack_sig"));
        assert_eq!(cfg.default_group_ids, vec!["grp_eng".to_string()]);
    }

    #[test]
    fn parse_registry_yaml() {
        let yaml = r#"
tools:
  github_api:
    hosts: ["api.github.com"]
    secret: GITHUB_TOKEN
    header: Authorization
    prefix: "Bearer "
  slack_api:
    hosts: ["slack.com", "www.slack.com"]
    secret: SLACK_BOT_TOKEN
    header: Authorization
    prefix: "Bearer "
"#;
        let tools = parse_tool_registry_yaml(yaml).unwrap();
        assert_eq!(tools.len(), 2);
        let gh = tools.get("github_api").unwrap();
        assert_eq!(gh.secret, "GITHUB_TOKEN");
        assert_eq!(gh.hosts, vec!["api.github.com"]);
        assert_eq!(gh.prefix, "Bearer ");
    }

    /// Local helper: prefer tempfile if present as dev-dep, else write under std::env::temp_dir.
    struct Tmp {
        path: std::path::PathBuf,
    }
    impl Tmp {
        fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
    fn tempfile_or_std() -> Tmp {
        let path = std::env::temp_dir().join(format!(
            "v1-secrets-test-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{}").unwrap();
        Tmp { path }
    }
}
