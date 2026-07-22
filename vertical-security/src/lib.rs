//! Centaur-inspired credential egress proxy library.
//!
//! Untrusted workers call external APIs through this proxy with
//! `X-AI-Manager-Tool` and **no** real Authorization. The proxy looks up
//! the tool → secret, injects the header, and forwards.
//!
//! Fail-closed: unknown hosts and missing secrets are rejected.

pub mod proxy;
pub mod redact;
pub mod registry;
pub mod secrets;

pub use proxy::{build_router, ProxyState};
pub use redact::redact_secrets;
pub use registry::{ToolConfig, ToolRegistry};
pub use secrets::{load_secrets_file, SecretsStore};
