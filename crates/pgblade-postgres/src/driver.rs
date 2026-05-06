use std::sync::Arc;

use async_trait::async_trait;
use tokio::runtime::Runtime;
use tokio_postgres::NoTls;

use pgblade_core::connection::{ConnectionProfile, SslMode};
use pgblade_core::driver::DatabaseDriver;
use pgblade_core::error::ConnectionError;

use crate::error_map::map_connection_error;
use crate::session::PostgresSession;

/// PostgreSQL driver implementation.
///
/// Holds a reference to the tokio runtime where the background
/// connection task runs.
#[derive(Clone)]
pub struct PostgresDriver {
    runtime: Arc<Runtime>,
}

impl PostgresDriver {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    /// Build a connection string from a ConnectionProfile.
    ///
    /// Values are single-quoted and any embedded single quotes are escaped
    /// with a backslash to prevent injection.
    fn build_connection_string(profile: &ConnectionProfile, password: &str) -> String {
        let ssl = match profile.ssl_mode {
            SslMode::Disable => "disable",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
        };
        format!(
            "host='{}' port={} dbname='{}' user='{}' password='{}' sslmode={}",
            profile.host.replace('\'', "\\'"),
            profile.port,
            profile.database.replace('\'', "\\'"),
            profile.username.replace('\'', "\\'"),
            password.replace('\'', "\\'"),
            ssl,
        )
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn connect(
        &self,
        profile: &ConnectionProfile,
        password: &str,
    ) -> Result<Box<dyn pgblade_core::driver::DatabaseSession>, ConnectionError> {
        let conn_string = Self::build_connection_string(profile, password);
        let read_only = profile.read_only_default;
        let runtime = self.runtime.clone();

        // Connect to Postgres — this must run on the tokio runtime
        // because it performs TCP I/O.
        let (client, connection) = runtime
            .spawn(async move { tokio_postgres::connect(&conn_string, NoTls).await })
            .await
            .map_err(|e| ConnectionError::Other {
                message: format!("runtime error: {e}"),
            })?
            .map_err(|e| map_connection_error(e, profile))?;

        // Spawn the background connection task on the tokio runtime.
        // This task handles all socket I/O for this connection.
        runtime.spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!("postgres connection task error: {e}");
            }
        });

        // Get server version
        let server_version = client
            .query_one("SELECT version()", &[])
            .await
            .map(|row| row.get::<_, String>(0))
            .unwrap_or_else(|_| "unknown".to_string());

        // Set read-only mode if profile requires it
        if read_only {
            client
                .batch_execute("SET default_transaction_read_only = on")
                .await
                .map_err(|e| ConnectionError::Other {
                    message: format!("failed to set read-only mode: {e}"),
                })?;
        }

        let cancel_token = client.cancel_token();

        let session =
            PostgresSession::new(client, cancel_token, runtime, server_version, read_only);

        Ok(Box::new(session))
    }
}
