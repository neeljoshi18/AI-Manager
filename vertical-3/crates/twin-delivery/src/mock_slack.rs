use crate::delivery::{DeliveryAdapterKind, DeliveryClient, DeliveryPostResult};
use crate::slack::{SlackClient, SlackPostResult};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use twin_core::TwinResult;

#[derive(Debug, Clone)]
pub struct SlackCall {
    pub kind: String,
    pub target: String,
    pub text: String,
}

/// In-memory Slack for TC-T* (counts calls; no network).
pub struct MockSlackClient {
    calls: Mutex<Vec<SlackCall>>,
    counter: AtomicU64,
    fail_channel: Mutex<bool>,
}

impl MockSlackClient {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            counter: AtomicU64::new(0),
            fail_channel: Mutex::new(false),
        })
    }

    pub fn calls(&self) -> Vec<SlackCall> {
        self.calls.lock().clone()
    }

    pub fn channel_posts(&self) -> Vec<SlackCall> {
        self.calls
            .lock()
            .iter()
            .filter(|c| c.kind == "channel")
            .cloned()
            .collect()
    }

    pub fn dm_posts(&self) -> Vec<SlackCall> {
        self.calls
            .lock()
            .iter()
            .filter(|c| c.kind == "dm")
            .cloned()
            .collect()
    }

    pub fn set_fail_channel(&self, fail: bool) {
        *self.fail_channel.lock() = fail;
    }
}

impl Default for MockSlackClient {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            counter: AtomicU64::new(0),
            fail_channel: Mutex::new(false),
        }
    }
}

#[async_trait]
impl SlackClient for MockSlackClient {
    async fn post_dm(&self, slack_user_id: &str, text: &str) -> TwinResult<SlackPostResult> {
        DeliveryClient::post_dm(self, slack_user_id, text).await
    }

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<SlackPostResult> {
        DeliveryClient::post_channel(self, channel_id, text).await
    }

    fn call_count(&self) -> u64 {
        DeliveryClient::call_count(self)
    }
}

#[async_trait]
impl DeliveryClient for MockSlackClient {
    fn adapter_kind(&self) -> DeliveryAdapterKind {
        DeliveryAdapterKind::Mock
    }

    async fn post_dm(&self, slack_user_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let ts = format!("dm.{n}");
        self.calls.lock().push(SlackCall {
            kind: "dm".into(),
            target: slack_user_id.into(),
            text: text.into(),
        });
        Ok(DeliveryPostResult {
            channel: format!("D{slack_user_id}"),
            ts,
        })
    }

    async fn post_channel(&self, channel_id: &str, text: &str) -> TwinResult<DeliveryPostResult> {
        if *self.fail_channel.lock() {
            return Err(twin_core::TwinError::Egress("mock channel fail".into()));
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let ts = format!("ch.{n}");
        self.calls.lock().push(SlackCall {
            kind: "channel".into(),
            target: channel_id.into(),
            text: text.into(),
        });
        Ok(DeliveryPostResult {
            channel: channel_id.into(),
            ts,
        })
    }

    fn call_count(&self) -> u64 {
        self.calls.lock().len() as u64
    }
}
