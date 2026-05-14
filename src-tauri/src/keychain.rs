//! Thin wrapper around the OS keychain (via the `keyring` crate).
//!
//! Exposes a low-level `set` / `get` / `delete` API plus higher-level helpers
//! for the Jira API token used by the rest of the application.

use keyring::Entry;
use thiserror::Error;

/// Reverse-DNS service identifier used for every secret this app stores.
pub const KEYCHAIN_SERVICE: &str = "com.tracker.app";

/// Account name (key) under which the Jira API token is stored.
pub const KEY_JIRA_TOKEN: &str = "jira-api-token";

/// Errors that can occur while interacting with the OS keychain.
#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keyring: {0}")]
    Keyring(#[from] keyring::Error),
}

// -----------------------------------------------------------------------------
// Low-level API
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// High-level helpers for the Jira API token
// -----------------------------------------------------------------------------

/// Persist the Jira API token to the OS keychain.
pub fn save_jira_token(token: &str) -> Result<(), KeychainError> {
    set(KEYCHAIN_SERVICE, KEY_JIRA_TOKEN, token)
}

/// Load the Jira API token from the OS keychain, if any.
pub fn load_jira_token() -> Result<Option<String>, KeychainError> {
    get(KEYCHAIN_SERVICE, KEY_JIRA_TOKEN)
}

/// Remove the Jira API token from the OS keychain (idempotent).
pub fn clear_jira_token() -> Result<(), KeychainError> {
    delete(KEYCHAIN_SERVICE, KEY_JIRA_TOKEN)
}
