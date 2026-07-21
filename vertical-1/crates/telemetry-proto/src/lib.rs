//! Compile-time generated Protocol Buffer types for enterprise telemetry.
//!
//! Schemas are defined in `proto/enterprise/telemetry/v1/events.proto` and
//! compiled via `prost-build` at crate build time.

// Generated module for package enterprise.telemetry.v1
pub mod enterprise {
    pub mod telemetry {
        pub mod v1 {
            include!(concat!(env!("OUT_DIR"), "/enterprise.telemetry.v1.rs"));
        }
    }
}

pub use enterprise::telemetry::v1::*;

/// Convenience re-exports for consumers.
pub mod prelude {
    pub use super::{
        AclContext, AclRevocationEvent, CanonicalEvent, EventCategory, SourceProvider,
        StreamEnvelope, UserIdentity,
    };
}

impl SourceProvider {
    pub fn as_str_name_lower(&self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Jira => "jira",
            Self::Linear => "linear",
            Self::Slack => "slack",
            Self::Teams => "teams",
            Self::Zendesk => "zendesk",
        }
    }

    pub fn from_str_name_lower(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "github" => Some(Self::Github),
            "gitlab" => Some(Self::Gitlab),
            "jira" => Some(Self::Jira),
            "linear" => Some(Self::Linear),
            "slack" => Some(Self::Slack),
            "teams" | "microsoft_teams" | "msteams" => Some(Self::Teams),
            "zendesk" => Some(Self::Zendesk),
            _ => None,
        }
    }

    /// ClickHouse Enum8 label used in analytical storage.
    pub fn clickhouse_label(&self) -> &'static str {
        match self {
            Self::Unspecified => "GITHUB",
            Self::Github => "GITHUB",
            Self::Gitlab => "GITLAB",
            Self::Jira => "JIRA",
            Self::Linear => "LINEAR",
            Self::Slack => "SLACK",
            Self::Teams => "TEAMS",
            Self::Zendesk => "ZENDESK",
        }
    }
}

impl EventCategory {
    pub fn as_str_name_lower(&self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Code => "code",
            Self::WorkItem => "work_item",
            Self::Communication => "communication",
            Self::Identity => "identity",
        }
    }

    pub fn clickhouse_label(&self) -> &'static str {
        match self {
            Self::Unspecified => "CODE",
            Self::Code => "CODE",
            Self::WorkItem => "WORK_ITEM",
            Self::Communication => "COMMUNICATION",
            Self::Identity => "IDENTITY",
        }
    }
}

// prost generates i32 enums; ensure Copy/Clone/PartialEq are available via prost.
