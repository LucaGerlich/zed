use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::QueryError;

/// Opaque identifier for a query execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryId(pub Uuid);

impl QueryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for QueryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Advisory classification of a SQL statement.
/// Used to drive safety UI — not a security boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryClassification {
    /// SELECT, EXPLAIN, SHOW, etc.
    Read,
    /// INSERT, UPDATE, COPY
    Write,
    /// DROP, TRUNCATE, DELETE (without WHERE is especially dangerous)
    Destructive,
    /// BEGIN, COMMIT, ROLLBACK, SET
    TransactionControl,
    /// Could not classify — treat as potentially dangerous.
    Unknown,
}

impl QueryClassification {
    pub fn is_safe(&self) -> bool {
        matches!(self, Self::Read | Self::TransactionControl)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Destructive => "Destructive",
            Self::TransactionControl => "Transaction",
            Self::Unknown => "Unknown",
        }
    }
}

/// State machine for a single query execution lifecycle.
#[derive(Debug, Clone)]
pub enum QueryState {
    /// SQL text ready but not submitted.
    Pending,

    /// Submitted to the database, awaiting first results.
    Executing { started_at: DateTime<Utc> },

    /// First page received, possibly more pages available.
    Streaming {
        started_at: DateTime<Utc>,
        rows_received: u64,
    },

    /// Execution complete, all results available.
    Completed {
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        total_rows: u64,
    },

    /// User or system cancelled the query.
    Cancelled {
        started_at: DateTime<Utc>,
        cancelled_at: DateTime<Utc>,
    },

    /// Query failed with a structured error.
    Failed {
        started_at: DateTime<Utc>,
        error: QueryError,
    },
}

impl QueryState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Cancelled { .. } | Self::Failed { .. }
        )
    }

    pub fn is_running(&self) -> bool {
        matches!(self, Self::Executing { .. } | Self::Streaming { .. })
    }

    pub fn elapsed_ms(&self) -> Option<i64> {
        match self {
            Self::Pending => None,
            Self::Executing { started_at } | Self::Streaming { started_at, .. } => {
                Some((Utc::now() - started_at).num_milliseconds())
            }
            Self::Completed {
                started_at,
                completed_at,
                ..
            } => Some((*completed_at - *started_at).num_milliseconds()),
            Self::Cancelled {
                started_at,
                cancelled_at,
            } => Some((*cancelled_at - *started_at).num_milliseconds()),
            Self::Failed { started_at, .. } => Some((Utc::now() - started_at).num_milliseconds()),
        }
    }
}

/// A record in the query history store.
#[derive(Debug, Clone)]
pub struct QueryRecord {
    pub id: QueryId,
    pub sql: String,
    pub classification: QueryClassification,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub row_count: Option<u64>,
    pub error: Option<String>,
    pub database: String,
}
