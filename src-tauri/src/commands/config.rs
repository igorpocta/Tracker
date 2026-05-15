//! Setup / configuration commands.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::config::{self, JiraConfig};
use crate::jira::{JiraClient, JiraError, JiraUser};
use crate::state::AppState;

/// Returns `true` if at least one usable connection (Jira or Freelo) is
/// configured. Phase 18F: previously only checked the legacy Jira shims;
/// now also accepts any hydrated connection so a Freelo-only install
/// reaches the main app instead of bouncing back to setup.
#[tauri::command]
pub async fn has_config(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let has_jira_legacy =
        state.jira_config.read().unwrap().is_some() && state.jira_client.read().unwrap().is_some();
    if has_jira_legacy {
        return Ok(true);
    }
    let any_connection = !state.connections.read().unwrap().is_empty();
    Ok(any_connection)
}

/// Payload accepted by `save_config`. Token is delivered together with the
/// other config fields so the frontend only has to make a single roundtrip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveConfigArgs {
    pub config: JiraConfig,
    pub token: String,
}

/// Persist the supplied Jira configuration to disk + secret file and rebuild
/// the in-memory client. Emits a `config-changed` event on success.
///
/// Phase 18A: also dual-writes into the multi-connection model so the new
/// frontend code paths can find the freshly-configured Jira account
/// immediately. If a connection named "Jira" already exists, it's updated;
/// otherwise a new row is created.
#[tauri::command]
pub async fn save_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: SaveConfigArgs,
) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config.toml");
    config::save_to_path(&path, &args.config).map_err(|e| e.to_string())?;
    crate::keychain::save_jira_token(&state.app_data_dir, &args.token)
        .map_err(|e| e.to_string())?;

    *state.jira_config.write().unwrap() = Some(args.config.clone());
    state.try_build_client().map_err(|e| e.to_string())?;

    // Phase 18A: upsert into `connections` so the new APIs see this account.
    upsert_legacy_jira_connection(&state, &args.config, &args.token)?;
    let _ = state.hydrate_connections();

    let _ = app.emit("config-changed", ());
    let _ = app.emit("connections-changed", ());
    Ok(())
}

fn upsert_legacy_jira_connection(
    state: &AppState,
    cfg: &JiraConfig,
    token: &str,
) -> Result<(), String> {
    use crate::cache;
    use crate::commands::connections::JiraConnectionConfig;

    let rows = cache::connections::list(&state.db).map_err(|e| e.to_string())?;
    let jira_cfg = JiraConnectionConfig {
        base_url: cfg.base_url.clone(),
        email: cfg.email.clone(),
        sync_jql: None,
        my_issues_jql: None,
    };
    let cfg_json = serde_json::to_string(&jira_cfg).unwrap_or_else(|_| "{}".to_string());

    let target_id = if let Some(row) = rows.iter().find(|r| r.provider == "jira") {
        cache::connections::update_fields(&state.db, row.id, None, Some(true), Some(&cfg_json))
            .map_err(|e| e.to_string())?;
        row.id
    } else {
        cache::connections::insert(
            &state.db,
            cache::connections::NewConnection {
                provider: "jira",
                name: "Jira",
                enabled: true,
                config_json: &cfg_json,
            },
        )
        .map_err(|e| e.to_string())?
    };

    let key = cache::connections::token_key(target_id);
    crate::keychain::set(
        &state.app_data_dir,
        crate::keychain::KEYCHAIN_SERVICE,
        &key,
        token,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Tell the main window to switch to the setup view. The frontend listens for
/// `main-window:navigate` and reacts accordingly.
#[tauri::command]
pub async fn enter_setup(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit("main-window:navigate", "setup");
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

/// Tell the main window to switch to the regular tracking UI.
#[tauri::command]
pub async fn enter_main_app(app: tauri::AppHandle) -> Result<(), String> {
    let _ = app.emit("main-window:navigate", "main");
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

/// Bring the main window to the foreground (or create the focus event so the
/// frontend re-renders). No-op if the window doesn't exist (e.g. during tests).
#[tauri::command]
pub async fn open_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.unminimize();
    }
    Ok(())
}

/// Test-only inner helper for [`test_jira_connection`]: builds a one-shot
/// [`JiraClient`] with the supplied credentials, calls `myself()`, and returns
/// the parsed user. The client is **not** stored anywhere — this is purely a
/// "are these credentials valid?" probe.
pub async fn test_jira_connection_inner(
    base_url: &str,
    email: &str,
    token: &str,
) -> Result<JiraUser, JiraError> {
    let client = JiraClient::new(base_url.to_string(), email.to_string(), token.to_string())?;
    client.myself().await
}

/// `test_jira_connection` — verify the supplied Jira credentials by hitting
/// `/rest/api/3/myself`. Does NOT persist anything; this is used by the setup
/// wizard to validate inputs before the user clicks "Finish".
#[tauri::command]
pub async fn test_jira_connection(
    base_url: String,
    email: String,
    token: String,
) -> Result<JiraUser, String> {
    test_jira_connection_inner(&base_url, &email, &token)
        .await
        .map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Phase 11A — runtime config mutation.
//
// `update_config_inner` and `sign_out_inner` are factored as Tauri-free
// helpers so unit tests can drive them with a tempdir-backed config path
// (and inject closures around the secret-store calls so tests don't need to
// agree on a particular on-disk location).
// -----------------------------------------------------------------------------

/// Pure helper for [`update_config`]: writes `config.toml`, optionally pushes
/// `new_token` into the secret file (via `save_token`), and rebuilds the
/// in-memory [`JiraClient`] from `state`.
///
/// `save_token` is a closure rather than a direct call to
/// [`crate::keychain::save_jira_token`] so tests can run without depending on
/// a particular secret-file location.
pub fn update_config_inner<F>(
    state: &AppState,
    config_path: &Path,
    new_cfg: JiraConfig,
    new_token: Option<String>,
    save_token: F,
) -> Result<(), String>
where
    F: FnOnce(&str) -> Result<(), String>,
{
    config::save_to_path(config_path, &new_cfg).map_err(|e| e.to_string())?;
    if let Some(tok) = new_token.as_deref() {
        save_token(tok)?;
    }
    *state.jira_config.write().unwrap() = Some(new_cfg);
    // try_build_client picks up the (possibly new) token from the secret file.
    state.try_build_client().map_err(|e| e.to_string())?;
    Ok(())
}

/// Pure helper for [`sign_out`]: deletes `config.toml`, clears the in-memory
/// state, and runs the supplied `clear_token` closure. Returns `Ok(())` even
/// if the config file is already absent (idempotent).
pub fn sign_out_inner<F>(state: &AppState, config_path: &Path, clear_token: F) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    // Best-effort delete — missing file is fine.
    if config_path.exists() {
        std::fs::remove_file(config_path).map_err(|e| e.to_string())?;
    }
    clear_token()?;
    *state.jira_config.write().unwrap() = None;
    *state.jira_client.write().unwrap() = None;
    Ok(())
}

fn config_path_for(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| e.to_string())
        .map(|d| d.join("config.toml"))
}

/// Return the on-disk Jira config (URL + email; no token). Returns `None`
/// when not configured.
#[tauri::command]
pub async fn get_current_config(
    state: tauri::State<'_, AppState>,
) -> Result<Option<JiraConfig>, String> {
    Ok(state.jira_config_cloned())
}

/// Update the persisted Jira config and (optionally) the on-disk token.
///
/// The frontend can omit `new_token` if only the URL or email is changing
/// (the existing token is still valid). Emits `config-changed` on success.
#[tauri::command]
pub async fn update_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    new_cfg: JiraConfig,
    new_token: Option<String>,
) -> Result<(), String> {
    let path = config_path_for(&app)?;
    update_config_inner(&state, &path, new_cfg, new_token, |tok| {
        crate::keychain::save_jira_token(&state.app_data_dir, tok).map_err(|e| e.to_string())
    })?;
    let _ = app.emit("config-changed", ());
    Ok(())
}

/// Clear the Jira config + on-disk token and reset the in-memory state.
///
/// Emits `config-changed` and tells the main window to navigate back to setup.
#[tauri::command]
pub async fn sign_out(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let path = config_path_for(&app)?;
    sign_out_inner(&state, &path, || {
        crate::keychain::clear_jira_token(&state.app_data_dir).map_err(|e| e.to_string())
    })?;
    let _ = app.emit("config-changed", ());
    let _ = app.emit("main-window:navigate", "setup");
    Ok(())
}
