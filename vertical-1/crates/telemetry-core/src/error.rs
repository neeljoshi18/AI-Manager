use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("duplicate event: {event_id}")]
    Duplicate { event_id: String },

    #[error("validation error: {0}")]
    Validation(String),

    #[error("normalization failed: {0}")]
    Normalization(String),

    #[error("acl denied: {0}")]
    AclDenied(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("bus error: {0}")]
    Bus(String),

    #[error("object store error: {0}")]
    ObjectStore(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("schema mutation / dead-letter: {0}")]
    DeadLetter(String),
}

impl CoreError {
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            CoreError::Auth(_)
                | CoreError::RateLimited { .. }
                | CoreError::Validation(_)
                | CoreError::AclDenied(_)
        )
    }

    /// Whether the edge should still ACK 200 (idempotent duplicate).
    pub fn is_benign_duplicate(&self) -> bool {
        matches!(self, CoreError::Duplicate { .. })
    }
}
