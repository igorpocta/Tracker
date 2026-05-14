//! On-disk configuration for the Tracker app.
//!
//! The Jira API token is intentionally **not** stored here — it lives in the
//! OS keychain and is retrieved through [`crate::keychain::load_jira_token`]
//! at use sites.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// User-supplied Jira configuration (everything except the secret token).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    /// Base URL of the Jira Cloud instance, e.g. `https://acme.atlassian.net`.
    pub base_url: String,
    /// Account email used together with the API token for basic auth.
    pub email: String,
}

/// Errors that can occur while loading or saving the config file.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("toml deserialize: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("toml serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

/// Read and parse a [`JiraConfig`] from a TOML file at `path`.
pub fn load_from_path(path: &Path) -> Result<JiraConfig, ConfigError> {
    let text = fs::read_to_string(path)?;
    let cfg = toml::from_str(&text)?;
    Ok(cfg)
}

/// Serialise `cfg` to TOML and write it to `path`, creating parent directories
/// as needed.
pub fn save_to_path(path: &Path, cfg: &JiraConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string(cfg)?;
    fs::write(path, text)?;
    Ok(())
}
