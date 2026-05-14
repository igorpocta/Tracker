//! Worklog history commands.

use chrono::{Duration, Local};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self, worklogs::WorklogRow};
use crate::jira;
use crate::state::AppState;

const DEFAULT_LIMIT: u32 = 50;

#[tauri::command]
pub async fn get_worklog_issues(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<WorklogRow>, String> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    cache::worklogs::recent(&state.db, limit).map_err(|e| e.to_string())
}

/// Return all worklogs whose `started_at` falls inside `[from_unix_s, to_unix_s]`.
///
/// `with_author` optionally restricts to a specific account id. When omitted
/// (the typical UI case), all authors are returned — the sync already filters
/// to the current user, so the rows in the DB are almost always "mine".
#[tauri::command]
pub async fn get_worklogs_for_range(
    state: tauri::State<'_, AppState>,
    from_unix_s: i64,
    to_unix_s: i64,
    with_author: Option<String>,
) -> Result<Vec<WorklogRow>, String> {
    cache::worklogs::for_date_range(
        &state.db,
        from_unix_s,
        to_unix_s,
        with_author.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// Result payload of [`refresh_all`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshAllResult {
    pub issues: usize,
    pub worklogs: usize,
}

/// Sync both issues AND worklogs (for the last `from_days` days) in one go.
///
/// Errors fetching the current user (or running the worklog sync) are
/// reported via the return type, not swallowed — but issue sync failures
/// short-circuit early because the worklog sync depends on the issue cache
/// for summary lookups.
#[tauri::command]
pub async fn refresh_all(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    from_days: Option<u32>,
) -> Result<RefreshAllResult, String> {
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "jira client not configured".to_string())?;

    let issues = jira::sync_issues_from_jira(&client, &state.db)
        .await
        .map_err(|e| e.to_string())?;

    let me = client.myself().await.map_err(|e| e.to_string())?;
    let days = from_days.unwrap_or(30);
    let today = Local::now().date_naive();
    let from = today - Duration::days(days as i64);

    let worklogs = jira::worklog_sync::sync_worklogs_for_range(
        &client,
        &state.db,
        &me.account_id,
        from,
        today,
    )
    .await
    .map_err(|e| e.to_string())?;

    let result = RefreshAllResult { issues, worklogs };
    let _ = app.emit("cache-refreshed", issues);
    let _ = app.emit("worklogs-refreshed", worklogs);
    Ok(result)
}
