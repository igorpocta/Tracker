//! Setup / configuration commands.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::config::{self, JiraConfig};
use crate::jira::{JiraClient, JiraError, JiraUser};
use crate::state::AppState;

/// Returns `true` if we have everything required to talk to Jira: an in-memory
/// `JiraConfig`, an in-memory `JiraClient`, and (implicitly, since the client
/// is only built when both exist) a Jira API token in the secret file.
#[tauri::command]
pub async fn has_config(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let has_cfg = state.jira_config.read().unwrap().is_some();
    let has_client = state.jira_client.read().unwrap().is_some();
    Ok(has_cfg && has_client)
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

    *state.jira_config.write().unwrap() = Some(args.config);
    state.try_build_client().map_err(|e| e.to_string())?;

    let _ = app.emit("config-changed", ());
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
pub fn sign_out_inner<F>(
    state: &AppState,
    config_path: &Path,
    clear_token: F,
) -> Result<(), String>
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
        crate::keychain::save_jira_token(&state.app_data_dir, tok)
            .map_err(|e| e.to_string())
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
        crate::keychain::clear_jira_token(&state.app_data_dir)
            .map_err(|e| e.to_string())
    })?;
    let _ = app.emit("config-changed", ());
    let _ = app.emit("main-window:navigate", "setup");
    Ok(())
}
