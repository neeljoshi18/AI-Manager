//! Vertical 2 projector: consume V1 Redpanda topics → context_graph.
//!
//! Production coupling:
//! - Dual-topic consume (`events.raw` + `events.acl` by default)
//! - Offset persistence in Cockroach `projector_offsets` (non-embedded)
//! - HybridMembership when `V1_COCKROACH_URL` is set
//!
//! For local/embedded demos prefer graph-api `POST /v2/project`.

use async_trait::async_trait;
use clap::Parser;
use graph_core::config::GraphConfig;
use graph_core::membership::{InMemoryMembership, MembershipStore};
use graph_core::project::ProjectEngine;
use graph_core::store::{GraphStore, InMemoryGraphStore};
use graph_core::store_crdb::{CrdbGraphStore, CrdbMembership};
use graph_core::v1_event::{V1AclRevocation, V1BusMessage, V1BusPayload, V1CanonicalEvent};
use rskafka::client::partition::{OffsetAt, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

// ── Offset persistence ──────────────────────────────────────────────────────

/// Next Kafka offset to fetch for a (consumer_group, topic, partition) key.
#[async_trait]
pub trait OffsetStore: Send + Sync {
    async fn load_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
    ) -> anyhow::Result<Option<i64>>;

    async fn save_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
        next_offset: i64,
    ) -> anyhow::Result<()>;
}

/// In-memory offsets (embedded mode / unit tests).
#[derive(Default)]
pub struct MemoryOffsetStore {
    inner: parking_lot::Mutex<std::collections::HashMap<(String, String, i32), i64>>,
}

impl MemoryOffsetStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OffsetStore for MemoryOffsetStore {
    async fn load_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
    ) -> anyhow::Result<Option<i64>> {
        let key = (
            consumer_group.to_string(),
            topic.to_string(),
            partition_id,
        );
        Ok(self.inner.lock().get(&key).copied())
    }

    async fn save_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
        next_offset: i64,
    ) -> anyhow::Result<()> {
        let key = (
            consumer_group.to_string(),
            topic.to_string(),
            partition_id,
        );
        self.inner.lock().insert(key, next_offset);
        Ok(())
    }
}

/// Cockroach-backed offsets (`projector_offsets` table).
pub struct CrdbOffsetStore {
    pool: PgPool,
}

impl CrdbOffsetStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(3)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// SQL used by load — exported for documentation / static review.
    pub const LOAD_SQL: &'static str = r#"
        SELECT next_offset FROM projector_offsets
        WHERE consumer_group = $1 AND topic = $2 AND partition_id = $3
    "#;

    /// SQL used by save (upsert).
    pub const SAVE_SQL: &'static str = r#"
        INSERT INTO projector_offsets (consumer_group, topic, partition_id, next_offset, updated_at)
        VALUES ($1, $2, $3, $4, now())
        ON CONFLICT (consumer_group, topic, partition_id) DO UPDATE SET
            next_offset = EXCLUDED.next_offset,
            updated_at = now()
    "#;
}

#[async_trait]
impl OffsetStore for CrdbOffsetStore {
    async fn load_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
    ) -> anyhow::Result<Option<i64>> {
        let row = sqlx::query(Self::LOAD_SQL)
            .bind(consumer_group)
            .bind(topic)
            .bind(partition_id as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<i64, _>("next_offset")))
    }

    async fn save_offset(
        &self,
        consumer_group: &str,
        topic: &str,
        partition_id: i32,
        next_offset: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(Self::SAVE_SQL)
            .bind(consumer_group)
            .bind(topic)
            .bind(partition_id as i64)
            .bind(next_offset)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Resolve starting offset: DB value if present, else broker Earliest.
pub async fn resolve_start_offset(
    offsets: &dyn OffsetStore,
    consumer_group: &str,
    topic: &str,
    partition_id: i32,
    earliest: i64,
) -> anyhow::Result<i64> {
    match offsets
        .load_offset(consumer_group, topic, partition_id)
        .await?
    {
        Some(n) => Ok(n),
        None => Ok(earliest),
    }
}

// ── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "graph-projector")]
struct Args {
    /// Comma-separated Kafka topics (default: events.raw,events.acl).
    /// Env: `KAFKA_TOPICS`. Legacy single-topic env `KAFKA_TOPIC` is honored when
    /// `KAFKA_TOPICS` is unset and this flag is not passed.
    #[arg(long = "topics", env = "KAFKA_TOPICS", default_value = "events.raw,events.acl")]
    topics: String,

    /// Partition to consume (single-partition topics assumed for mid-market).
    #[arg(long, default_value_t = 0)]
    partition: i32,
}

fn parse_topics(raw: &str) -> Vec<String> {
    // Prefer explicit multi-topic string; fall back to legacy KAFKA_TOPIC.
    let src = if raw.trim().is_empty() {
        std::env::var("KAFKA_TOPIC").unwrap_or_else(|_| "events.raw,events.acl".into())
    } else if raw == "events.raw,events.acl" {
        // clap default — allow legacy KAFKA_TOPIC to override when KAFKA_TOPICS unset
        if std::env::var("KAFKA_TOPICS").is_err() {
            if let Ok(legacy) = std::env::var("KAFKA_TOPIC") {
                if !legacy.trim().is_empty() {
                    return legacy
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }
        raw.to_string()
    } else {
        raw.to_string()
    };
    src.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ── Payload handling ────────────────────────────────────────────────────────

/// Apply one bus record value. Returns Ok(()) if handled or intentionally skipped.
async fn handle_payload(engine: &ProjectEngine, value: &[u8]) -> anyhow::Result<()> {
    // Prefer bus envelope; also accept bare CanonicalEvent / bare AclRevocation JSON.
    if let Ok(msg) = serde_json::from_slice::<V1BusMessage>(value) {
        match msg.payload {
            V1BusPayload::Event(ev) => {
                engine.project_event(&ev).await?;
            }
            V1BusPayload::Acl(rev) => {
                engine.project_acl_revocation(&rev).await?;
            }
        }
        return Ok(());
    }
    if let Ok(ev) = serde_json::from_slice::<V1CanonicalEvent>(value) {
        engine.project_event(&ev).await?;
        return Ok(());
    }
    if let Ok(rev) = serde_json::from_slice::<V1AclRevocation>(value) {
        engine.project_acl_revocation(&rev).await?;
        return Ok(());
    }
    // Unrecognized — caller treats as skip (not a hard error for offset commit).
    Err(PayloadError::Unrecognized.into())
}

#[derive(Debug, thiserror::Error)]
enum PayloadError {
    #[error("unrecognized bus payload")]
    Unrecognized,
}

// ── Consume loop ────────────────────────────────────────────────────────────

async fn consume_topic(
    bootstrap: Vec<String>,
    topic: String,
    partition_id: i32,
    consumer_group: String,
    engine: Arc<ProjectEngine>,
    offsets: Arc<dyn OffsetStore>,
) -> anyhow::Result<()> {
    let client = ClientBuilder::new(bootstrap).build().await?;
    let partition = client
        .partition_client(topic.clone(), partition_id, UnknownTopicHandling::Retry)
        .await?;

    let earliest = partition
        .get_offset(OffsetAt::Earliest)
        .await
        .unwrap_or(0);
    let mut offset = resolve_start_offset(
        offsets.as_ref(),
        &consumer_group,
        &topic,
        partition_id,
        earliest,
    )
    .await?;
    info!(%topic, %partition_id, %offset, %earliest, "starting consume");

    loop {
        match partition.fetch_records(offset, 1..1_048_576, 500).await {
            Ok((records, _hw)) => {
                for rec in records {
                    let next = rec.offset + 1;
                    let Some(value) = rec.record.value else {
                        // Empty record — advance
                        offset = next;
                        if let Err(e) = offsets
                            .save_offset(&consumer_group, &topic, partition_id, offset)
                            .await
                        {
                            warn!(error = %e, %topic, "offset save failed");
                        }
                        continue;
                    };

                    match handle_payload(engine.as_ref(), &value).await {
                        Ok(()) => {
                            // Successful project — commit offset AFTER apply
                            offset = next;
                            if let Err(e) = offsets
                                .save_offset(&consumer_group, &topic, partition_id, offset)
                                .await
                            {
                                warn!(error = %e, %topic, "offset save failed after project");
                            }
                        }
                        Err(e) if e.downcast_ref::<PayloadError>().is_some() => {
                            // Bad / unrecognized payload: log + skip (don't crash)
                            warn!(%topic, offset = rec.offset, "unrecognized bus payload; skipping");
                            offset = next;
                            if let Err(se) = offsets
                                .save_offset(&consumer_group, &topic, partition_id, offset)
                                .await
                            {
                                warn!(error = %se, %topic, "offset save failed after skip");
                            }
                        }
                        Err(e) => {
                            // Apply failure: do not advance so we can retry
                            warn!(
                                error = %e,
                                %topic,
                                offset = rec.offset,
                                "project failed; offset not advanced"
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            break; // re-fetch same offset
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, %topic, "fetch failed");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    let args = Args::parse();
    let topics = parse_topics(&args.topics);
    if topics.is_empty() {
        anyhow::bail!("no topics configured (KAFKA_TOPICS / --topics)");
    }
    let cfg = GraphConfig::from_env();
    let partition_id = args.partition;
    let consumer_group = cfg.consumer_group.clone();

    let (store, membership, offsets): (
        Arc<dyn GraphStore>,
        Arc<dyn MembershipStore>,
        Arc<dyn OffsetStore>,
    ) = if cfg.is_embedded() {
        info!("projector embedded mode (in-memory graph + offsets — not durable)");
        (
            InMemoryGraphStore::new(),
            InMemoryMembership::new(),
            Arc::new(MemoryOffsetStore::new()),
        )
    } else {
        let url = cfg
            .cockroach_url
            .clone()
            .ok_or_else(|| anyhow::anyhow!("COCKROACH_URL required for production mode"))?;
        info!(%url, "connecting cockroach context_graph");
        let store: Arc<dyn GraphStore> = CrdbGraphStore::connect(&url).await?;
        let local_mem: Arc<dyn MembershipStore> = CrdbMembership::connect(&url).await?;
        let membership: Arc<dyn MembershipStore> =
            if let Some(v1_url) = cfg.v1_cockroach_url.as_deref() {
                info!(%v1_url, "HybridMembership: live ACL groups from Vertical 1");
                graph_core::membership_v1::HybridMembership::with_v1_identity(local_mem, v1_url)
                    .await?
            } else {
                graph_core::membership_v1::HybridMembership::local_only(local_mem)
            };
        let offset_store: Arc<dyn OffsetStore> =
            Arc::new(CrdbOffsetStore::connect(&url).await?);
        (store, membership, offset_store)
    };

    let engine = Arc::new(ProjectEngine::new(store, membership));

    let brokers = cfg
        .kafka_brokers
        .clone()
        .ok_or_else(|| anyhow::anyhow!("KAFKA_BROKERS required"))?;
    let bootstrap: Vec<String> = brokers
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    info!(?bootstrap, ?topics, %consumer_group, "connecting redpanda");

    // One partition client loop per topic (concurrent).
    let mut handles = Vec::new();
    for topic in topics {
        let bootstrap = bootstrap.clone();
        let engine = engine.clone();
        let offsets = offsets.clone();
        let consumer_group = consumer_group.clone();
        handles.push(tokio::spawn(async move {
            if let Err(e) = consume_topic(
                bootstrap,
                topic.clone(),
                partition_id,
                consumer_group,
                engine,
                offsets,
            )
            .await
            {
                warn!(error = %e, %topic, "consume loop exited");
            }
        }));
    }

    // Wait forever (any task exit is logged; keep others running).
    futures_util::future::join_all(handles).await;
    Ok(())
}

// ── Unit tests (offset helpers; no Redpanda required) ───────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_offset_roundtrip() {
        let store = MemoryOffsetStore::new();
        assert!(store
            .load_offset("g", "events.raw", 0)
            .await
            .unwrap()
            .is_none());
        store
            .save_offset("g", "events.raw", 0, 42)
            .await
            .unwrap();
        assert_eq!(
            store.load_offset("g", "events.raw", 0).await.unwrap(),
            Some(42)
        );
        // Independent keys
        store
            .save_offset("g", "events.acl", 0, 7)
            .await
            .unwrap();
        assert_eq!(
            store.load_offset("g", "events.raw", 0).await.unwrap(),
            Some(42)
        );
        assert_eq!(
            store.load_offset("g", "events.acl", 0).await.unwrap(),
            Some(7)
        );
    }

    #[tokio::test]
    async fn resolve_start_prefers_db_then_earliest() {
        let store = MemoryOffsetStore::new();
        let earliest = 3i64;
        let o = resolve_start_offset(&store, "g", "t", 0, earliest)
            .await
            .unwrap();
        assert_eq!(o, earliest);
        store.save_offset("g", "t", 0, 99).await.unwrap();
        let o2 = resolve_start_offset(&store, "g", "t", 0, earliest)
            .await
            .unwrap();
        assert_eq!(o2, 99);
    }

    #[test]
    fn parse_topics_default_and_csv() {
        // Clear legacy so default path is stable
        std::env::remove_var("KAFKA_TOPIC");
        std::env::remove_var("KAFKA_TOPICS");
        let t = parse_topics("events.raw,events.acl");
        assert_eq!(t, vec!["events.raw", "events.acl"]);
        let t2 = parse_topics(" events.raw , events.acl ");
        assert_eq!(t2, vec!["events.raw", "events.acl"]);
        let t3 = parse_topics("only.one");
        assert_eq!(t3, vec!["only.one"]);
    }

    #[test]
    fn crdb_offset_sql_is_upsert() {
        assert!(CrdbOffsetStore::SAVE_SQL.contains("ON CONFLICT"));
        assert!(CrdbOffsetStore::SAVE_SQL.contains("next_offset"));
        assert!(CrdbOffsetStore::LOAD_SQL.contains("projector_offsets"));
    }

    #[tokio::test]
    async fn handle_payload_rejects_garbage() {
        let store = InMemoryGraphStore::new();
        let mem = InMemoryMembership::new();
        let eng = ProjectEngine::new(store, mem);
        let err = handle_payload(&eng, b"not-json{{{").await.unwrap_err();
        assert!(err.downcast_ref::<PayloadError>().is_some());
    }

    #[tokio::test]
    async fn handle_payload_bare_event() {
        let store = InMemoryGraphStore::new();
        let mem = InMemoryMembership::new();
        let eng = ProjectEngine::new(store, mem);
        let body = serde_json::json!({
            "event_id": "e1",
            "tenant_id": "t",
            "event_type": "pull_request.opened",
            "event_timestamp": "2026-01-01T00:00:00Z",
            "actor": {"global_user_id": "u1", "provider_user_id": "1", "display_name": "A"},
            "acl": {"tenant_id": "t", "allowed_group_ids": [], "is_private": false, "acl_version": 1},
            "resource_id": "acme/app/pr/1",
            "parent_resource_id": "acme/app",
            "attributes": {"title": "x"}
        });
        handle_payload(&eng, &serde_json::to_vec(&body).unwrap())
            .await
            .unwrap();
    }
}
