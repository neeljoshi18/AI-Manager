//! Vertical 2 graph domain: project V1 events → ACL-safe context graph.

pub mod acl;
pub mod config;
pub mod error;
pub mod ids;
pub mod intent;
pub mod membership;
pub mod model;
pub mod project;
pub mod store;
pub mod v1_event;

#[cfg(feature = "production")]
pub mod membership_v1;
#[cfg(feature = "production")]
pub mod store_crdb;

pub use error::{GraphError, GraphResult};
pub use model::*;
pub use project::ProjectEngine;
pub use store::{GraphStore, InMemoryGraphStore};
pub use membership::{InMemoryMembership, MembershipStore};
pub use v1_event::V1CanonicalEvent;
