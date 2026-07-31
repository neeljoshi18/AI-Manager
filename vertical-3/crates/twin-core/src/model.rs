use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Deterministic confidence tiers (TAS §4.3 / §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceTier {
    High,
    Medium,
    Blocker,
}

impl ConfidenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Blocker => "blocker",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "high" => Some(Self::High),
            "medium" => Some(Self::Medium),
            "blocker" => Some(Self::Blocker),
            _ => None,
        }
    }
}

impl std::fmt::Display for ConfidenceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwinKind {
    Person,
    Team,
}

impl TwinKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Team => "team",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "person" => Some(Self::Person),
            "team" => Some(Self::Team),
            _ => None,
        }
    }
}

/// Draft delivery status (TAS §4.4 / data model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    Shadow,
    Pending,
    Edited,
    Vetoed,
    PublishQueued,
    Published,
    Expired,
    ForceHuman,
    PublishFailed,
}

impl DraftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shadow => "shadow",
            Self::Pending => "pending",
            Self::Edited => "edited",
            Self::Vetoed => "vetoed",
            Self::PublishQueued => "publish_queued",
            Self::Published => "published",
            Self::Expired => "expired",
            Self::ForceHuman => "force_human",
            Self::PublishFailed => "publish_failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "shadow" => Some(Self::Shadow),
            "pending" => Some(Self::Pending),
            "edited" => Some(Self::Edited),
            "vetoed" => Some(Self::Vetoed),
            "publish_queued" => Some(Self::PublishQueued),
            "published" => Some(Self::Published),
            "expired" => Some(Self::Expired),
            "force_human" => Some(Self::ForceHuman),
            "publish_failed" => Some(Self::PublishFailed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Twin {
    pub tenant_id: String,
    pub twin_id: String,
    pub twin_kind: TwinKind,
    pub subject_id: String,
    pub display_name: String,
    pub timezone: String,
    pub channel_id: String,
    pub shadow_until: Option<DateTime<Utc>>,
    pub high_auto_publish: bool,
    pub enabled: bool,
    pub config_json: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Twin {
    pub fn is_in_shadow(&self, now: DateTime<Utc>) -> bool {
        self.shadow_until.map(|u| now < u).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LedgerPeriod {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LedgerItem {
    pub kind: String,
    pub resource_id: String,
    pub node_id: String,
    pub summary: String,
    pub confidence: ConfidenceTier,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockerItem {
    pub node_id: String,
    pub summary: String,
    pub confidence: ConfidenceTier,
    pub evidence_refs: Vec<String>,
}

/// StatusLedger JSON contract (TAS §6.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusLedger {
    pub tenant_id: String,
    pub person_id: String,
    pub period: LedgerPeriod,
    pub confidence_rollup: ConfidenceTier,
    pub items: Vec<LedgerItem>,
    pub open_blockers: Vec<BlockerItem>,
    pub graph_as_of: DateTime<Utc>,
    pub compiled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LedgerSnapshot {
    pub tenant_id: String,
    pub ledger_id: String,
    pub twin_id: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub confidence_rollup: ConfidenceTier,
    pub ledger: StatusLedger,
    pub graph_as_of: DateTime<Utc>,
    pub compiled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftDelivery {
    pub tenant_id: String,
    pub draft_id: String,
    pub ledger_id: String,
    pub twin_id: String,
    pub status: DraftStatus,
    pub slack_dm_channel: String,
    pub slack_dm_ts: String,
    pub draft_text: String,
    pub edited_text: Option<String>,
    pub veto_deadline: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DraftDelivery {
    /// Body that will be published (edited if present, else draft).
    pub fn publish_body(&self) -> &str {
        self.edited_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(self.draft_text.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublishRecord {
    pub tenant_id: String,
    pub publish_id: String,
    pub ledger_id: String,
    pub draft_id: String,
    pub channel_id: String,
    pub slack_ts: String,
    pub body_hash: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompileRun {
    pub tenant_id: String,
    pub run_id: String,
    pub twin_id: String,
    pub status: String,
    pub error_text: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlackUserMap {
    pub tenant_id: String,
    pub global_user_id: String,
    pub slack_user_id: String,
    pub slack_team_id: String,
}

/// Request body for twin upsert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTwinRequest {
    pub twin_kind: TwinKind,
    pub subject_id: String,
    pub display_name: Option<String>,
    pub timezone: Option<String>,
    pub channel_id: Option<String>,
    pub shadow_until: Option<DateTime<Utc>>,
    pub high_auto_publish: Option<bool>,
    pub enabled: Option<bool>,
    pub config_json: Option<JsonValue>,
    /// Optional override; default person twin id from subject.
    pub twin_id: Option<String>,
    pub slack_user_id: Option<String>,
}

/// Graph fixture inputs used by compiler (ACL-already-filtered view).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphView {
    pub nodes: Vec<GraphNodeView>,
    pub edges: Vec<GraphEdgeView>,
    pub states: Vec<EntityStateView>,
    pub blockers: Vec<GraphEdgeView>,
    pub graph_as_of: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNodeView {
    pub node_id: String,
    pub node_type: String,
    pub display_name: String,
    pub resource_id: String,
    #[serde(default)]
    pub properties: JsonValue,
    #[serde(default)]
    pub is_private: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEdgeView {
    pub edge_id: String,
    pub edge_type: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub event_id: String,
    #[serde(default)]
    pub properties: JsonValue,
    #[serde(default)]
    pub is_private: bool,
    /// Origin event time from V2 (optional for fixtures / older payloads).
    #[serde(default)]
    pub valid_from: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityStateView {
    pub node_id: String,
    pub state_key: String,
    pub state_value: String,
    pub event_id: String,
    pub as_of: DateTime<Utc>,
}

/// Product defaults (TAS §5).
pub const DEFAULT_SHADOW_MODE_DAYS: i64 = 10;
pub const DEFAULT_MEDIUM_VETO_WINDOW_SECS: i64 = 2 * 3600;
pub const DEFAULT_BLOCKER_VETO_WINDOW_SECS: i64 = 24 * 3600;
