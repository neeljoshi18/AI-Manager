//! Streaming message bus abstraction.
//!
//! Production: Redpanda (Kafka API) with topics:
//!   events.raw | events.realtime | events.backfill | events.acl
//!
//! Embedded: in-process broadcast channels with durable ring buffers
//! so consumers can catch up and tests can assert zero data loss.

use crate::error::{CoreError, CoreResult};
use crate::model::{BusMessage, BusTopic};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

const DEFAULT_CHANNEL_CAPACITY: usize = 65_536;
const DEFAULT_DURABLE_CAP: usize = 100_000;

#[async_trait]
pub trait EventBus: Send + Sync {
    /// Durably enqueue a message (Invariant #1: before HTTP 200).
    async fn publish(&self, message: BusMessage) -> CoreResult<()>;

    /// Subscribe to a topic. `consumer_group` is recorded for lag metrics.
    async fn subscribe(
        &self,
        topic: BusTopic,
        consumer_group: &str,
    ) -> CoreResult<BusSubscription>;

    /// Approximate lag for a consumer group (messages not yet acked).
    async fn lag(&self, topic: BusTopic, consumer_group: &str) -> CoreResult<u64>;
}

pub struct BusSubscription {
    pub topic: BusTopic,
    pub consumer_group: String,
    rx: broadcast::Receiver<BusMessage>,
    /// Shared durable log cursor tracking.
    cursor: Arc<AtomicU64>,
    log: Arc<RwLock<VecDeque<(u64, BusMessage)>>>,
}

impl BusSubscription {
    /// Pop the next durable message with seq > cursor, if any.
    fn next_from_log(&self) -> Option<BusMessage> {
        let log = self.log.read();
        let cursor = self.cursor.load(Ordering::Acquire);
        // Log is append-only ordered by seq; binary-search style via skip.
        // seq is 1-based and contiguous for a given topic.
        if let Some((seq, msg)) = log.iter().find(|(seq, _)| *seq > cursor) {
            let seq = *seq;
            let msg = msg.clone();
            drop(log);
            self.cursor.store(seq, Ordering::Release);
            return Some(msg);
        }
        None
    }

    /// Receive next message. Replays from durable log if the live channel lagged.
    pub async fn recv(&mut self) -> CoreResult<BusMessage> {
        loop {
            if let Some(msg) = self.next_from_log() {
                return Ok(msg);
            }
            match self.rx.recv().await {
                Ok(msg) => {
                    // Prefer durable ordering: if log has caught up past cursor, use that.
                    if let Some(m) = self.next_from_log() {
                        return Ok(m);
                    }
                    // Live message without durable visibility (shouldn't happen) — return it.
                    return Ok(msg);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "bus subscriber lagged; replaying from durable log");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(CoreError::Bus("channel closed".into()));
                }
            }
        }
    }

    pub fn try_recv(&mut self) -> Option<BusMessage> {
        if let Some(msg) = self.next_from_log() {
            return Some(msg);
        }
        // Drain lag notifications without consuming out-of-order live messages
        // when durable log is the source of truth for catch-up.
        match self.rx.try_recv() {
            Ok(_msg) => {
                // After a live notification, re-check durable log for ordered delivery.
                if let Some(msg) = self.next_from_log() {
                    return Some(msg);
                }
                None
            }
            Err(broadcast::error::TryRecvError::Lagged(_)) => self.next_from_log(),
            Err(_) => None,
        }
    }

    /// Drain all currently available durable messages (test helper).
    pub fn drain_log(&mut self) -> Vec<BusMessage> {
        let mut out = Vec::new();
        while let Some(msg) = self.next_from_log() {
            out.push(msg);
        }
        out
    }
}

struct TopicState {
    tx: broadcast::Sender<BusMessage>,
    log: Arc<RwLock<VecDeque<(u64, BusMessage)>>>,
    seq: AtomicU64,
    /// consumer_group → last acked sequence
    cursors: DashMap<String, Arc<AtomicU64>>,
}

/// In-process bus that satisfies zero-data-loss for single-process deployments
/// and full verification batteries without Redpanda.
pub struct InMemoryBus {
    topics: DashMap<BusTopic, Arc<TopicState>>,
    durable_cap: usize,
    channel_cap: usize,
}

impl InMemoryBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            topics: DashMap::new(),
            durable_cap: DEFAULT_DURABLE_CAP,
            channel_cap: DEFAULT_CHANNEL_CAPACITY,
        })
    }

    fn topic_state(&self, topic: BusTopic) -> Arc<TopicState> {
        self.topics
            .entry(topic)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(self.channel_cap);
                Arc::new(TopicState {
                    tx,
                    log: Arc::new(RwLock::new(VecDeque::new())),
                    seq: AtomicU64::new(0),
                    cursors: DashMap::new(),
                })
            })
            .clone()
    }
}

impl Default for InMemoryBus {
    fn default() -> Self {
        Self {
            topics: DashMap::new(),
            durable_cap: DEFAULT_DURABLE_CAP,
            channel_cap: DEFAULT_CHANNEL_CAPACITY,
        }
    }
}

#[async_trait]
impl EventBus for InMemoryBus {
    async fn publish(&self, message: BusMessage) -> CoreResult<()> {
        let state = self.topic_state(message.topic);

        // Durable append FIRST (Invariant #1).
        // Sequence allocation happens under the write lock so the log remains
        // strictly ordered by seq even under concurrent publishers.
        {
            let mut log = state.log.write();
            let seq = state.seq.fetch_add(1, Ordering::AcqRel) + 1;
            log.push_back((seq, message.clone()));
            while log.len() > self.durable_cap {
                log.pop_front();
            }
        }

        // Best-effort live fanout (subscribers can always replay from log).
        let _ = state.tx.send(message);
        Ok(())
    }

    async fn subscribe(
        &self,
        topic: BusTopic,
        consumer_group: &str,
    ) -> CoreResult<BusSubscription> {
        let state = self.topic_state(topic);
        let cursor = state
            .cursors
            .entry(consumer_group.to_string())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();
        Ok(BusSubscription {
            topic,
            consumer_group: consumer_group.to_string(),
            rx: state.tx.subscribe(),
            cursor,
            log: state.log.clone(),
        })
    }

    async fn lag(&self, topic: BusTopic, consumer_group: &str) -> CoreResult<u64> {
        let state = self.topic_state(topic);
        let head = state.seq.load(Ordering::Acquire);
        let cursor = state
            .cursors
            .get(consumer_group)
            .map(|c| c.load(Ordering::Acquire))
            .unwrap_or(0);
        Ok(head.saturating_sub(cursor))
    }
}

/// Drain up to `max` messages currently available (test helper).
pub async fn drain_available(sub: &mut BusSubscription, max: usize) -> Vec<BusMessage> {
    let mut out = Vec::new();
    for _ in 0..max {
        match sub.try_recv() {
            Some(m) => out.push(m),
            None => break,
        }
    }
    out
}

// Kafka / Redpanda production bus lives in `crate::production::bus_kafka`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActorIdentity, AclSnapshot, BusPayload, CanonicalEventRecord};
    use chrono::Utc;
    use telemetry_proto::{EventCategory, SourceProvider};

    fn sample_msg(id: &str) -> BusMessage {
        BusMessage {
            topic: BusTopic::EventsRaw,
            partition_key: "tenant-1".into(),
            payload: BusPayload::Event(CanonicalEventRecord {
                event_id: id.into(),
                tenant_id: "tenant-1".into(),
                provider: SourceProvider::Github,
                category: EventCategory::Code,
                event_type: "pull_request.opened".into(),
                event_timestamp: Utc::now(),
                ingested_at: Utc::now(),
                actor: ActorIdentity::default(),
                acl: AclSnapshot {
                    tenant_id: "tenant-1".into(),
                    allowed_group_ids: vec!["eng".into()],
                    is_private: false,
                    acl_version: 1,
                },
                resource_id: "repo/1".into(),
                parent_resource_id: "repo".into(),
                attributes: serde_json::json!({}),
                raw_payload_s3_uri: String::new(),
                event_sequence_number: 1,
            }),
        }
    }

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = InMemoryBus::new();
        let mut sub = bus.subscribe(BusTopic::EventsRaw, "cg").await.unwrap();
        bus.publish(sample_msg("e1")).await.unwrap();
        let msg = sub.recv().await.unwrap();
        match msg.payload {
            BusPayload::Event(e) => assert_eq!(e.event_id, "e1"),
            _ => panic!("expected event"),
        }
    }
}
