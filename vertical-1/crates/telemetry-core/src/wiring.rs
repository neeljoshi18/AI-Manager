//! Runtime wiring: construct a complete Vertical 1 stack from AppConfig.

use crate::acl::{AclStore, InMemoryAclStore};
use crate::bus::{EventBus, InMemoryBus};
use crate::config::AppConfig;
use crate::dedup::{DedupStore, InMemoryDedup};
use crate::error::{CoreError, CoreResult};
use crate::metrics::IngestMetrics;
use crate::object_store::{InMemoryObjectStore, ObjectStore};
use crate::pipeline::{IngestPipeline, InMemoryTenantRegistry, TenantRegistry};
use crate::rate_limit::{InMemoryRateLimiter, RateLimiter};
use crate::store::{EventStore, InMemoryEventStore};
use std::sync::Arc;
use tracing::info;

/// Fully wired Vertical 1 runtime (embedded or production handles).
pub struct Vertical1Runtime {
    pub config: AppConfig,
    pub pipeline: Arc<IngestPipeline>,
    pub store: Arc<dyn EventStore>,
    pub acl: Arc<dyn AclStore>,
    pub bus: Arc<dyn EventBus>,
    pub tenants: Arc<dyn TenantRegistry>,
    pub metrics: Arc<IngestMetrics>,
    pub object_store: Arc<dyn ObjectStore>,
}

/// Build the embedded (in-process) stack — no external infrastructure required.
pub fn build_embedded(config: AppConfig) -> Vertical1Runtime {
    let tenants: Arc<dyn TenantRegistry> = InMemoryTenantRegistry::new();
    let rate_limiter: Arc<dyn RateLimiter> =
        InMemoryRateLimiter::new(config.rate_limit_per_minute);
    let dedup: Arc<dyn DedupStore> = InMemoryDedup::new(config.dedup_ttl_secs);
    let object_store: Arc<dyn ObjectStore> = InMemoryObjectStore::new(&config.s3_bucket);
    let acl: Arc<dyn AclStore> = InMemoryAclStore::new();
    let bus: Arc<dyn EventBus> = InMemoryBus::new();
    let store: Arc<dyn EventStore> = InMemoryEventStore::new();
    let metrics = IngestMetrics::new();

    let pipeline = Arc::new(IngestPipeline {
        tenants: tenants.clone(),
        auth_skip: config.skip_auth,
        rate_limiter,
        dedup,
        object_store: object_store.clone(),
        acl: acl.clone(),
        bus: bus.clone(),
        metrics: metrics.clone(),
        inline_store: Some(store.clone()),
    });

    Vertical1Runtime {
        config,
        pipeline,
        store,
        acl,
        bus,
        tenants,
        metrics,
        object_store,
    }
}

/// Build from environment.
pub fn build_from_env() -> Vertical1Runtime {
    let config = AppConfig::from_env();
    if config.is_embedded() {
        info!("runtime mode=embedded");
        return build_embedded(config);
    }

    #[cfg(feature = "production")]
    {
        info!("runtime mode=production — connecting external backends");
        match build_production_blocking(config) {
            Ok(rt) => return rt,
            Err(e) => {
                panic!("production backend wiring failed: {e}");
            }
        }
    }

    #[cfg(not(feature = "production"))]
    {
        tracing::warn!(
            "RUNTIME_MODE=production but binary built without `production` feature; using embedded"
        );
        build_embedded(config)
    }
}

/// Async production builder (preferred for async contexts / tests).
#[cfg(feature = "production")]
pub async fn build_production(config: AppConfig) -> CoreResult<Vertical1Runtime> {
    use crate::production::{
        ClickHouseEventStore, CockroachAclStore, CockroachTenantRegistry, RedisDedup,
        RedisRateLimiter, RedpandaBus, S3ObjectStore,
    };

    let redis_url = config
        .redis_url
        .as_deref()
        .ok_or_else(|| CoreError::Storage("REDIS_URL required".into()))?;
    let kafka = config
        .kafka_brokers
        .as_deref()
        .ok_or_else(|| CoreError::Bus("KAFKA_BROKERS required".into()))?;
    let crdb = config
        .cockroach_url
        .as_deref()
        .ok_or_else(|| CoreError::Storage("COCKROACH_URL required".into()))?;
    let ch_url = config
        .clickhouse_url
        .as_deref()
        .ok_or_else(|| CoreError::Storage("CLICKHOUSE_URL required".into()))?;
    let s3_endpoint = config
        .s3_endpoint
        .as_deref()
        .ok_or_else(|| CoreError::ObjectStore("S3_ENDPOINT required".into()))?;
    let s3_ak = config
        .s3_access_key
        .as_deref()
        .ok_or_else(|| CoreError::ObjectStore("S3_ACCESS_KEY required".into()))?;
    let s3_sk = config
        .s3_secret_key
        .as_deref()
        .ok_or_else(|| CoreError::ObjectStore("S3_SECRET_KEY required".into()))?;

    info!(%redis_url, "connecting redis");
    let dedup: Arc<dyn DedupStore> = RedisDedup::connect(redis_url, config.dedup_ttl_secs).await?;
    let rate_limiter: Arc<dyn RateLimiter> =
        RedisRateLimiter::connect(redis_url, config.rate_limit_per_minute).await?;

    info!("connecting cockroachdb");
    let acl: Arc<dyn AclStore> = CockroachAclStore::connect(crdb).await?;
    let tenants: Arc<dyn TenantRegistry> = CockroachTenantRegistry::connect(crdb).await?;

    info!(%ch_url, "connecting clickhouse");
    let ch = ClickHouseEventStore::connect(
        ch_url,
        &config.clickhouse_database,
        &config.clickhouse_user,
        &config.clickhouse_password,
    )?;
    ch.ping().await?;
    let store: Arc<dyn EventStore> = ch;

    info!(%s3_endpoint, "connecting minio/s3");
    let object_store: Arc<dyn ObjectStore> = S3ObjectStore::connect(
        s3_endpoint,
        &config.s3_region,
        &config.s3_bucket,
        s3_ak,
        s3_sk,
    )
    .await?;

    info!(%kafka, "connecting redpanda");
    let bus: Arc<dyn EventBus> = RedpandaBus::connect(kafka).await?;

    let metrics = IngestMetrics::new();
    let inline = if config.inline_consumer {
        info!("INLINE_CONSUMER=true — ingest path also writes ClickHouse");
        Some(store.clone())
    } else {
        info!("INLINE_CONSUMER=false — rely on bus consumer for ClickHouse writes");
        None
    };

    let pipeline = Arc::new(IngestPipeline {
        tenants: tenants.clone(),
        auth_skip: config.skip_auth,
        rate_limiter,
        dedup,
        object_store: object_store.clone(),
        acl: acl.clone(),
        bus: bus.clone(),
        metrics: metrics.clone(),
        inline_store: inline,
    });

    info!("production stack ready");
    Ok(Vertical1Runtime {
        config,
        pipeline,
        store,
        acl,
        bus,
        tenants,
        metrics,
        object_store,
    })
}

#[cfg(feature = "production")]
fn build_production_blocking(config: AppConfig) -> CoreResult<Vertical1Runtime> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(build_production(config))),
        Err(_) => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| CoreError::Internal(format!("tokio runtime: {e}")))?;
            rt.block_on(build_production(config))
        }
    }
}
