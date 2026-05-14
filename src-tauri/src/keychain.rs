//! Thin wrapper around the OS keychain (via the `keyring` crate).

use keyring::Entry;
use thiserror::Error;

/// Errors that can occur while interacting with the OS keychain.
#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keyring: {0}")]
    Keyring(#[from] keyring::Error),
}

/// Store `secret` under (`service`, `account`). Overwrites any existing entry.
pub fn set(service: &str, account: &str, secret: &str) -> Result<(), KeychainError> {
    Entry::new(service, account)?.set_password(secret)?;
    Ok(())
}

/// Fetch the secret stored under (`service`, `account`).
///
/// Returns `Ok(None)` if no entry exists, `Ok(Some(_))` on success, and
/// `Err(_)` for any other backend failure.
pub fn get(service: &str, account: &str) -> Result<Option<String>, KeychainError> {
    match Entry::new(service, account)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Delete the entry under (`service`, `account`). Idempotent: missing entries
/// are treated as success.
pub fn delete(service: &str, account: &str) -> Result<(), KeychainError> {
    match Entry::new(service, account)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
