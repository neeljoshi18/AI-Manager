//! Microsoft Teams delivery adapter (Bot Framework + Adaptive Cards).
//!
//! Vault secret: `TEAMS_BOT_TOKEN` (Bot Framework bearer / connector token) — ADR-012.
//! Public env (non-secret): `TEAMS_APP_ID`, optional `TEAMS_SERVICE_URL`, `TEAMS_TENANT_ID`.
//!
//! Proactive DM target is the Teams/AAD user id (or conversation id) mapped on the twin.
//! Same actions as Slack: Approve · Edit · Don't send.

use crate::delivery::{egress_err, DeliveryAdapterKind, DeliveryClient, DeliveryPostResult};
use crate::mock_slack::SlackCall;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use twin_core::egress::{EgressClient, TEAMS_TOOL};
use twin_core::TwinResult;

/// Real Teams via egress (`teams_bot` tool). Never attaches bot secret on twin env.
pub struct EgressTeamsClient {
    egress: EgressClient,
    /// Bot Framework app id (public).
    app_id: String,
    /// Connector base, e.g. https://smba.trafficmanager.net/amer
    service_url: String,
}

impl EgressTeamsClient {
    pub fn new(egress: EgressClient) -> Self {
        let app_id = std::env::var("TEAMS_APP_ID").unwrap_or_default();
        let service_url = std::env::var("TEAMS_SERVICE_URL")
            .unwrap_or_else(|_| "https://smba.trafficmanager.net/amer".into())
            .trim_end_matches('/')
            .to_string();
        Self {
            egress,
            app_id,
            service_url,
        }
    }

    fn activity_url(&self, conversation_id: &str) -> String {
        format!(
            "{}/v3/conversations/{}/activities",
            self.service_url,
            urlencoding_path(conversation_id)
        )
    }

    /// Adaptive Card with Approve / Edit / Don't send submit actions.
    fn adaptive_card(draft_id: &str, draft_text: &str, deadline_s: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "AdaptiveCard",
            "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
            "version": "1.4",
            "body": [
                {
                    "type": "TextBlock",
                    "text": "Status digest",
                    "weight": "Bolder",
                    "size": "Medium"
                },
                {
                    "type": "TextBlock",
                    "text": draft_text,
                    "wrap": true
                },
                {
                    "type": "TextBlock",
                    "text": format!("Window until {deadline_s}. We'll only ping again if this story changes."),
                    "wrap": true,
                    "isSubtle": true,
                    "size": "Small"
                }
            ],
            "actions": [
                {
                    "type": "Action.Submit",
                    "title": "Approve",
                    "data": {
                        "action": "publish",
                        "draft_id": draft_id
                    }
                },
                {
                    "type": "Action.Submit",
                    "title": "Edit",
                    "data": {
                        "action": "edit",
                        "draft_id": draft_id
                    }
                },
                {
                    "type": "Action.Submit",
                    "title": "Don't send",
                    "data": {
                        "action": "veto",
                        "draft_id": draft_id
                    }
                }
            ]
        })
    }

    async fn post_activity(
        &self,
        conversation_or_user: &str,
        activity: serde_json::Value,
    ) -> TwinResult<DeliveryPostResult> {
        let conv_id = if conversation_or_user.starts_with("a:")
            || conversation_or_user.starts_with("19:")
        {
            conversation_or_user.to_string()
        } else {
            let create_url = format!("{}/v3/conversations", self.service_url);
            let create_body = serde_json::json!({
                "bot": { "id": self.app_id, "name": "AI Manager" },
                "members": [{ "id": conversation_or_user }],
                "channelData": {
                    "tenant": {
                        "id": std::env::var("TEAMS_TENANT_ID").unwrap_or_default()
                    }
                },
                "isGroup": false
            });
            let resp = self
                .egress
                .post_json(TEAMS_TOOL, &create_url, &create_body)
                .await?;
            let status = resp.status();
            let json: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| egress_err("teams", e.to_string()))?;
            if !status.is_success() {
                tracing::warn!(
                    status = %status,
                    body = %json,
                    "teams create conversation failed; posting to user id as conversation"
                );
                conversation_or_user.to_string()
            } else {
                json.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or(conversation_or_user)
                    .to_string()
            }
        };

        let url = self.activity_url(&conv_id);
        let resp = self.egress.post_json(TEAMS_TOOL, &url, &activity).await?;
        let status = resp.status();
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| egress_err("teams", e.to_string()))?;
        if !status.is_success() {
            return Err(egress_err(
                "teams",
                format!("post activity failed: status={status} body={json}"),
            ));
        }
        let ts = json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(DeliveryPostResult {
            channel: conv_id,
            ts,
        })
    }
}

#[async_trait]
impl DeliveryClient for EgressTeamsClient {
    fn adapter_kind(&self) -> DeliveryAdapterKind {
        DeliveryAdapterKind::Teams
    }

    async fn post_dm(&self, user_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        let activity = serde_json::json!({
            "type": "message",
            "text": text,
            "from": { "id": self.app_id, "name": "AI Manager" },
        });
        self.post_activity(user_id, activity).await
    }

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        let activity = serde_json::json!({
            "type": "message",
            "text": text,
            "from": { "id": self.app_id, "name": "AI Manager" },
        });
        self.post_activity(channel_id, activity).await
    }

    async fn post_draft_dm(
        &self,
        user_id: &str,
        draft_id: &str,
        draft_text: &str,
        deadline_s: &str,
    ) -> TwinResult<DeliveryPostResult> {
        let card = Self::adaptive_card(draft_id, draft_text, deadline_s);
        let activity = serde_json::json!({
            "type": "message",
            "summary": "Status digest — Approve / Edit / Don't send",
            "from": { "id": self.app_id, "name": "AI Manager" },
            "attachments": [{
                "contentType": "application/vnd.microsoft.card.adaptive",
                "content": card
            }],
            "text": format!(
                "{draft_text}\n\nApprove · Edit · Don't send (draft {draft_id})\nWindow until {deadline_s}."
            ),
        });
        self.post_activity(user_id, activity).await
    }
}

/// In-memory Teams for tests (counts calls; no network).
pub struct MockTeamsClient {
    calls: Mutex<Vec<SlackCall>>,
    counter: AtomicU64,
}

impl MockTeamsClient {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            counter: AtomicU64::new(0),
        })
    }

    pub fn calls(&self) -> Vec<SlackCall> {
        self.calls.lock().clone()
    }
}

#[async_trait]
impl DeliveryClient for MockTeamsClient {
    fn adapter_kind(&self) -> DeliveryAdapterKind {
        DeliveryAdapterKind::Teams
    }

    async fn post_dm(&self, user_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.calls.lock().push(SlackCall {
            kind: "dm".into(),
            target: user_id.into(),
            text: text.into(),
        });
        Ok(DeliveryPostResult {
            channel: format!("teams-dm-{user_id}"),
            ts: format!("teams.dm.{n}"),
        })
    }

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        self.calls.lock().push(SlackCall {
            kind: "channel".into(),
            target: channel_id.into(),
            text: text.into(),
        });
        Ok(DeliveryPostResult {
            channel: channel_id.into(),
            ts: format!("teams.ch.{n}"),
        })
    }

    async fn post_draft_dm(
        &self,
        user_id: &str,
        draft_id: &str,
        draft_text: &str,
        deadline_s: &str,
    ) -> TwinResult<DeliveryPostResult> {
        let text = format!(
            "[AdaptiveCard draft={draft_id}] {draft_text}\nApprove|Edit|Don't send until {deadline_s}"
        );
        self.post_dm(user_id, &text).await
    }

    fn call_count(&self) -> u64 {
        self.calls.lock().len() as u64
    }
}

fn urlencoding_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::DeliveryClient;

    #[tokio::test]
    async fn mock_teams_draft_dm_includes_actions() {
        let client = MockTeamsClient::new();
        let r = client
            .post_draft_dm("29:user-aad", "dft_test", "• Shipped PR #1", "2026-08-05 12:00 UTC")
            .await
            .unwrap();
        assert!(r.channel.contains("user-aad") || r.channel.contains("teams"));
        assert!(!r.ts.is_empty());
        let calls = client.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].text.contains("Approve"));
        assert!(calls[0].text.contains("dft_test"));
        assert_eq!(client.adapter_kind(), DeliveryAdapterKind::Teams);
    }
}
