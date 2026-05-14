//! Issue cache lookup and sync commands.

use tauri::Emitter;

use crate::cache::{self, issues::IssueRow};
use crate::jira;
use crate::state::AppState;

const DEFAULT_SEARCH_LIMIT: u32 = 50;
const DEFAULT_RECENT_LIMIT: u32 = 20;

#[tauri::command]
pub async fn search_issues_cache(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<IssueRow>, String> {
    let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
    cache::issues::search(&state.db, &query, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_recent_issues(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<IssueRow>, String> {
    let limit = limit.unwrap_or(DEFAULT_RECENT_LIMIT);
    cache::issues::recent(&state.db, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_suggested_issues(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<IssueRow>, String> {
    let limit = limit.unwrap_or(DEFAULT_RECENT_LIMIT);
    cache::issues::suggested(&state.db, limit).map_err(|e| e.to_string())
}

/// Sync issues from Jira into the local cache. Emits `cache-refreshed` with
/// the number of issues processed.
#[tauri::command]
pub async fn refresh_cache(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "jira client not configured".to_string())?;
    let n = jira::sync_issues_from_jira(&client, &state.db)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("cache-refreshed", n);
    Ok(n)
}
