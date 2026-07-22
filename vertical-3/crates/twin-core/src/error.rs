use thiserror::Error;

pub type TwinResult<T> = Result<T, TwinError>;

#[derive(Debug, Error)]
pub enum TwinError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("acl denied: {0}")]
    AclDenied(String),
    #[error("egress: {0}")]
    Egress(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("upstream: {0}")]
    Upstream(String),
    #[error("internal: {0}")]
    Internal(String),
}
