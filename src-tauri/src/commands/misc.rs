//! One-off commands that don't naturally fit into a bigger group.

use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, ProviderClient};

/// Open `<base_url>/browse/<issue_key>` in the user's default browser.
///
/// Legacy single-Jira path. Kept for backwards compatibility with older
/// callers; new code should prefer [`open_issue`] which routes by provider.
#[tauri::command]
pub async fn open_jira_issue(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    let url = jira_url_for_key(&state, &key)
        .ok_or_else(|| "jira config not loaded".to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Provider-aware "open this issue in the browser" command.
///
/// Routing:
///   - `FREELO-{id}`     → `https://app.freelo.io/task/{id}`
///   - `FREELO-P-{id}`   → `https://app.freelo.io/project/{id}` (rare, but
///                         project keys exist in the cache as historical
///                         parent context)
///   - anything else     → treated as a Jira key. We look up the issue's
///                         `connection_id` and use that connection's
///                         `base_url` so multi-Jira installs route to the
///                         correct host. Falls back to the legacy
///                         `state.jira_config` if the issue isn't cached.
#[tauri::command]
pub async fn open_issue(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    // Freelo task — hardcoded URL pattern per Freelo's web UI.
    if let Some(task_id) = crate::freelo::parse_task_key(&key) {
        let url = format!("https://app.freelo.io/task/{task_id}");
        return app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string());
    }
    if let Some(project_id) = crate::freelo::parse_project_key(&key) {
        let url = format!("https://app.freelo.io/project/{project_id}");
        return app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string());
    }

    // Otherwise Jira. Resolve the base URL from the connection that owns
    // this issue (multi-Jira friendly).
    let url = jira_url_for_key(&state, &key)
        .ok_or_else(|| "no Jira connection configured for this issue".to_string())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Build a Jira `<base>/browse/<key>` URL for `key`, picking the right
/// connection. Returns `None` if no Jira config / connection can be found.
fn jira_url_for_key(state: &AppState, key: &str) -> Option<String> {
    // Prefer the connection the cache says owns this issue (multi-Jira
    // friendly). Fall back to the first Jira connection. Fall back further
    // to the legacy single-Jira shim.
    let conn_id = crate::cache::issues::get_connection_id_by_key(&state.db, key)
        .ok()
        .flatten();

    let connections = state.connections.read().ok()?;
    let base_from_conn = conn_id
        .and_then(|id| connections.iter().find(|c| c.id == id))
        .or_else(|| {
            connections
                .iter()
                .find(|c| matches!(c.client, ProviderClient::Jira(_)))
        })
        .and_then(|c| match &c.client {
            ProviderClient::Jira(_) => c.jira_base_url(),
            _ => None,
        });

    let base = base_from_conn
        .or_else(|| state.jira_config_cloned().map(|c| c.base_url))?;
    let base = base.trim_end_matches('/').to_string();
    Some(format!("{base}/browse/{key}"))
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
