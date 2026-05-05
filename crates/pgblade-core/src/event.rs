use crate::connection::{ConnectionProfile, ConnectionState};
use crate::query::{QueryClassification, QueryId, QueryRecord, QueryState};
use crate::result::ResultPage;
use crate::safety::WritePolicy;
use crate::schema::SchemaTree;

/// Application-wide events that flow between the controller and the UI.
///
/// The AppController emits these events. UI views subscribe and re-render
/// in response. This is the single communication channel from backend to UI.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Connection state transitioned (connecting, connected, failed, disconnected).
    ConnectionStateChanged(ConnectionState),

    /// A query's execution state changed.
    QueryStateChanged {
        query_id: QueryId,
        state: QueryState,
    },

    /// A new page of results is ready for display.
    ResultPageReady { query_id: QueryId, page: ResultPage },

    /// The active write policy changed (e.g., user toggled read-only).
    WritePolicyChanged(WritePolicy),

    /// Write confirmation needed — workspace should show confirmation dialog.
    WriteConfirmationNeeded {
        sql: String,
        classification: QueryClassification,
    },

    /// Schema introspection completed.
    SchemaLoaded(SchemaTree),

    /// Saved connections loaded from storage.
    SavedConnectionsLoaded(Vec<ConnectionProfile>),

    /// Query history loaded.
    HistoryLoaded(Vec<QueryRecord>),
}
