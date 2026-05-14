//! Application-wide shared state for Tauri commands.
//!
//! Holds the open SQLite database connection pool plus the currently loaded
//! [`JiraConfig`] / [`JiraClient`]. Both are wrapped in `RwLock` because we
//! rebuild the client when the user updates credentials, but we do not want to
//! make `Db` itself locked (it's already a connection pool).
//!
//! Since Phase 17 the on-disk secret store lives at `app_data_dir/secret.toml`,
//! so [`AppState`] also carries `app_data_dir` to give command handlers a
//! single place to read it from without re-querying the [`tauri::AppHandle`].

use std::path::PathBuf;
use std::sync::RwLock;

use crate::cache::Db;
use crate::config::JiraConfig;
use crate::jira::JiraClient;

/// Tauri-managed state shared across all command invocations.
pub struct AppState {
    /// Local SQLite cache (issues, worklogs, settings, active timer).
    pub db: Db,
    /// Application data directory — used to resolve the on-disk secret file
    /// (`secret.toml`) and any other per-install assets.
    pub app_data_dir: PathBuf,
    /// Last-known Jira configuration loaded from disk, if any.
    pub jira_config: RwLock<Option<JiraConfig>>,
    /// Live HTTP client. Present iff both config and the on-disk token resolved.
    pub jira_client: RwLock<Option<JiraClient>>,
}

impl AppState {
    /// Create a new [`AppState`] wrapping the given DB and remembering the
    /// `app_data_dir` (used to locate `secret.toml`). The Jira client is
    /// initially unset; call [`AppState::try_build_client`] after loading
    /// config to materialise it.
    pub fn new(db: Db, app_data_dir: PathBuf) -> Self {
        Self {
            db,
            app_data_dir,
            jira_config: RwLock::new(None),
            jira_client: RwLock::new(None),
        }
    }

    /// Try to build a fresh [`JiraClient`] from the current `jira_config`
    /// and the on-disk Jira API token.
    ///
    /// Returns `Ok(true)` if a client was built and stored, `Ok(false)` if any
    /// prerequisite (config or token) was missing — in which case any
    /// previously stored client is cleared. Returns `Err` on secret-store /
    /// HTTP build failures.
    pub fn try_build_client(&self) -> Result<bool, anyhow::Error> {
        let cfg = self.jira_config.read().unwrap().clone();
        let token = crate::keychain::load_jira_token(&self.app_data_dir)?;
        match (cfg, token) {
            (Some(c), Some(token)) => {
                let client = JiraClient::new(c.base_url.clone(), c.email.clone(), token)?;
                *self.jira_client.write().unwrap() = Some(client);
                Ok(true)
            }
            _ => {
                *self.jira_client.write().unwrap() = None;
                Ok(false)
            }
        }
    }

    /// Return a cheap clone of the current [`JiraClient`], if any.
    pub fn jira_client_cloned(&self) -> Option<JiraClient> {
        self.jira_client.read().unwrap().clone()
    }

    /// Return a clone of the current [`JiraConfig`], if any.
    pub fn jira_config_cloned(&self) -> Option<JiraConfig> {
        self.jira_config.read().unwrap().clone()
    }
}
