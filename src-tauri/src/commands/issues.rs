//! Issue cache lookup and sync commands.

use serde::Serialize;
use tauri::Emitter;

use crate::cache::{self, issues::IssueRow};
use crate::jira;
use crate::state::AppState;

/// Shape returned by [`get_cache_stats`] — small summary of what's currently
/// stored locally. Useful for telemetry surfaces like the sidebar badge.
#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub issues: i64,
    pub worklogs_local: i64,
}

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
    // Legacy single-Jira shim: when no multi-connection rows are configured,
    // use connection_id = 0 as a sentinel for the legacy path.
    let conn_id: i64 = state
        .connections
        .read()
        .unwrap()
        .iter()
        .find(|c| matches!(c.client, crate::state::ProviderClient::Jira(_)))
        .map(|c| c.id)
        .unwrap_or(0);
    let n = jira::sync_issues_from_jira(&client, &state.db, conn_id)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("cache-refreshed", n);
    Ok(n)
}

/// Lightweight summary of what's in the local SQLite cache — exposed for
/// surfaces like the sidebar badge so they can show "real" numbers instead
/// of a hardcoded placeholder.
#[tauri::command]
pub async fn get_cache_stats(state: tauri::State<'_, AppState>) -> Result<CacheStats, String> {
    let issues = cache::issues::count(&state.db).map_err(|e| e.to_string())?;
    let worklogs_local = cache::worklogs::count(&state.db).map_err(|e| e.to_string())?;
    Ok(CacheStats {
        issues,
        worklogs_local,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Db;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn cache_stats_starts_at_zero() {
        let db = open_db();
        assert_eq!(cache::issues::count(&db).unwrap(), 0);
        assert_eq!(cache::worklogs::count(&db).unwrap(), 0);
    }

    #[test]
    fn cache_stats_reflects_inserts() {
        let db = open_db();
        // issues_v2 has a FK to connections — create one first.
        let conn_id = cache::connections::insert(
            &db,
            cache::connections::NewConnection {
                provider: "jira",
                name: "test",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        let issue = cache::issues::IssueRow {
            connection_id: conn_id,
            issue_id: "1".into(),
            issue_key: "ABC-1".into(),
            name: "hello".into(),
            created_at: 0,
            updated_at: 0,
            ..Default::default()
        };
        cache::issues::upsert(&db, &issue).unwrap();
        assert_eq!(cache::issues::count(&db).unwrap(), 1);
    }
}
