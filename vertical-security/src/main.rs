//! Egress proxy binary — injects vault secrets into outbound API calls.

use anyhow::{Context, Result};
use clap::Parser;
use egress_proxy::{build_router, load_secrets_file, ProxyState, ToolRegistry};
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "egress-proxy",
    about = "Centaur-inspired credential egress proxy for AI Manager"
)]
struct Args {
    /// Listen address.
    #[arg(long, env = "EGRESS_BIND", default_value = "0.0.0.0:18090")]
    bind: SocketAddr,

    /// Path to tool registry YAML.
    #[arg(
        long,
        env = "TOOL_REGISTRY",
        default_value = "config/tool_registry.yaml"
    )]
    registry: PathBuf,

    /// Path to secrets JSON (name → value).
    #[arg(long, env = "SECRETS_FILE", default_value = "secrets/dev_secrets.json")]
    secrets: PathBuf,

    /// Disable response-body secret redaction.
    #[arg(long, env = "EGRESS_NO_REDACT", default_value_t = false)]
    no_redact: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    let args = Args::parse();

    let registry = ToolRegistry::load(&args.registry)
        .with_context(|| format!("load registry from {}", args.registry.display()))?;
    info!(tools = registry.len(), path = %args.registry.display(), "loaded tool registry");

    let secrets = load_secrets_file(&args.secrets)
        .with_context(|| format!("load secrets from {}", args.secrets.display()))?;
    // Log names only — never values.
    let names: Vec<&str> = secrets.names().collect();
    info!(
        count = secrets.len(),
        secrets = ?names,
        path = %args.secrets.display(),
        "loaded secrets (names only)"
    );

    let mut state = ProxyState::new(registry, secrets);
    state.redact_responses = !args.no_redact;

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(args.bind)
        .await
        .with_context(|| format!("bind {}", args.bind))?;
    info!(%args.bind, "egress-proxy listening");

    axum::serve(listener, app)
        .await
        .context("serve")?;
    Ok(())
}
