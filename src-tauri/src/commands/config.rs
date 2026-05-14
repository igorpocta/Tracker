//! Setup / configuration commands.

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::config::{self, JiraConfig};
use crate::jira::{JiraClient, JiraError, JiraUser};
use crate::state::AppState;

/// Returns `true` if we have everything required to talk to Jira: an in-memory
/// `JiraConfig`, an in-memory `JiraClient`, and (implicitly, since the client
/// is only built when both exist) a keychain token.
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

/// Persist the supplied Jira configuration to disk + keychain and rebuild the
/// in-memory client. Emits a `config-changed` event on success.
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
    crate::keychain::save_jira_token(&args.token).map_err(|e| e.to_string())?;

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
