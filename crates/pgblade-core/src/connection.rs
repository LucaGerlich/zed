use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ConnectionError;

/// SSH authentication method for tunnel connections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SshAuth {
    /// Use the system SSH agent (ssh-agent).
    #[default]
    Agent,
    /// Use a private key file at the given path.
    KeyFile { path: String },
}

/// Configuration for an SSH tunnel used to reach the database host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 22,
            username: String::new(),
            auth: SshAuth::default(),
        }
    }
}

/// Opaque identifier for a saved connection profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub Uuid);

impl ConnectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Declares the environment risk level of a connection.
/// Drives safety classification and UI indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    Local,
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Development => "Development",
            Self::Staging => "Staging",
            Self::Production => "Production",
        }
    }
}

/// SSL connection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
}

/// How credentials are managed for this connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialStorage {
    /// Persisted in OS keychain under a service key derived from the connection ID.
    Keychain,
    /// Held in memory for the session only; discarded on close.
    Temporary,
}

/// A saved connection profile. Secrets are never stored here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: ConnectionId,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub environment: Environment,
    pub ssl_mode: SslMode,
    /// Whether this profile defaults to read-only mode when connected.
    pub read_only_default: bool,
    /// SSH tunnel configuration for reaching remote databases.
    #[serde(default)]
    pub ssh: SshConfig,
}

impl ConnectionProfile {
    /// Service key used for keychain credential storage.
    pub fn keychain_service_key(&self) -> String {
        format!("pgblade:{}", self.id.0)
    }
}

/// State machine for a database connection lifecycle.
/// Each variant carries only the data valid for that state.
#[derive(Debug, Clone)]
pub enum ConnectionState {
    Disconnected,

    Connecting {
        profile_id: ConnectionId,
        attempt: u32,
    },

    Connected {
        profile_id: ConnectionId,
        session_id: Uuid,
        server_version: String,
        database: String,
        read_only: bool,
    },

    Failed {
        profile_id: ConnectionId,
        error: ConnectionError,
        retries: u32,
    },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected)
    }

    pub fn profile_id(&self) -> Option<ConnectionId> {
        match self {
            Self::Disconnected => None,
            Self::Connecting { profile_id, .. }
            | Self::Connected { profile_id, .. }
            | Self::Failed { profile_id, .. } => Some(*profile_id),
        }
    }
}
