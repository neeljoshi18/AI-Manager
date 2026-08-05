//! Shared delivery adapter interface (Slack today · Teams · future chat).
//!
//! Chat = delivery plane only. Same digest + Approve / Edit / Don't send on every adapter.
//! Tokens never live on twin-api env (ADR-012) — adapters call egress.

use async_trait::async_trait;
use twin_core::{TwinError, TwinResult};

/// Result of an outbound delivery post (DM or channel).
#[derive(Debug, Clone)]
pub struct DeliveryPostResult {
    pub channel: String,
    pub ts: String,
}

/// Which chat adapter is active for this process (or tenant later).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryAdapterKind {
    Slack,
    Teams,
    Mock,
}

impl DeliveryAdapterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Slack => "slack",
            Self::Teams => "teams",
            Self::Mock => "mock",
        }
    }

    pub fn from_env() -> Self {
        let raw = std::env::var("DELIVERY_ADAPTER")
            .or_else(|_| std::env::var("CHAT_ADAPTER"))
            .unwrap_or_default();
        match raw.trim().to_ascii_lowercase().as_str() {
            "teams" | "msteams" | "microsoft_teams" => Self::Teams,
            "mock" => Self::Mock,
            "slack" | "" => {
                // Prefer Teams only when explicitly requested; Slack remains default.
                Self::Slack
            }
            other => {
                tracing::warn!(adapter = %other, "unknown DELIVERY_ADAPTER; defaulting to slack");
                Self::Slack
            }
        }
    }
}

/// Outbound chat surface used by DeliveryService.
///
/// Implementors: Slack (live), Teams (Adaptive Cards), Mock (tests).
#[async_trait]
pub trait DeliveryClient: Send + Sync {
    fn adapter_kind(&self) -> DeliveryAdapterKind;

    async fn post_dm(&self, user_id: &str, text: &str) -> TwinResult<DeliveryPostResult>;

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<DeliveryPostResult>;

    /// Interactive draft DM: Approve · Edit · Don't send.
    /// Default: plain-text actions line (Slack-compatible). Teams overrides with Adaptive Card.
    async fn post_draft_dm(
        &self,
        user_id: &str,
        draft_id: &str,
        draft_text: &str,
        deadline_s: &str,
    ) -> TwinResult<DeliveryPostResult> {
        let _ = draft_id;
        let dm_text = format!(
            "{draft_text}\n\n*Approve* · *Edit* · *Don't send*\n\
             We'll only ping again if this status story changes.\n\
             Window until {deadline_s}."
        );
        self.post_dm(user_id, &dm_text).await
    }

    fn call_count(&self) -> u64 {
        0
    }
}

/// Build the standard action footer used when adapters fall back to plain text.
pub fn draft_dm_plain_text(draft_text: &str, deadline_s: &str) -> String {
    format!(
        "{draft_text}\n\n*Approve* · *Edit* · *Don't send*\n\
         We'll only ping again if this status story changes.\n\
         Window until {deadline_s}."
    )
}

/// Map adapter errors into TwinError::Egress consistently.
pub fn egress_err(adapter: &str, msg: impl Into<String>) -> TwinError {
    TwinError::Egress(format!("{adapter}: {}", msg.into()))
}
