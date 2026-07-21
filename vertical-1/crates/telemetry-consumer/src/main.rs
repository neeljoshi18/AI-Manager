//! Vertical 1 — Stream consumer (Redpanda/embedded bus → analytical store).
//!
//! In embedded single-process deployments the ingestion service writes inline;
//! this binary is for multi-process / production topologies and verification.

use clap::Parser;
use std::sync::Arc;
use telemetry_core::pipeline::run_consumer_loop;
use telemetry_core::wiring::build_from_env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "telemetry-consumer", about = "Vertical 1 bus → store consumer")]
struct Args {
    /// Consumer group id
    #[arg(long, env = "CONSUMER_GROUP", default_value = "v1-clickhouse-writer")]
    consumer_group: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args = Args::parse();
    let rt = Arc::new(build_from_env());
    info!(
        group = %args.consumer_group,
        mode = %rt.config.runtime_mode,
        "telemetry-consumer starting"
    );

    let (tx, rx) = tokio::sync::watch::channel(false);

    // Graceful shutdown on ctrl-c
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("ctrl-c received");
        let _ = tx_clone.send(true);
    });

    run_consumer_loop(
        rt.bus.clone(),
        rt.store.clone(),
        rt.acl.clone(),
        &args.consumer_group,
        rx,
    )
    .await?;

    Ok(())
}
