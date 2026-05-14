//! Worklog history commands.

use crate::cache::{self, worklogs::WorklogRow};
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
