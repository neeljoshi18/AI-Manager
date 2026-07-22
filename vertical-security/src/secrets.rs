//! File-backed secrets store for local/dev (and simple deploy overlays).
//!
//! Map: secret name → secret value, loaded from JSON:
//! ```json
//! { "GITHUB_TOKEN": "ghp_...", "SLACK_BOT_TOKEN": "xoxb_..." }
//! ```
//!
//! Path: `SECRETS_FILE` env or default `secrets/dev_secrets.json`.
//! Never log secret values.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecretsError {
    #[error("secrets file not found: {0}")]
    NotFound(String),
    #[error("failed to read secrets file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse secrets JSON: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("secret not found: {0}")]
    Missing(String),
}

/// In-memory name → value map. Values must never appear in logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsStore {
    #[serde(flatten)]
    map: HashMap<String, String>,
}

impl SecretsStore {
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

    pub fn require(&self, name: &str) -> Result<&str, SecretsError> {
        self.get(name)
            .ok_or_else(|| SecretsError::Missing(name.to_string()))
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.map.insert(name.into(), value.into());
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// All secret **names** (never values) — for redaction lists / TC-S01 style checks.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(|k| k.as_str())
    }

    /// All secret values — used only for response body redaction, never logged.
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.map.values().map(|v| v.as_str())
    }

    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.map
    }
}

/// Load secrets from a JSON file (name → value object).
pub fn load_secrets_file(path: impl AsRef<Path>) -> Result<SecretsStore, SecretsError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(SecretsError::NotFound(path.display().to_string()));
    }
    let raw = std::fs::read_to_string(path)?;
    let map: HashMap<String, String> = serde_json::from_str(&raw)?;
    Ok(SecretsStore::new(map))
}

/// Resolve path: `SECRETS_FILE` env, else `default_path`.
pub fn load_secrets_from_env_or(default_path: impl AsRef<Path>) -> Result<SecretsStore, SecretsError> {
    let path = std::env::var("SECRETS_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| default_path.as_ref().to_path_buf());
    load_secrets_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// TC-S01: secrets module loads from file.
    #[test]
    fn tc_s01_load_secrets_from_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(
            f,
            r#"{{"GITHUB_TOKEN":"ghp_test_token_abc","SLACK_BOT_TOKEN":"xoxb-test"}}"#
        )
        .unwrap();
        f.flush().unwrap();

        let store = load_secrets_file(f.path()).expect("load");
        assert_eq!(store.get("GITHUB_TOKEN"), Some("ghp_test_token_abc"));
        assert_eq!(store.get("SLACK_BOT_TOKEN"), Some("xoxb-test"));
        assert!(store.get("MISSING").is_none());
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn missing_file_errors() {
        let err = load_secrets_file("/nonexistent/path/secrets.json").unwrap_err();
        assert!(matches!(err, SecretsError::NotFound(_)));
    }

    #[test]
    fn require_missing() {
        let store = SecretsStore::empty();
        assert!(store.require("X").is_err());
    }
}
