use pgblade_core::security::{CredentialError, CredentialStore};

const SERVICE_NAME: &str = "pgblade";

/// macOS Keychain-backed credential store using the `keyring` crate.
pub struct KeychainStore;

impl KeychainStore {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KeychainStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for KeychainStore {
    fn store(&self, service_key: &str, password: &str) -> Result<(), CredentialError> {
        let entry =
            keyring::Entry::new(SERVICE_NAME, service_key).map_err(|e| CredentialError::Other {
                message: e.to_string(),
            })?;
        entry.set_password(password).map_err(|e| match e {
            keyring::Error::NoEntry => CredentialError::NotFound {
                key: service_key.to_string(),
            },
            keyring::Error::Ambiguous(_) => CredentialError::Other {
                message: e.to_string(),
            },
            _ => CredentialError::Other {
                message: e.to_string(),
            },
        })
    }

    fn retrieve(&self, service_key: &str) -> Result<Option<String>, CredentialError> {
        let entry =
            keyring::Entry::new(SERVICE_NAME, service_key).map_err(|e| CredentialError::Other {
                message: e.to_string(),
            })?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(CredentialError::Other {
                message: e.to_string(),
            }),
        }
    }

    fn delete(&self, service_key: &str) -> Result<(), CredentialError> {
        let entry =
            keyring::Entry::new(SERVICE_NAME, service_key).map_err(|e| CredentialError::Other {
                message: e.to_string(),
            })?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted, that's fine
            Err(e) => Err(CredentialError::Other {
                message: e.to_string(),
            }),
        }
    }
}
