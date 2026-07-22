//! Tool registry: tool name → allowed hosts + secret injection config.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry file not found: {0}")]
    NotFound(String),
    #[error("failed to read registry: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse registry YAML: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

/// Per-tool allowlist + credential injection rules.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolConfig {
    /// Hostnames this tool may call (exact match, lowercase).
    pub hosts: Vec<String>,
    /// Secret name in the secrets store (e.g. `GITHUB_TOKEN`).
    pub secret: String,
    /// Header to set (default `Authorization`).
    #[serde(default = "default_header")]
    pub header: String,
    /// Prefix before the secret value (e.g. `"Bearer "`).
    #[serde(default)]
    pub prefix: String,
}

fn default_header() -> String {
    "Authorization".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolRegistryFile {
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
}

/// Loaded tool registry.
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolConfig>,
}

impl ToolRegistry {
    pub fn new(tools: HashMap<String, ToolConfig>) -> Self {
        // Normalize hosts to lowercase.
        let tools = tools
            .into_iter()
            .map(|(k, mut v)| {
                v.hosts = v.hosts.into_iter().map(|h| h.to_ascii_lowercase()).collect();
                (k, v)
            })
            .collect();
        Self { tools }
    }

    pub fn from_yaml_str(yaml: &str) -> Result<Self, RegistryError> {
        let file: ToolRegistryFile = serde_yaml::from_str(yaml)?;
        Ok(Self::new(file.tools))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, RegistryError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(RegistryError::NotFound(path.display().to_string()));
        }
        let raw = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }

    pub fn get(&self, tool: &str) -> Option<&ToolConfig> {
        self.tools.get(tool)
    }

    pub fn require(&self, tool: &str) -> Result<&ToolConfig, RegistryError> {
        self.get(tool)
            .ok_or_else(|| RegistryError::UnknownTool(tool.to_string()))
    }

    /// True if `host` is allowlisted for `tool`.
    pub fn host_allowed(&self, tool: &str, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        // Strip optional port.
        let host_only = host.split(':').next().unwrap_or(&host);
        self.get(tool)
            .map(|c| c.hosts.iter().any(|h| h == host_only))
            .unwrap_or(false)
    }

    pub fn tools(&self) -> &HashMap<String, ToolConfig> {
        &self.tools
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
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

    #[test]
    fn parse_registry_yaml() {
        let reg = ToolRegistry::from_yaml_str(SAMPLE).unwrap();
        assert_eq!(reg.len(), 2);
        let gh = reg.get("github_api").unwrap();
        assert_eq!(gh.secret, "GITHUB_TOKEN");
        assert_eq!(gh.prefix, "Bearer ");
        assert!(reg.host_allowed("github_api", "api.github.com"));
        assert!(reg.host_allowed("github_api", "API.GitHub.com"));
        assert!(!reg.host_allowed("github_api", "evil.com"));
        assert!(!reg.host_allowed("unknown", "api.github.com"));
        assert!(reg.host_allowed("slack_api", "www.slack.com"));
    }
}
