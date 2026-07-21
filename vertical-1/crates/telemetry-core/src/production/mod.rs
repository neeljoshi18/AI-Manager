//! Production backends: Redis, Redpanda (rskafka), CockroachDB, ClickHouse, MinIO.
//!
//! Enabled with feature `production`.

#[cfg(feature = "production")]
pub mod acl_crdb;
#[cfg(feature = "production")]
pub mod bus_kafka;
#[cfg(feature = "production")]
pub mod object_s3;
#[cfg(feature = "production")]
pub mod store_ch;
#[cfg(feature = "production")]
pub mod tenants_crdb;

#[cfg(feature = "production")]
pub use acl_crdb::CockroachAclStore;
#[cfg(feature = "production")]
pub use bus_kafka::RedpandaBus;
#[cfg(feature = "production")]
pub use object_s3::S3ObjectStore;
#[cfg(feature = "production")]
pub use store_ch::ClickHouseEventStore;
#[cfg(feature = "production")]
pub use tenants_crdb::CockroachTenantRegistry;

// Re-export redis helpers from existing modules when production is on.
#[cfg(feature = "production")]
pub use crate::dedup::redis_impl::RedisDedup;
#[cfg(feature = "production")]
pub use crate::rate_limit::redis_impl::RedisRateLimiter;
