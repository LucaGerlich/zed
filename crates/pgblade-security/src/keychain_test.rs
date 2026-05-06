use pgblade_core::security::CredentialStore;
use crate::keychain::KeychainStore;

#[test]
fn test_keychain_store() {
    let store = KeychainStore::new();
    let key = "test-key-via-store";
    let password = "test-password";

    store.store(key, password).expect("store failed");
    
    let retrieved = store.retrieve(key).expect("retrieve failed");
    assert_eq!(retrieved, Some(password.to_string()));

    store.delete(key).expect("delete failed");
    assert_eq!(store.retrieve(key).unwrap(), None);
}
