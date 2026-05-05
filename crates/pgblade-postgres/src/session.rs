use std::pin::pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use tokio::runtime::Runtime;
use tokio_postgres::{CancelToken, Client, NoTls};

use pgblade_core::driver::DatabaseSession;
use pgblade_core::error::QueryError;
use pgblade_core::result::ResultSet;

use crate::error_map::map_query_error;
use crate::row_convert::{convert_row, extract_columns};

const DEFAULT_PAGE_SIZE: usize = 500;

/// A live PostgreSQL session wrapping a tokio-postgres Client.
pub struct PostgresSession {
    client: Client,
    cancel_token: CancelToken,
    runtime: Arc<Runtime>,
    server_version: String,
    _read_only: bool,
}

impl PostgresSession {
    pub fn new(
        client: Client,
        cancel_token: CancelToken,
        runtime: Arc<Runtime>,
        server_version: String,
        read_only: bool,
    ) -> Self {
        Self {
            client,
            cancel_token,
            runtime,
            server_version,
            _read_only: read_only,
        }
    }
}

#[async_trait]
impl DatabaseSession for PostgresSession {
    async fn execute(&self, sql: &str) -> Result<ResultSet, QueryError> {
        // Prepare the statement to get column metadata
        let stmt = self.client.prepare(sql).await.map_err(map_query_error)?;

        let columns = extract_columns(stmt.columns());

        // Execute and stream results
        let params: Vec<String> = vec![];
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params.iter().map(|p| p as _).collect();

        let stream = self
            .client
            .query_raw(&stmt, param_refs)
            .await
            .map_err(map_query_error)?;

        let mut stream = pin!(stream);

        // Collect rows up to a reasonable limit
        let max_rows = DEFAULT_PAGE_SIZE * 10; // 5000 rows max for M2
        let mut rows = Vec::new();

        while let Some(result) = stream.next().await {
            let row = result.map_err(map_query_error)?;
            rows.push(convert_row(&row));

            if rows.len() >= max_rows {
                tracing::warn!(max_rows, "result set truncated at {} rows", rows.len());
                break;
            }
        }

        let total_rows = rows.len() as u64;

        Ok(ResultSet {
            columns,
            rows,
            total_rows,
        })
    }

    async fn cancel(&self) -> Result<(), QueryError> {
        // Cancel must run on the tokio runtime because it opens
        // a new TCP connection to send the cancel signal.
        let token = self.cancel_token.clone();
        self.runtime
            .spawn(async move { token.cancel_query(NoTls).await })
            .await
            .map_err(|e| QueryError::Other {
                message: format!("runtime error during cancel: {e}"),
            })?
            .map_err(|e| QueryError::Other {
                message: format!("cancel failed: {e}"),
            })?;

        Ok(())
    }

    fn server_version(&self) -> &str {
        &self.server_version
    }

    async fn list_schemas(&self) -> Result<Vec<pgblade_core::schema::SchemaInfo>, QueryError> {
        crate::introspection::fetch_schemas(&self.client).await
    }

    async fn list_tables(
        &self,
        schema: &str,
    ) -> Result<Vec<pgblade_core::schema::TableInfo>, QueryError> {
        crate::introspection::fetch_tables(&self.client, schema).await
    }

    async fn list_columns(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<pgblade_core::schema::ColumnInfo>, QueryError> {
        crate::introspection::fetch_columns(&self.client, schema, table).await
    }

    async fn list_constraints(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<pgblade_core::schema::ConstraintInfo>, QueryError> {
        crate::introspection::fetch_constraints(&self.client, schema, table).await
    }

    async fn list_foreign_keys(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<pgblade_core::schema::ForeignKeyInfo>, QueryError> {
        crate::introspection::fetch_foreign_keys(&self.client, schema, table).await
    }

    async fn list_indexes(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<pgblade_core::schema::IndexInfo>, QueryError> {
        crate::introspection::fetch_indexes(&self.client, schema, table).await
    }

    async fn list_triggers(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<pgblade_core::schema::TriggerInfo>, QueryError> {
        crate::introspection::fetch_triggers(&self.client, schema, table).await
    }

    async fn list_functions(
        &self,
        schema: &str,
    ) -> Result<Vec<pgblade_core::schema::FunctionInfo>, QueryError> {
        crate::introspection::fetch_functions(&self.client, schema).await
    }

    async fn list_sequences(
        &self,
        schema: &str,
    ) -> Result<Vec<pgblade_core::schema::SequenceInfo>, QueryError> {
        crate::introspection::fetch_sequences(&self.client, schema).await
    }
}
