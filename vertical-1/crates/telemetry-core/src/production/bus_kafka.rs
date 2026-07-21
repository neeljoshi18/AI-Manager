//! Redpanda / Kafka bus via pure-Rust `rskafka`.
//!
//! Publish is durable to the broker before returning Ok.
//! Subscribe fans out through an in-process durable log that is also filled
//! by a background poller — so `run_consumer_loop` works unchanged while
//! messages remain on Redpanda for multi-process consumers.

use crate::bus::{BusSubscription, EventBus, InMemoryBus};
use crate::error::{CoreError, CoreResult};
use crate::model::{BusMessage, BusTopic};
use async_trait::async_trait;
use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use rskafka::record::Record;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub struct RedpandaBus {
    client: rskafka::client::Client,
    /// Local fan-out so existing subscribe/consumer-loop APIs keep working.
    local: Arc<InMemoryBus>,
    /// Next offset to poll per topic partition 0 (single-partition topics).
    offsets: Mutex<BTreeMap<String, i64>>,
}

impl RedpandaBus {
    pub async fn connect(brokers: &str) -> CoreResult<Arc<Self>> {
        let bootstrap: Vec<String> = brokers
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if bootstrap.is_empty() {
            return Err(CoreError::Bus("empty KAFKA_BROKERS".into()));
        }

        let client = ClientBuilder::new(bootstrap)
            .build()
            .await
            .map_err(|e| CoreError::Bus(format!("rskafka client: {e}")))?;

        // Ensure required topics exist.
        let controller = client
            .controller_client()
            .map_err(|e| CoreError::Bus(format!("controller client: {e}")))?;
        for topic in [
            BusTopic::EventsRaw,
            BusTopic::EventsRealtime,
            BusTopic::EventsBackfill,
            BusTopic::EventsAcl,
        ] {
            let name = topic.as_str().to_string();
            match controller
                .create_topic(name.clone(), 1, 1, 5_000)
                .await
            {
                Ok(()) => info!(topic = %name, "created kafka topic"),
                Err(e) => {
                    // Topic may already exist — continue.
                    tracing::debug!(topic = %name, error = %e, "create_topic (may already exist)");
                }
            }
        }

        let bus = Arc::new(Self {
            client,
            local: InMemoryBus::new(),
            offsets: Mutex::new(BTreeMap::new()),
        });

        // Background poller: Redpanda → local bus for in-process subscribers.
        let bg = Arc::clone(&bus);
        tokio::spawn(async move {
            loop {
                if let Err(e) = bg.poll_once().await {
                    warn!(error = %e, "redpanda poll error");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                } else {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        });

        Ok(bus)
    }

    async fn produce(&self, message: &BusMessage) -> CoreResult<()> {
        let topic = message.topic.as_str().to_string();
        let payload = serde_json::to_vec(message)
            .map_err(|e| CoreError::Bus(format!("serialize bus message: {e}")))?;

        let partition = self
            .client
            .partition_client(topic.clone(), 0, UnknownTopicHandling::Retry)
            .await
            .map_err(|e| CoreError::Bus(format!("partition client {topic}: {e}")))?;

        let record = Record {
            key: Some(message.partition_key.as_bytes().to_vec()),
            value: Some(payload),
            headers: BTreeMap::new(),
            timestamp: chrono::Utc::now(),
        };

        partition
            .produce(vec![record], Compression::NoCompression)
            .await
            .map_err(|e| CoreError::Bus(format!("produce {topic}: {e}")))?;
        Ok(())
    }

    async fn poll_once(&self) -> CoreResult<()> {
        for topic in [
            BusTopic::EventsRaw,
            BusTopic::EventsRealtime,
            BusTopic::EventsBackfill,
            BusTopic::EventsAcl,
        ] {
            let name = topic.as_str().to_string();
            let partition = match self
                .client
                .partition_client(name.clone(), 0, UnknownTopicHandling::Retry)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!(topic = %name, error = %e, "partition client failed");
                    continue;
                }
            };

            let offset = {
                let offsets = self.offsets.lock().await;
                *offsets.get(&name).unwrap_or(&0)
            };

            // High watermark / fetch
            let (records, _hw) = match partition
                .fetch_records(offset, 1..1_048_576, 200)
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };

            let mut next = offset;
            for rec in records {
                next = rec.offset + 1;
                if let Some(value) = rec.record.value {
                    match serde_json::from_slice::<BusMessage>(&value) {
                        Ok(msg) => {
                            // Avoid re-publish loop: write only to local fanout.
                            let _ = self.local.publish(msg).await;
                        }
                        Err(e) => warn!(error = %e, "bad bus payload"),
                    }
                }
            }
            if next != offset {
                self.offsets.lock().await.insert(name, next);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl EventBus for RedpandaBus {
    async fn publish(&self, message: BusMessage) -> CoreResult<()> {
        // Durable first (Invariant #1).
        self.produce(&message).await?;
        // Local fanout for same-process consumers (poller will also deliver;
        // InMemoryDedup-style bus dedup isn't needed — consumer is idempotent via CH).
        self.local.publish(message).await?;
        Ok(())
    }

    async fn subscribe(
        &self,
        topic: BusTopic,
        consumer_group: &str,
    ) -> CoreResult<BusSubscription> {
        self.local.subscribe(topic, consumer_group).await
    }

    async fn lag(&self, topic: BusTopic, consumer_group: &str) -> CoreResult<u64> {
        self.local.lag(topic, consumer_group).await
    }
}
