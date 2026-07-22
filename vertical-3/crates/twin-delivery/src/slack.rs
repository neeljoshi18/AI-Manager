use async_trait::async_trait;
use twin_core::egress::{EgressClient, SLACK_TOOL};
use twin_core::{TwinError, TwinResult};

#[derive(Debug, Clone)]
pub struct SlackPostResult {
    pub channel: String,
    pub ts: String,
}

/// Slack Web API surface used by delivery. Always via egress in production.
#[async_trait]
pub trait SlackClient: Send + Sync {
    async fn post_dm(&self, slack_user_id: &str, text: &str) -> TwinResult<SlackPostResult>;
    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<SlackPostResult>;
    /// Number of outbound Slack calls (for tests).
    fn call_count(&self) -> u64 {
        0
    }
}

/// Real Slack via egress proxy (`slack_api` tool). Never attaches bot token.
pub struct EgressSlackClient {
    egress: EgressClient,
}

impl EgressSlackClient {
    pub fn new(egress: EgressClient) -> Self {
        Self { egress }
    }
}

#[async_trait]
impl SlackClient for EgressSlackClient {
    async fn post_dm(&self, slack_user_id: &str, text: &str) -> TwinResult<SlackPostResult> {
        // conversations.open then chat.postMessage — simplified: postMessage with user channel
        let open_body = serde_json::json!({ "users": slack_user_id });
        let open_resp = self
            .egress
            .post_json(
                SLACK_TOOL,
                "https://slack.com/api/conversations.open",
                &open_body,
            )
            .await?;
        let open_json: serde_json::Value = open_resp
            .json()
            .await
            .map_err(|e| TwinError::Egress(e.to_string()))?;
        let channel = open_json
            .pointer("/channel/id")
            .and_then(|v| v.as_str())
            .unwrap_or(slack_user_id)
            .to_string();

        self.post_channel(&channel, text).await
    }

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<SlackPostResult> {
        let body = serde_json::json!({
            "channel": channel_id,
            "text": text,
        });
        let resp = self
            .egress
            .post_json(SLACK_TOOL, "https://slack.com/api/chat.postMessage", &body)
            .await?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| TwinError::Egress(e.to_string()))?;
        if !status.is_success()
            || json.get("ok").and_then(|v| v.as_bool()) == Some(false)
        {
            return Err(TwinError::Egress(format!(
                "slack post failed: status={status} body={json}"
            )));
        }
        let ts = json
            .get("ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(SlackPostResult {
            channel: channel_id.to_string(),
            ts,
        })
    }
}
