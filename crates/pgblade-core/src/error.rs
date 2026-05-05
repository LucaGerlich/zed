#[derive(Debug, Clone, thiserror::Error)]
pub enum ConnectionError {
    #[error("host resolution failed: {host}")]
    HostResolution { host: String },

    #[error("authentication failed for user '{user}'")]
    Authentication { user: String },

    #[error("SSL negotiation failed: {reason}")]
    Ssl { reason: String },

    #[error("database '{database}' does not exist")]
    DatabaseNotFound { database: String },

    #[error("connection refused: {reason}")]
    Refused { reason: String },

    #[error("connection timed out after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    #[error("{message}")]
    Other { message: String },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum QueryError {
    #[error("{severity}: {message} ({code})")]
    Postgres {
        code: String,
        severity: String,
        message: String,
        detail: Option<String>,
        hint: Option<String>,
        position: Option<u32>,
    },

    #[error("query cancelled")]
    Cancelled,

    #[error("not connected to a database")]
    NotConnected,

    #[error("write blocked: connection is in read-only mode")]
    ReadOnlyViolation,

    #[error("{message}")]
    Other { message: String },
}
