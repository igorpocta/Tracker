//! One-off commands that don't naturally fit into a bigger group.

use tauri_plugin_opener::OpenerExt;

use crate::state::AppState;

/// Open `<base_url>/browse/<issue_key>` in the user's default browser.
#[tauri::command]
pub async fn open_jira_issue(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    let cfg = state
        .jira_config_cloned()
        .ok_or_else(|| "jira config not loaded".to_string())?;
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/browse/{key}");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Open an arbitrary URL in the user's default browser.
#[tauri::command]
pub async fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// No-op on desktop. Exposed so mobile / future haptic-capable surfaces share
/// a single command name with the rest of the app.
#[tauri::command]
pub async fn haptic_feedback(_kind: String) -> Result<(), String> {
    Ok(())
}
