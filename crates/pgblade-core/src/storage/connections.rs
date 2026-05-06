use rusqlite::params;

use crate::connection::{ConnectionId, ConnectionProfile, Environment, SshConfig, SslMode};

use super::{StorageError, StorageManager};

impl StorageManager {
    /// Check if a connection with the same host, port, database, and username already exists.
    pub fn find_duplicate(
        &self,
        profile: &ConnectionProfile,
    ) -> Result<Option<ConnectionId>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM connections WHERE host = ?1 AND port = ?2 AND database_name = ?3 AND username = ?4",
        )?;

        let result = stmt.query_row(
            params![
                profile.host,
                profile.port as i64,
                profile.database,
                profile.username
            ],
            |row| {
                let id_str: String = row.get(0)?;
                Ok(ConnectionId(
                    uuid::Uuid::parse_str(&id_str).unwrap_or_else(|_| uuid::Uuid::new_v4()),
                ))
            },
        );

        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// Persists a connection profile, replacing any existing entry with the same ID.
    pub fn save_connection(&self, profile: &ConnectionProfile) -> Result<(), StorageError> {
        let now = chrono::Utc::now().to_rfc3339();
        let ssh_json = serde_json::to_string(&profile.ssh).unwrap_or_else(|_| "{}".to_string());
        self.conn.execute(
            "INSERT OR REPLACE INTO connections
                (id, name, host, port, database_name, username, environment, ssl_mode, read_only_default, ssh_config, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                profile.id.0.to_string(),
                profile.name,
                profile.host,
                profile.port as i64,
                profile.database,
                profile.username,
                format!("{:?}", profile.environment),
                format!("{:?}", profile.ssl_mode),
                profile.read_only_default as i64,
                ssh_json,
                now,
                now,
            ],
        )?;
        Ok(())
    }

    /// Loads all saved connection profiles ordered by name.
    pub fn load_connections(&self) -> Result<Vec<ConnectionProfile>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, host, port, database_name, username, environment, ssl_mode, read_only_default, ssh_config
             FROM connections
             ORDER BY name",
        )?;

        let profiles = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                let host: String = row.get(2)?;
                let port: i64 = row.get(3)?;
                let database: String = row.get(4)?;
                let username: String = row.get(5)?;
                let env_str: String = row.get(6)?;
                let ssl_str: String = row.get(7)?;
                let read_only: i64 = row.get(8)?;
                let ssh_json: String = row.get::<_, String>(9).unwrap_or_else(|_| "{}".to_string());

                let ssh: SshConfig = serde_json::from_str(&ssh_json).unwrap_or_default();

                Ok(ConnectionProfile {
                    id: ConnectionId(
                        uuid::Uuid::parse_str(&id_str).unwrap_or_else(|_| uuid::Uuid::new_v4()),
                    ),
                    name,
                    host,
                    port: port as u16,
                    database,
                    username,
                    environment: parse_environment(&env_str),
                    ssl_mode: parse_ssl_mode(&ssl_str),
                    read_only_default: read_only != 0,
                    ssh,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }

    /// Deletes a connection profile by its ID.
    pub fn delete_connection(&self, id: &ConnectionId) -> Result<(), StorageError> {
        let affected = self.conn.execute(
            "DELETE FROM connections WHERE id = ?1",
            params![id.0.to_string()],
        )?;

        if affected == 0 {
            return Err(StorageError::NotFound);
        }

        Ok(())
    }
}

/// Parses an environment string back into the `Environment` enum.
fn parse_environment(s: &str) -> Environment {
    match s {
        "Local" => Environment::Local,
        "Development" => Environment::Development,
        "Staging" => Environment::Staging,
        "Production" => Environment::Production,
        _ => Environment::Development,
    }
}

/// Parses an SSL mode string back into the `SslMode` enum.
fn parse_ssl_mode(s: &str) -> SslMode {
    match s {
        "Disable" => SslMode::Disable,
        "Prefer" => SslMode::Prefer,
        "Require" => SslMode::Require,
        _ => SslMode::Prefer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profile() -> ConnectionProfile {
        ConnectionProfile {
            id: ConnectionId::new(),
            name: "Test DB".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "postgres".to_string(),
            environment: Environment::Development,
            ssl_mode: SslMode::Prefer,
            read_only_default: false,
            ssh: SshConfig::default(),
        }
    }

    #[test]
    fn saves_and_loads_connection() {
        let storage = StorageManager::in_memory().unwrap();
        let profile = test_profile();

        storage.save_connection(&profile).unwrap();
        let loaded = storage.load_connections().unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Test DB");
        assert_eq!(loaded[0].host, "localhost");
        assert_eq!(loaded[0].port, 5432);
        assert_eq!(loaded[0].environment, Environment::Development);
        assert_eq!(loaded[0].ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn deletes_connection() {
        let storage = StorageManager::in_memory().unwrap();
        let profile = test_profile();
        let id = profile.id;

        storage.save_connection(&profile).unwrap();
        storage.delete_connection(&id).unwrap();

        let loaded = storage.load_connections().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let storage = StorageManager::in_memory().unwrap();
        let result = storage.delete_connection(&ConnectionId::new());
        assert!(result.is_err());
    }

    #[test]
    fn parses_environment_variants() {
        assert_eq!(parse_environment("Local"), Environment::Local);
        assert_eq!(parse_environment("Production"), Environment::Production);
        assert_eq!(parse_environment("unknown"), Environment::Development);
    }

    #[test]
    fn parses_ssl_mode_variants() {
        assert_eq!(parse_ssl_mode("Disable"), SslMode::Disable);
        assert_eq!(parse_ssl_mode("Require"), SslMode::Require);
        assert_eq!(parse_ssl_mode("unknown"), SslMode::Prefer);
    }

    #[test]
    fn find_duplicate_returns_existing_id() {
        let storage = StorageManager::in_memory().unwrap();
        let profile = test_profile();
        storage.save_connection(&profile).unwrap();

        let result = storage.find_duplicate(&profile).unwrap();
        assert_eq!(result, Some(profile.id));
    }

    #[test]
    fn find_duplicate_returns_none_when_no_match() {
        let storage = StorageManager::in_memory().unwrap();
        let profile = test_profile();

        let result = storage.find_duplicate(&profile).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn find_duplicate_ignores_different_port() {
        let storage = StorageManager::in_memory().unwrap();
        let profile = test_profile();
        storage.save_connection(&profile).unwrap();

        let mut different = test_profile();
        different.port = 5433;
        let result = storage.find_duplicate(&different).unwrap();
        assert_eq!(result, None);
    }
}
