//! Vertical 2 projector: consume V1 Redpanda topics → context_graph.
//!
//! For local/embedded demos prefer graph-api `POST /v2/project`.
//! This binary is for production bus coupling.

use clap::Parser;
use graph_core::config::GraphConfig;
use graph_core::membership::InMemoryMembership;
use graph_core::project::ProjectEngine;
use graph_core::store::InMemoryGraphStore;
use graph_core::store_crdb::{CrdbGraphStore, CrdbMembership};
use graph_core::store::GraphStore;
use graph_core::membership::MembershipStore;
use graph_core::v1_event::{V1BusMessage, V1BusPayload};
use rskafka::client::partition::{OffsetAt, UnknownTopicHandling};
use rskafka::client::ClientBuilder;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "graph-projector")]
struct Args {
    #[arg(long, env = "KAFKA_TOPIC", default_value = "events.raw")]
    topic: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();
    let args = Args::parse();
    let cfg = GraphConfig::from_env();

    let (store, membership): (Arc<dyn GraphStore>, Arc<dyn MembershipStore>) =
        if cfg.is_embedded() {
            info!("projector embedded mode (in-memory — not durable across restarts)");
            (InMemoryGraphStore::new(), InMemoryMembership::new())
        } else {
            let url = cfg
                .cockroach_url
                .clone()
                .ok_or_else(|| anyhow::anyhow!("COCKROACH_URL required"))?;
            (
                CrdbGraphStore::connect(&url).await?,
                CrdbMembership::connect(&url).await?,
            )
        };
    let engine = ProjectEngine::new(store, membership);

    let brokers = cfg
        .kafka_brokers
        .clone()
        .ok_or_else(|| anyhow::anyhow!("KAFKA_BROKERS required"))?;
    let bootstrap: Vec<String> = brokers.split(',').map(|s| s.trim().to_string()).collect();
    info!(?bootstrap, topic = %args.topic, "connecting redpanda");
    let client = ClientBuilder::new(bootstrap).build().await?;
    let partition = client
        .partition_client(args.topic.clone(), 0, UnknownTopicHandling::Retry)
        .await?;

    // Start from earliest for rebuild friendliness; production would use committed offsets.
    let mut offset = partition
        .get_offset(OffsetAt::Earliest)
        .await
        .unwrap_or(0);
    info!(%offset, "starting consume");

    loop {
        match partition
            .fetch_records(offset, 1..1_048_576, 500)
            .await
        {
            Ok((records, _hw)) => {
                for rec in records {
                    offset = rec.offset + 1;
                    let Some(value) = rec.record.value else { continue };
                    // Accept either BusMessage envelope or bare CanonicalEvent JSON
                    if let Ok(msg) = serde_json::from_slice::<V1BusMessage>(&value) {
                        match msg.payload {
                            V1BusPayload::Event(ev) => {
                                if let Err(e) = engine.project_event(&ev).await {
                                    warn!(error = %e, "project event failed");
                                }
                            }
                            V1BusPayload::Acl(rev) => {
                                if let Err(e) = engine.project_acl_revocation(&rev).await {
                                    warn!(error = %e, "project acl failed");
                                }
                            }
                        }
                    } else if let Ok(ev) =
                        serde_json::from_slice::<graph_core::v1_event::V1CanonicalEvent>(&value)
                    {
                        if let Err(e) = engine.project_event(&ev).await {
                            warn!(error = %e, "project bare event failed");
                        }
                    } else {
                        warn!("unrecognized bus payload");
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "fetch failed");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
