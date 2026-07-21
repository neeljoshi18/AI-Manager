//! Domain models used across ingestion, consumer, and query paths.
//!
//! These mirror the Protobuf canonical schema but are ergonomic Rust types
//! used before / after serialization.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use telemetry_proto::{EventCategory, SourceProvider};
use uuid::Uuid;

/// Fully normalized telemetry event ready for the bus and analytical lake.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CanonicalEventRecord {
    pub event_id: String,
    pub tenant_id: String,
    pub provider: SourceProvider,
    pub category: EventCategory,
    pub event_type: String,
    /// Origin-system timestamp (drives state reconstruction).
    pub event_timestamp: DateTime<Utc>,
    /// Platform ingestion timestamp.
    pub ingested_at: DateTime<Utc>,
    pub actor: ActorIdentity,
    pub acl: AclSnapshot,
    pub resource_id: String,
    pub parent_resource_id: String,
    pub attributes: JsonValue,
    pub raw_payload_s3_uri: String,
    /// Monotonic sequence used for ReplacingMergeTree / argMax state.
    pub event_sequence_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActorIdentity {
    pub global_user_id: String,
    pub provider_user_id: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AclSnapshot {
    pub tenant_id: String,
    pub allowed_group_ids: Vec<String>,
    pub is_private: bool,
    pub acl_version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AclRevocationRecord {
    pub event_id: String,
    pub tenant_id: String,
    pub global_user_id: String,
    pub provider_user_id: String,
    pub provider: SourceProvider,
    pub group_id: String,
    /// removed_from_group | added_to_group | role_changed
    pub change_type: String,
    pub acl_version: u64,
    pub timestamp: DateTime<Utc>,
}

/// Request context attached to every analytical query (Vertical 2+).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryContext {
    pub tenant_id: String,
    pub global_user_id: String,
    /// Resolved group memberships at query time (from ACL store / cache).
    pub group_ids: Vec<String>,
}

/// Filter for ACL-aware event reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventQuery {
    pub tenant_id: String,
    pub categories: Vec<EventCategory>,
    pub providers: Vec<SourceProvider>,
    pub resource_id: Option<String>,
    pub parent_resource_id: Option<String>,
    pub event_type: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: usize,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self {
            tenant_id: String::new(),
            categories: Vec::new(),
            providers: Vec::new(),
            resource_id: None,
            parent_resource_id: None,
            event_type: None,
            since: None,
            until: None,
            limit: 100,
        }
    }
}

/// Result of an ingestion attempt (returned before HTTP response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOutcome {
    pub event_id: String,
    pub status: IngestStatus,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IngestStatus {
    Accepted,
    Duplicate,
    DeadLettered,
}

/// Topics on the Redpanda / embedded bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BusTopic {
    /// All accepted raw/canonical events (default landing topic).
    EventsRaw,
    /// Real-time path isolated from historical backfill.
    EventsRealtime,
    /// Historical backfill workload with adaptive rate limiting.
    EventsBackfill,
    /// ACL revocation / membership change stream.
    EventsAcl,
}

impl BusTopic {
    pub fn as_str(&self) -> &'static str {
        match self {
            BusTopic::EventsRaw => "events.raw",
            BusTopic::EventsRealtime => "events.realtime",
            BusTopic::EventsBackfill => "events.backfill",
            BusTopic::EventsAcl => "events.acl",
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            "events.raw" => Some(BusTopic::EventsRaw),
            "events.realtime" => Some(BusTopic::EventsRealtime),
            "events.backfill" => Some(BusTopic::EventsBackfill),
            "events.acl" => Some(BusTopic::EventsAcl),
            _ => None,
        }
    }
}

/// Envelope published to the streaming bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusMessage {
    pub topic: BusTopic,
    pub partition_key: String,
    pub payload: BusPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BusPayload {
    Event(CanonicalEventRecord),
    Acl(AclRevocationRecord),
}

/// Tenant webhook configuration (secrets, mapping).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub github_webhook_secret: Option<String>,
    pub gitlab_webhook_secret: Option<String>,
    pub jira_webhook_secret: Option<String>,
    pub linear_webhook_secret: Option<String>,
    pub slack_signing_secret: Option<String>,
    pub teams_webhook_secret: Option<String>,
    pub zendesk_webhook_secret: Option<String>,
    /// Default groups attached when source payload lacks explicit ACL.
    pub default_group_ids: Vec<String>,
}

impl TenantConfig {
    pub fn secret_for(&self, provider: SourceProvider) -> Option<&str> {
        match provider {
            SourceProvider::Github => self.github_webhook_secret.as_deref(),
            SourceProvider::Gitlab => self.gitlab_webhook_secret.as_deref(),
            SourceProvider::Jira => self.jira_webhook_secret.as_deref(),
            SourceProvider::Linear => self.linear_webhook_secret.as_deref(),
            SourceProvider::Slack => self.slack_signing_secret.as_deref(),
            SourceProvider::Teams => self.teams_webhook_secret.as_deref(),
            SourceProvider::Zendesk => self.zendesk_webhook_secret.as_deref(),
            SourceProvider::Unspecified => None,
        }
    }
}

/// Generate a new event ID (UUID v4).
pub fn new_event_id() -> String {
    Uuid::new_v4().to_string()
}

/// Build a stable resource id helper.
pub fn resource_id(parts: &[&str]) -> String {
    parts.join("/")
}
