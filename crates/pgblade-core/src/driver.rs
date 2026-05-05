use async_trait::async_trait;

use crate::connection::ConnectionProfile;
use crate::error::{ConnectionError, QueryError};
use crate::result::ResultSet;
use crate::schema::{
    ColumnInfo, ConstraintInfo, ForeignKeyInfo, FunctionInfo, IndexInfo, SchemaInfo, SequenceInfo,
    TableInfo, TriggerInfo,
};

/// Trait boundary for database drivers.
///
/// PgBlade ships only a Postgres driver in v1, but this boundary keeps
/// future drivers (SQLite, MySQL) possible without an external plugin system.
///
/// The application controller depends on this trait, not concrete Postgres types.
#[async_trait]
pub trait DatabaseDriver: Send + Sync + 'static {
    /// Establish a connection to the database.
    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: &str,
    ) -> Result<Box<dyn DatabaseSession>, ConnectionError>;
}

/// A live session with a database.
///
/// Owns the connection resources and provides query execution.
/// Dropping the session closes the underlying connection.
#[async_trait]
pub trait DatabaseSession: Send + Sync {
    /// Execute a SQL statement and return the full result set.
    async fn execute(&self, sql: &str) -> Result<ResultSet, QueryError>;

    /// Cancel any currently running query on this session.
    async fn cancel(&self) -> Result<(), QueryError>;

    /// The server version string (e.g., "PostgreSQL 16.2").
    fn server_version(&self) -> &str;

    /// List all user-visible schemas.
    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>, QueryError>;

    /// List tables and views in a schema.
    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>, QueryError>;

    /// List columns for a table.
    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>, QueryError>;

    /// List constraints for a table.
    async fn list_constraints(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ConstraintInfo>, QueryError>;

    /// List foreign keys for a table.
    async fn list_foreign_keys(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ForeignKeyInfo>, QueryError>;

    /// List indexes for a table.
    async fn list_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>, QueryError>;

    /// List triggers for a table.
    async fn list_triggers(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<TriggerInfo>, QueryError>;

    /// List functions and procedures in a schema.
    async fn list_functions(&self, schema: &str) -> Result<Vec<FunctionInfo>, QueryError>;

    /// List sequences in a schema.
    async fn list_sequences(&self, schema: &str) -> Result<Vec<SequenceInfo>, QueryError>;
}
