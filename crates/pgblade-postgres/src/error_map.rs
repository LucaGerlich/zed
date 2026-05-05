use pgblade_core::error::{ConnectionError, QueryError};

/// Map a tokio-postgres error that occurred during query execution
/// into our domain QueryError type.
pub fn map_query_error(e: tokio_postgres::Error) -> QueryError {
    if let Some(db_err) = e.as_db_error() {
        QueryError::Postgres {
            code: db_err.code().code().to_string(),
            severity: db_err.severity().to_string(),
            message: db_err.message().to_string(),
            detail: db_err.detail().map(|s| s.to_string()),
            hint: db_err.hint().map(|s| s.to_string()),
            position: db_err.position().and_then(|p| match p {
                tokio_postgres::error::ErrorPosition::Original(pos) => Some(*pos),
                tokio_postgres::error::ErrorPosition::Internal { .. } => None,
            }),
        }
    } else if e.is_closed() {
        QueryError::NotConnected
    } else {
        QueryError::Other {
            message: e.to_string(),
        }
    }
}

/// Map a tokio-postgres error that occurred during connection
/// into our domain ConnectionError type.
///
/// Uses heuristics on the error message to classify into specific
/// error variants that drive different UI guidance.
pub fn map_connection_error(
    e: tokio_postgres::Error,
    profile: &pgblade_core::connection::ConnectionProfile,
) -> ConnectionError {
    // If Postgres sent a structured DB error during connection (e.g., "database does not exist"),
    // extract it directly from the SQLSTATE code.
    if let Some(db_err) = e.as_db_error() {
        let code = db_err.code().code();
        return match code {
            // 28P01 = invalid_password, 28000 = invalid_authorization_specification
            "28P01" | "28000" => ConnectionError::Authentication {
                user: profile.username.clone(),
            },
            // 3D000 = invalid_catalog_name (database does not exist)
            "3D000" => ConnectionError::DatabaseNotFound {
                database: profile.database.clone(),
            },
            _ => ConnectionError::Other {
                message: db_err.message().to_string(),
            },
        };
    }

    // Fall back to string-matching on the raw error message.
    let msg = e.to_string();
    let lower = msg.to_lowercase();

    if lower.contains("could not translate host name")
        || lower.contains("name or service not known")
        || lower.contains("nodename nor servname provided")
        || lower.contains("no address associated")
    {
        ConnectionError::HostResolution {
            host: profile.host.clone(),
        }
    } else if lower.contains("password authentication failed")
        || lower.contains("no pg_hba.conf entry")
        || lower.contains("authentication failed")
    {
        ConnectionError::Authentication {
            user: profile.username.clone(),
        }
    } else if lower.contains("ssl") || lower.contains("tls") {
        ConnectionError::Ssl {
            reason: msg.clone(),
        }
    } else if lower.contains("connection refused")
        || lower.contains("could not connect to server")
        || lower.contains("no route to host")
    {
        ConnectionError::Refused { reason: msg }
    } else if lower.contains("timeout") || lower.contains("timed out") {
        ConnectionError::Timeout { timeout_secs: 0 }
    } else {
        ConnectionError::Other { message: msg }
    }
}
