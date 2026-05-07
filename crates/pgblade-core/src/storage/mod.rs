pub mod connections;
pub mod history;

use rusqlite::Connection;

/// Errors originating from the local storage layer.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("record not found")]
    NotFound,
}

/// Manages the local SQLite database used for persisting connection
/// profiles, query history, and application state.
pub struct StorageManager {
    conn: Connection,
}

impl StorageManager {
    /// Initializes the storage manager with the default platform data directory.
    ///
    /// Creates the directory and database file if they do not exist,
    /// then runs migrations to ensure the schema is up to date.
    pub fn init() -> Result<Self, StorageError> {
        let path = Self::db_path()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        let manager = Self { conn };
        manager.run_migrations()?;
        Ok(manager)
    }

    /// Creates an in-memory storage manager suitable for tests.
    pub fn in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let manager = Self { conn };
        manager.run_migrations()?;
        Ok(manager)
    }

    /// Returns a reference to the underlying SQLite connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Resolves the default database file path using platform conventions.
    fn db_path() -> Result<std::path::PathBuf, StorageError> {
        let data_dir = dirs::data_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not determine platform data directory",
            )
        })?;
        Ok(data_dir.join("pgblade").join("pgblade.db"))
    }

    /// Runs schema migrations to create or update tables.
    fn run_migrations(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS connections (
                id              TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                host            TEXT NOT NULL,
                port            INTEGER NOT NULL,
                database_name   TEXT NOT NULL,
                username        TEXT NOT NULL,
                environment     TEXT NOT NULL,
                ssl_mode        TEXT NOT NULL,
                read_only_default INTEGER NOT NULL DEFAULT 0,
                created_at      TEXT NOT NULL,
                updated_at      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS query_history (
                id              TEXT PRIMARY KEY,
                connection_id   TEXT NOT NULL,
                sql_text        TEXT NOT NULL,
                classification  TEXT NOT NULL,
                executed_at     TEXT NOT NULL,
                duration_ms     INTEGER,
                row_count       INTEGER,
                error           TEXT,
                database_name   TEXT NOT NULL
            );
            ",
        )?;

        // Migration: add ssh_config column (idempotent — silently ignored if exists)
        let _ = self.conn.execute(
            "ALTER TABLE connections ADD COLUMN ssh_config TEXT DEFAULT '{}'",
            [],
        );

        // Migration: add password column as fallback for keychain
        let _ = self.conn.execute(
            "ALTER TABLE connections ADD COLUMN password TEXT DEFAULT ''",
            [],
        );

        // Migration: bookmarks table
        let _ = self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                sql_text TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        );

        Ok(())
    }

    /// Save a SQL query as a named bookmark.
    pub fn save_bookmark(&self, name: &str, sql: &str) -> Result<(), StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO bookmarks (id, name, sql_text, created_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, name, sql, now],
        )?;
        Ok(())
    }

    /// Load all bookmarks ordered by most recent first.
    pub fn load_bookmarks(&self) -> Result<Vec<(String, String, String, String)>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, sql_text, created_at FROM bookmarks ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    /// Delete a bookmark by its ID.
    pub fn delete_bookmark(&self, id: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM bookmarks WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    /// Store a password for a connection (base64 encoded, not plaintext).
    pub fn save_password(
        &self,
        id: &crate::connection::ConnectionId,
        password: &str,
    ) -> Result<(), StorageError> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(password);
        self.conn.execute(
            "UPDATE connections SET password = ?1 WHERE id = ?2",
            rusqlite::params![encoded, id.0.to_string()],
        )?;
        Ok(())
    }

    /// Retrieve a stored password for a connection (base64 decoded).
    pub fn load_password(
        &self,
        id: &crate::connection::ConnectionId,
    ) -> Result<Option<String>, StorageError> {
        use base64::Engine;
        let result = self.conn.query_row(
            "SELECT password FROM connections WHERE id = ?1",
            rusqlite::params![id.0.to_string()],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(encoded) if !encoded.is_empty() => {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(&encoded)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok());
                Ok(decoded)
            }
            Ok(_) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }
}
