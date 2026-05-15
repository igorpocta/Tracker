//! Application-wide shared state for Tauri commands.
//!
//! Holds the open SQLite database connection pool plus the active set of
//! [`Connection`] instances (one live HTTP client per configured account) and
//! a few in-process recorders (e.g. user-activity).
//!
//! Phase 18A: replaced the single-connection model with a list. Legacy fields
//! `jira_config` / `jira_client` are retained as thin shims pointing at the
//! FIRST Jira connection so the existing commands (`save_config`,
//! `test_jira_connection`, etc.) continue to work while the frontend transitions
//! to the multi-connection API.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::cache::{self, activity::ActivityRecorder, Db};
use crate::commands::connections::{FreeloConnectionConfig, JiraConnectionConfig};
use crate::config::JiraConfig;
use crate::freelo::FreeloClient;
use crate::jira::JiraClient;

/// Discriminated provider clients. Phase 18E added the Freelo variant; future
/// providers (Toggl, Clockify, …) get their own variant here.
#[derive(Debug, Clone)]
pub enum ProviderClient {
    Jira(JiraClient),
    Freelo(FreeloClient, FreeloConnectionConfig),
}

/// An "active" connection: the row from the DB plus a built HTTP client.
#[derive(Debug, Clone)]
pub struct ActiveConnection {
    pub id: i64,
    pub kind: String,
    pub name: String,
    pub enabled: bool,
    pub client: ProviderClient,
}

impl ActiveConnection {
    /// Return the connection's Jira base URL if this is a Jira connection.
    /// Used by routing helpers that need to build `<base>/browse/KEY` style
    /// links for multi-Jira installs.
    pub fn jira_base_url(&self) -> Option<String> {
        match &self.client {
            ProviderClient::Jira(client) => Some(client.base_url().to_string()),
            _ => None,
        }
    }
}

/// Tauri-managed state shared across all command invocations.
pub struct AppState {
    /// Local SQLite cache (issues, worklogs, settings, active timer).
    pub db: Db,
    /// Application data directory — used to resolve the on-disk secret file
    /// (`secret.toml`) and any other per-install assets.
    pub app_data_dir: PathBuf,
    /// Multi-connection: one [`ActiveConnection`] per row in `connections`
    /// that has a usable token. Hydrated at startup via
    /// [`hydrate_connections`] and re-hydrated whenever the connection set
    /// changes (add/update/remove).
    pub connections: RwLock<Vec<ActiveConnection>>,
    /// In-process state for the user-activity feature.
    pub activity_recorder: ActivityRecorder,

    // ----- Legacy single-Jira shims (Phase 17 → 18A bridge) -------------------
    /// Last-known Jira configuration loaded from disk, if any. Phase 18A:
    /// derived from the first Jira connection; kept here for the old commands.
    pub jira_config: RwLock<Option<JiraConfig>>,
    /// Live HTTP client. Phase 18A: a clone of the first connection's client.
    pub jira_client: RwLock<Option<JiraClient>>,
}

impl AppState {
    pub fn new(db: Db, app_data_dir: PathBuf) -> Self {
        Self {
            db,
            app_data_dir,
            connections: RwLock::new(Vec::new()),
            activity_recorder: ActivityRecorder::new(),
            jira_config: RwLock::new(None),
            jira_client: RwLock::new(None),
        }
    }

    /// Rebuild the in-memory [`ActiveConnection`] list from the DB + secret
    /// file. Skips rows that have no token (the UI surfaces the missing token
    /// separately via `has_token: false` on the DTO).
    ///
    /// Also refreshes the legacy `jira_config` / `jira_client` shims using
    /// the first enabled Jira connection.
    pub fn hydrate_connections(&self) -> Result<(), anyhow::Error> {
        let rows = cache::connections::list(&self.db)?;
        let mut built = Vec::new();
        for row in rows {
            if !row.enabled {
                continue;
            }
            match row.provider.as_str() {
                "jira" => {
                    let cfg: JiraConnectionConfig =
                        serde_json::from_str(&row.config_json).unwrap_or_default();
                    let key = cache::connections::token_key(row.id);
                    let token = match crate::keychain::get(
                        &self.app_data_dir,
                        crate::keychain::KEYCHAIN_SERVICE,
                        &key,
                    )? {
                        Some(t) => t,
                        None => continue, // no token yet — skip
                    };
                    let client = JiraClient::new(cfg.base_url.clone(), cfg.email.clone(), token)?;
                    built.push(ActiveConnection {
                        id: row.id,
                        kind: row.provider.clone(),
                        name: row.name.clone(),
                        enabled: row.enabled,
                        client: ProviderClient::Jira(client),
                    });
                }
                "freelo" => {
                    let cfg: FreeloConnectionConfig =
                        serde_json::from_str(&row.config_json).unwrap_or_default();
                    let key = cache::connections::token_key(row.id);
                    let api_key = match crate::keychain::get(
                        &self.app_data_dir,
                        crate::keychain::KEYCHAIN_SERVICE,
                        &key,
                    )? {
                        Some(t) => t,
                        None => continue,
                    };
                    let base_url = if cfg.base_url.is_empty() {
                        crate::freelo::DEFAULT_BASE_URL.to_string()
                    } else {
                        cfg.base_url.clone()
                    };
                    let client = FreeloClient::new(base_url, cfg.email.clone(), api_key)?;
                    built.push(ActiveConnection {
                        id: row.id,
                        kind: row.provider.clone(),
                        name: row.name.clone(),
                        enabled: row.enabled,
                        client: ProviderClient::Freelo(client, cfg),
                    });
                }
                _ => continue, // unknown provider
            }
        }

        // Refresh legacy shims from the first Jira connection (if any).
        let first_jira = built.iter().find(|c| c.kind == "jira");
        if let Some(c) = first_jira {
            let client = match &c.client {
                ProviderClient::Jira(client) => client.clone(),
                // Should be unreachable because we filter on `kind == "jira"`.
                _ => unreachable!("first_jira filter broken"),
            };
            *self
                .jira_client
                .write()
                .expect("AppState.jira_client RwLock poisoned") = Some(client);
            // Best-effort derive a JiraConfig from the persisted JSON.
            let row = cache::connections::get_by_id(&self.db, c.id)?.unwrap();
            if let Ok(cfg) = serde_json::from_str::<JiraConnectionConfig>(&row.config_json) {
                *self
                    .jira_config
                    .write()
                    .expect("AppState.jira_config RwLock poisoned") = Some(JiraConfig {
                    base_url: cfg.base_url,
                    email: cfg.email,
                });
            }
        } else {
            *self
                .jira_client
                .write()
                .expect("AppState.jira_client RwLock poisoned") = None;
            *self
                .jira_config
                .write()
                .expect("AppState.jira_config RwLock poisoned") = None;
        }

        *self
            .connections
            .write()
            .expect("AppState.connections RwLock poisoned") = built;
        Ok(())
    }

    /// Try to build a fresh [`JiraClient`] from the legacy `jira_config` and
    /// on-disk token. Retained for backwards compatibility with the old
    /// `save_config`/`update_config` commands; new code paths go through
    /// [`hydrate_connections`].
    pub fn try_build_client(&self) -> Result<bool, anyhow::Error> {
        let cfg = self
            .jira_config
            .read()
            .expect("AppState.jira_config RwLock poisoned")
            .clone();
        let token = crate::keychain::load_jira_token(&self.app_data_dir)?;
        match (cfg, token) {
            (Some(c), Some(token)) => {
                let client = JiraClient::new(c.base_url.clone(), c.email.clone(), token)?;
                *self
                    .jira_client
                    .write()
                    .expect("AppState.jira_client RwLock poisoned") = Some(client);
                Ok(true)
            }
            _ => {
                *self
                    .jira_client
                    .write()
                    .expect("AppState.jira_client RwLock poisoned") = None;
                Ok(false)
            }
        }
    }

    pub fn jira_client_cloned(&self) -> Option<JiraClient> {
        self.jira_client
            .read()
            .expect("AppState.jira_client RwLock poisoned")
            .clone()
    }

    pub fn jira_config_cloned(&self) -> Option<JiraConfig> {
        self.jira_config
            .read()
            .expect("AppState.jira_config RwLock poisoned")
            .clone()
    }
}
