#[derive(Debug, Clone, thiserror::Error)]
pub enum CredentialError {
    #[error("keychain access denied")]
    AccessDenied,

    #[error("credential not found for key '{key}'")]
    NotFound { key: String },

    #[error("keychain error: {message}")]
    Other { message: String },
}

/// Trait for secure credential storage.
///
/// The keychain-backed implementation lives in `pgblade-security`.
/// Tests can use an in-memory mock.
pub trait CredentialStore: Send + Sync {
    fn store(&self, service_key: &str, password: &str) -> Result<(), CredentialError>;
    fn retrieve(&self, service_key: &str) -> Result<Option<String>, CredentialError>;
    fn delete(&self, service_key: &str) -> Result<(), CredentialError>;
}
