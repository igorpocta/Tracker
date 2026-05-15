//! Multi-connection / multi-provider commands (Phase 18A — Items 13/15/28).
//!
//! The frontend uses these to:
//! - List configured connections (`list_connections`).
//! - Pre-flight a provider config without persisting (`test_connection_for_provider`).
//! - Persist a new connection (`add_connection`).
//! - Edit a connection (`update_connection`).
//! - Remove / enable / disable a connection.
//! - Run a per-connection "My issues" JQL.
//!
//! Legacy single-Jira commands (`save_config`, `update_config`, `sign_out`,
//! `test_jira_connection`, `get_current_config`) continue to work and operate
//! on the FIRST Jira connection so the existing frontend keeps functioning.

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self, connections::ConnectionRow};
use crate::freelo::FreeloClient;
use crate::jira::{JiraClient, JiraUser};
use crate::state::AppState;

/// Jira-specific config persisted in the `config_json` column. Free-form
/// JSON on disk, but we deserialise to this struct in the backend for
/// validation and convenience.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JiraConnectionConfig {
    pub base_url: String,
    pub email: String,
    /// Optional override for the issue-sync JQL (Phase 18A — Item 15).
    /// Falls back to `crate::jira::DEFAULT_JQL` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_jql: Option<String>,
    /// Optional JQL for the "My issues" page (Phase 18A — Item 15). Falls
    /// back to `assignee = currentUser() AND statusCategory != "Done"
    /// ORDER BY updated DESC`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub my_issues_jql: Option<String>,
}

/// Freelo-specific config persisted in the `config_json` column.
///
/// The user must explicitly opt-in projects via the project picker
/// (`set_freelo_selected_projects`); only tasks from those projects are
/// pulled into the shared cache and surfaced in timer / search dropdowns.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FreeloConnectionConfig {
    /// Defaults to [`crate::freelo::DEFAULT_BASE_URL`] when empty.
    #[serde(default)]
    pub base_url: String,
    pub email: String,
    /// Freelo project ids the user has opted in to syncing. Empty = no sync.
    #[serde(default)]
    pub selected_project_ids: Vec<i64>,
    /// Freelo user id of the authenticated account, cached after the first
    /// `me()` call. Used by the worklog sync to avoid re-discovering it on
    /// every pull.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_user_id: Option<i64>,
}

/// DTO returned to the frontend. The token is NEVER included — it lives in
/// the secret file under `connection:<id>:token`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDto {
    pub id: i64,
    pub provider: String,
    pub name: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Provider-specific config (e.g. `JiraConnectionConfig` for `provider="jira"`).
    pub config: serde_json::Value,
    /// `true` if a token is stored for this connection. We don't return the
    /// token itself but the UI uses this to know whether to prompt for one.
    pub has_token: bool,
}

/// Unified "who am I" result returned by [`test_connection_for_provider`].
/// Provider-specific fields are surfaced under their own keys so the frontend
/// can show a consistent "Connected as …" message regardless of provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUser {
    /// Stable provider-specific account id (Jira account id, Freelo user id …).
    pub account_id: String,
    pub display_name: String,
    pub email_address: Option<String>,
    /// `"jira"` or `"freelo"`. Lets the frontend choose the right icon /
    /// formatting without re-passing the provider key.
    pub provider: String,
}

/// Validate the optional JQL fields on a `JiraConnectionConfig`. Empty
/// values are normalised away by the typed deserializer; non-empty values
/// must pass the [`crate::validation::validate_jql`] checks (length, NUL).
fn validate_jira_config(cfg: &JiraConnectionConfig) -> Result<(), String> {
    if let Some(j) = &cfg.sync_jql {
        if !j.trim().is_empty() {
            crate::validation::validate_jql(j)?;
        }
    }
    if let Some(j) = &cfg.my_issues_jql {
        if !j.trim().is_empty() {
            crate::validation::validate_jql(j)?;
        }
    }
    Ok(())
}

impl ConnectionDto {
    pub fn from_row(row: ConnectionRow, has_token: bool) -> Self {
        let config: serde_json::Value =
            serde_json::from_str(&row.config_json).unwrap_or(serde_json::Value::Null);
        Self {
            id: row.id,
            provider: row.provider,
            name: row.name,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
            config,
            has_token,
        }
    }
}

#[tauri::command]
pub async fn list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionDto>, String> {
    let rows = cache::connections::list(&state.db).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let key = cache::connections::token_key(row.id);
        let has_token =
            crate::keychain::get(&state.app_data_dir, crate::keychain::KEYCHAIN_SERVICE, &key)
                .ok()
                .flatten()
                .is_some();
        out.push(ConnectionDto::from_row(row, has_token));
    }
    Ok(out)
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddConnectionArgs {
    pub provider: String,
    pub name: String,
    pub config: serde_json::Value,
    pub token: String,
}

#[tauri::command]
pub async fn add_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: AddConnectionArgs,
) -> Result<ConnectionDto, String> {
    if args.name.trim().is_empty() {
        return Err("Název připojení nesmí být prázdný".into());
    }
    if args.provider != "jira" && args.provider != "freelo" {
        return Err(format!(
            "Provider {:?} zatím není podporován",
            args.provider
        ));
    }
    if args.token.trim().is_empty() {
        return Err("Token nesmí být prázdný".into());
    }
    // Best-effort: validate JQL fields if the config deserialises as Jira.
    if args.provider == "jira" {
        if let Ok(cfg) = serde_json::from_value::<JiraConnectionConfig>(args.config.clone()) {
            validate_jira_config(&cfg)?;
        }
    }
    let config_json =
        serde_json::to_string(&args.config).map_err(|e| format!("invalid config JSON: {e}"))?;

    let id = cache::connections::insert(
        &state.db,
        cache::connections::NewConnection {
            provider: &args.provider,
            name: &args.name,
            enabled: true,
            config_json: &config_json,
        },
    )
    .map_err(|e| e.to_string())?;

    // Persist the token under the per-connection key.
    let key = cache::connections::token_key(id);
    crate::keychain::set(
        &state.app_data_dir,
        crate::keychain::KEYCHAIN_SERVICE,
        &key,
        &args.token,
    )
    .map_err(|e| e.to_string())?;

    // Re-hydrate the live client list so the new connection is usable
    // immediately (sync / commands picks it up).
    let _ = state.hydrate_connections();

    let row = cache::connections::get_by_id(&state.db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found after insert".to_string())?;

    let dto = ConnectionDto::from_row(row, true);
    let _ = app.emit("connections-changed", &dto);
    Ok(dto)
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateConnectionArgs {
    pub id: i64,
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub token: Option<String>,
    pub enabled: Option<bool>,
}

#[tauri::command]
pub async fn update_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: UpdateConnectionArgs,
) -> Result<ConnectionDto, String> {
    let cfg_str = match args.config {
        Some(v) => {
            if let Ok(cfg) = serde_json::from_value::<JiraConnectionConfig>(v.clone()) {
                validate_jira_config(&cfg)?;
            }
            Some(serde_json::to_string(&v).map_err(|e| e.to_string())?)
        }
        None => None,
    };
    cache::connections::update_fields(
        &state.db,
        args.id,
        args.name.as_deref(),
        args.enabled,
        cfg_str.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    if let Some(tok) = args.token.as_deref() {
        let key = cache::connections::token_key(args.id);
        crate::keychain::set(
            &state.app_data_dir,
            crate::keychain::KEYCHAIN_SERVICE,
            &key,
            tok,
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = state.hydrate_connections();

    let row = cache::connections::get_by_id(&state.db, args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found".to_string())?;
    let key = cache::connections::token_key(row.id);
    let has_token =
        crate::keychain::get(&state.app_data_dir, crate::keychain::KEYCHAIN_SERVICE, &key)
            .ok()
            .flatten()
            .is_some();
    let dto = ConnectionDto::from_row(row, has_token);
    let _ = app.emit("connections-changed", &dto);
    Ok(dto)
}

#[tauri::command]
pub async fn remove_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    // Token first (it would be orphaned if we crash between).
    let key = cache::connections::token_key(id);
    let _ = crate::keychain::delete(&state.app_data_dir, crate::keychain::KEYCHAIN_SERVICE, &key);
    cache::connections::delete(&state.db, id).map_err(|e| e.to_string())?;
    let _ = state.hydrate_connections();
    let _ = app.emit("connections-changed", id);
    Ok(())
}

#[tauri::command]
pub async fn enable_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<ConnectionDto, String> {
    cache::connections::update_fields(&state.db, id, None, Some(enabled), None)
        .map_err(|e| e.to_string())?;
    let _ = state.hydrate_connections();
    let row = cache::connections::get_by_id(&state.db, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found".to_string())?;
    let key = cache::connections::token_key(row.id);
    let has_token =
        crate::keychain::get(&state.app_data_dir, crate::keychain::KEYCHAIN_SERVICE, &key)
            .ok()
            .flatten()
            .is_some();
    let dto = ConnectionDto::from_row(row, has_token);
    let _ = app.emit("connections-changed", &dto);
    Ok(dto)
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestProviderArgs {
    pub provider: String,
    pub config: serde_json::Value,
    pub token: String,
}

/// Verify provider credentials without persisting. Dispatches per
/// `provider`: Jira → `myself()`, Freelo → `me()`.
#[tauri::command]
pub async fn test_connection_for_provider(args: TestProviderArgs) -> Result<ProviderUser, String> {
    match args.provider.as_str() {
        "jira" => {
            let cfg: JiraConnectionConfig =
                serde_json::from_value(args.config).map_err(|e| format!("invalid config: {e}"))?;
            let client =
                JiraClient::new(cfg.base_url, cfg.email, args.token).map_err(|e| e.to_string())?;
            let u: JiraUser = client.myself().await.map_err(|e| e.to_string())?;
            Ok(ProviderUser {
                account_id: u.account_id,
                display_name: u.display_name,
                email_address: u.email_address,
                provider: "jira".into(),
            })
        }
        "freelo" => {
            let cfg: FreeloConnectionConfig =
                serde_json::from_value(args.config).map_err(|e| format!("invalid config: {e}"))?;
            let base_url = if cfg.base_url.is_empty() {
                crate::freelo::DEFAULT_BASE_URL.to_string()
            } else {
                cfg.base_url
            };
            let client = FreeloClient::new(base_url, cfg.email.clone(), args.token)
                .map_err(|e| e.to_string())?;
            let u = client.me().await.map_err(|e| e.to_string())?;
            Ok(ProviderUser {
                account_id: u.id.to_string(),
                display_name: u.best_name(),
                email_address: u.email.clone().or(Some(cfg.email)),
                provider: "freelo".into(),
            })
        }
        other => Err(format!("Provider {:?} zatím není podporován", other)),
    }
}

/// Phase 18A — Item 15: list issues matching a connection's `my_issues_jql`.
/// Falls back to the default "assigned to me, not done" query if unset.
///
/// For Freelo connections, returns the cached tasks from the user's selected
/// projects (no JQL); the actual sync happens via the background pass.
#[tauri::command]
pub async fn list_my_issues(
    state: tauri::State<'_, AppState>,
    connection_id: i64,
    limit: Option<u32>,
) -> Result<Vec<cache::issues::IssueRow>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let row = cache::connections::get_by_id(&state.db, connection_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connection not found".to_string())?;

    // Resolve the live client for this connection.
    let client = {
        let conns = state.connections.read().unwrap();
        let active = conns
            .iter()
            .find(|c| c.id == connection_id && c.enabled)
            .ok_or_else(|| "Connection is not active".to_string())?;
        active.client.clone()
    };

    match client {
        crate::state::ProviderClient::Jira(client) => {
            let cfg: JiraConnectionConfig =
                serde_json::from_str(&row.config_json).unwrap_or_default();
            let jql = cfg.my_issues_jql.as_deref().unwrap_or(
                r#"assignee = currentUser() AND statusCategory != "Done" ORDER BY updated DESC"#,
            );
            let page = client
                .search_jql(jql, None, crate::jira::SYNC_FIELDS, limit)
                .await
                .map_err(|e| e.to_string())?;
            let mut out = Vec::with_capacity(page.issues.len());
            for issue in &page.issues {
                let row = crate::jira::map_issue_to_row(issue);
                cache::issues::upsert(&state.db, &row).map_err(|e| e.to_string())?;
                out.push(row);
            }
            Ok(out)
        }
        crate::state::ProviderClient::Freelo(_client, _cfg) => {
            // For Freelo: recent tasks from the cache (the sync pass writes
            // them on the schedule). We filter to keys that start with the
            // Freelo prefix to avoid mixing with Jira issues.
            let recent = cache::issues::recent(&state.db, limit * 4)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|r| crate::freelo::is_freelo_key(&r.issue_key))
                .take(limit as usize)
                .collect();
            Ok(recent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_jira_config_accepts_default() {
        let cfg = JiraConnectionConfig {
            base_url: "https://x.atlassian.net".into(),
            email: "u@example.com".into(),
            sync_jql: None,
            my_issues_jql: None,
        };
        assert!(validate_jira_config(&cfg).is_ok());
    }

    #[test]
    fn validate_jira_config_accepts_typical_jql() {
        let cfg = JiraConnectionConfig {
            base_url: String::new(),
            email: String::new(),
            sync_jql: Some("project = ACME".into()),
            my_issues_jql: Some("assignee = currentUser()".into()),
        };
        assert!(validate_jira_config(&cfg).is_ok());
    }

    #[test]
    fn validate_jira_config_rejects_nul_in_sync_jql() {
        let cfg = JiraConnectionConfig {
            base_url: String::new(),
            email: String::new(),
            sync_jql: Some("project = ACME\0".into()),
            my_issues_jql: None,
        };
        assert!(validate_jira_config(&cfg).is_err());
    }

    #[test]
    fn validate_jira_config_treats_empty_string_as_none() {
        // An empty string should be normalised away by the caller and
        // skipped here, not rejected as "empty JQL".
        let cfg = JiraConnectionConfig {
            base_url: String::new(),
            email: String::new(),
            sync_jql: Some(String::new()),
            my_issues_jql: Some("   ".into()),
        };
        assert!(validate_jira_config(&cfg).is_ok());
    }

    #[test]
    fn validate_jira_config_rejects_overlong_my_issues_jql() {
        let cfg = JiraConnectionConfig {
            base_url: String::new(),
            email: String::new(),
            sync_jql: None,
            my_issues_jql: Some("x".repeat(2001)),
        };
        assert!(validate_jira_config(&cfg).is_err());
    }
}
