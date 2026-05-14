//! File-based secret store.
//!
//! Despite the module name (kept for git-history continuity), this no longer
//! talks to the OS keychain. Instead we persist secrets to a TOML file inside
//! the application data directory (`secret.toml`), with restrictive file
//! permissions (chmod 0600 on Unix; NTFS user-profile defaults on Windows).
//!
//! The motivation: macOS Keychain prompts every fresh dev build for permission,
//! which is noisy and annoying. The app data directory is already user-private,
//! so storing secrets there with `0600` mode gives equivalent practical
//! protection without the GUI prompts.
//!
//! Exposes the same low-level `set` / `get` / `delete` API plus higher-level
//! helpers for the Jira API token. **All functions now require an
//! `app_data_dir` parameter** so the caller decides where the secret file
//! lives — production callers should pass `app.path().app_data_dir()`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Reverse-DNS service identifier. Retained as a stable map-key prefix for
/// backward compatibility; no longer corresponds to a real OS keychain entry.
pub const KEYCHAIN_SERVICE: &str = "com.tracker.app";

/// Account name (key) under which the Jira API token is stored.
pub const KEY_JIRA_TOKEN: &str = "jira-api-token";

/// Errors that can occur while interacting with the on-disk secret store.
#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("toml serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

/// On-disk schema for the secret file. Keys are `"service:account"` so multiple
/// secrets can coexist; today we only ever store the Jira token but the format
/// is forward-compatible.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SecretFile {
    #[serde(default)]
    secrets: BTreeMap<String, String>,
}

/// Resolve the secret-file path inside `app_data_dir`.
fn secret_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("secret.toml")
}

fn load_file(app_data_dir: &Path) -> Result<SecretFile, KeychainError> {
    let p = secret_path(app_data_dir);
    if !p.exists() {
        return Ok(SecretFile::default());
    }
    let text = fs::read_to_string(&p)?;
    if text.trim().is_empty() {
        return Ok(SecretFile::default());
    }
    Ok(toml::from_str(&text)?)
}

fn save_file(app_data_dir: &Path, file: &SecretFile) -> Result<(), KeychainError> {
    let p = secret_path(app_data_dir);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string(file)?;
    // Write atomically: tmp file -> chmod -> rename.
    let tmp = p.with_extension("toml.tmp");
    fs::write(&tmp, text)?;
    set_restrictive_permissions(&tmp)?;
    fs::rename(&tmp, &p)?;
    // Re-apply perms on the final path in case the platform lost them through
    // the rename (cheap and idempotent).
    set_restrictive_permissions(&p)?;
    Ok(())
}

#[cfg(unix)]
fn set_restrictive_permissions(p: &Path) -> Result<(), KeychainError> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(p, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_restrictive_permissions(_p: &Path) -> Result<(), KeychainError> {
    // Windows: NTFS ACLs on user-profile paths default to user-only access,
    // which gives equivalent practical protection. No extra hardening here.
    Ok(())
}

// -----------------------------------------------------------------------------
// Low-level API
// -----------------------------------------------------------------------------

/// Store `secret` under (`service`, `account`). Overwrites any existing entry.
pub fn set(
    app_data_dir: &Path,
    service: &str,
    account: &str,
    secret: &str,
) -> Result<(), KeychainError> {
    let mut f = load_file(app_data_dir)?;
    f.secrets
        .insert(format!("{service}:{account}"), secret.to_owned());
    save_file(app_data_dir, &f)
}

/// Fetch the secret stored under (`service`, `account`).
///
/// Returns `Ok(None)` if no entry exists, `Ok(Some(_))` on success.
pub fn get(
    app_data_dir: &Path,
    service: &str,
    account: &str,
) -> Result<Option<String>, KeychainError> {
    let f = load_file(app_data_dir)?;
    Ok(f.secrets.get(&format!("{service}:{account}")).cloned())
}

/// Delete the entry under (`service`, `account`). Idempotent: missing entries
/// are treated as success.
pub fn delete(
    app_data_dir: &Path,
    service: &str,
    account: &str,
) -> Result<(), KeychainError> {
    let mut f = load_file(app_data_dir)?;
    f.secrets.remove(&format!("{service}:{account}"));
    save_file(app_data_dir, &f)
}

// -----------------------------------------------------------------------------
// High-level helpers for the Jira API token
// -----------------------------------------------------------------------------

/// Persist the Jira API token to the secret file.
pub fn save_jira_token(app_data_dir: &Path, token: &str) -> Result<(), KeychainError> {
    set(app_data_dir, KEYCHAIN_SERVICE, KEY_JIRA_TOKEN, token)
}

/// Load the Jira API token from the secret file, if any.
pub fn load_jira_token(app_data_dir: &Path) -> Result<Option<String>, KeychainError> {
    get(app_data_dir, KEYCHAIN_SERVICE, KEY_JIRA_TOKEN)
}

/// Remove the Jira API token from the secret file (idempotent).
pub fn clear_jira_token(app_data_dir: &Path) -> Result<(), KeychainError> {
    delete(app_data_dir, KEYCHAIN_SERVICE, KEY_JIRA_TOKEN)
}
