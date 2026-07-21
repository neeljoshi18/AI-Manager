//! Shared domain types, storage traits, and backend implementations for Vertical 1.
//!
//! ## Invariants (from Technical Architecture Spec)
//! 1. **Zero data loss** — durable append before HTTP 200
//! 2. **Zero-trust ACL isolation** — query-time group membership filter
//! 3. **Sub-50ms P99 ingestion** — auth + enqueue only at the edge
//! 4. **Deterministic type safety** — Protobuf canonical events

pub mod acl;
pub mod auth;
pub mod bus;
pub mod config;
pub mod dedup;
pub mod error;
pub mod metrics;
pub mod model;
pub mod normalize;
pub mod object_store;
pub mod pipeline;
pub mod production;
pub mod rate_limit;
pub mod store;
pub mod time;
pub mod wiring;

pub use error::{CoreError, CoreResult};
pub use model::*;

/// Runtime mode selecting which concrete backends to wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    /// In-process stores — no external infrastructure required.
    Embedded,
    /// Production Redis / Redpanda / CockroachDB / ClickHouse / S3.
    Production,
}
