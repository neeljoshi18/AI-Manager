//! Vertical 1 wire-compatible event DTO (JSON).
//!
//! Matches `CanonicalEventRecord` / bus payload shape from vertical-1 so we can
//! project without a hard Cargo dependency on the V1 crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1CanonicalEvent {
    pub event_id: String,
    pub tenant_id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub category: String,
    pub event_type: String,
    pub event_timestamp: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub ingested_at: DateTime<Utc>,
    #[serde(default)]
    pub actor: V1Actor,
    #[serde(default)]
    pub acl: V1Acl,
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub parent_resource_id: String,
    #[serde(default)]
    pub attributes: JsonValue,
    #[serde(default)]
    pub raw_payload_s3_uri: String,
    #[serde(default)]
    pub event_sequence_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct V1Actor {
    #[serde(default)]
    pub global_user_id: String,
    #[serde(default)]
    pub provider_user_id: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct V1Acl {
    #[serde(default)]
    pub tenant_id: String,
    #[serde(default)]
    pub allowed_group_ids: Vec<String>,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub acl_version: u64,
}

/// Bus envelope used by V1 (`BusMessage`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1BusMessage {
    pub topic: String,
    #[serde(default)]
    pub partition_key: String,
    pub payload: V1BusPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum V1BusPayload {
    Event(V1CanonicalEvent),
    Acl(V1AclRevocation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1AclRevocation {
    pub event_id: String,
    pub tenant_id: String,
    #[serde(default)]
    pub global_user_id: String,
    #[serde(default)]
    pub provider_user_id: String,
    #[serde(default)]
    pub provider: String,
    pub group_id: String,
    pub change_type: String,
    #[serde(default)]
    pub acl_version: u64,
    pub timestamp: DateTime<Utc>,
}

impl V1CanonicalEvent {
    pub fn person_key(&self) -> String {
        if !self.actor.global_user_id.is_empty() {
            self.actor.global_user_id.clone()
        } else if !self.actor.provider_user_id.is_empty() {
            format!("prov:{}", self.actor.provider_user_id)
        } else {
            "unknown".into()
        }
    }
}
