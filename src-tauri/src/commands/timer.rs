//! Active timer commands.
//!
//! The timer state is single-row (`active_timer` table, fixed `id = 1`). All
//! timestamps stored in the DB are **seconds** since the Unix epoch. The
//! Tauri-facing API accepts and returns **milliseconds** because that's what
//! JavaScript's `Date.now()` natively produces.

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self, timer::ActiveTimer, worklogs::WorklogRow, Db};
use crate::state::AppState;

/// Snapshot of a running timer as the frontend wants to see it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveTimerState {
    /// Jira issue key (e.g. `"ACME-12"`).
    pub issue_key: String,
    /// When the timer started, milliseconds since Unix epoch.
    pub started_at: i64,
    /// Elapsed seconds at the moment the snapshot was taken.
    pub elapsed_seconds: i64,
}

impl ActiveTimerState {
    fn from_timer(t: &ActiveTimer, now_ms: i64) -> Self {
        let started_at_ms = t.started_at.saturating_mul(1000);
        let elapsed_seconds = ((now_ms - started_at_ms).max(0)) / 1000;
        Self {
            issue_key: t.issue_key.clone(),
            started_at: started_at_ms,
            elapsed_seconds,
        }
    }
}

// -----------------------------------------------------------------------------
// Inner (Tauri-free) helpers — unit testable.
// -----------------------------------------------------------------------------

/// Pure logic for `get_timer_state`. Returns `Ok(None)` if no timer is running.
pub fn get_timer_state_inner(db: &Db, now_ms: i64) -> Result<Option<ActiveTimerState>, String> {
    match cache::timer::get(db).map_err(|e| e.to_string())? {
        Some(t) => Ok(Some(ActiveTimerState::from_timer(&t, now_ms))),
        None => Ok(None),
    }
}

/// Pure logic for `start_timer`. Replaces any running timer for the same row.
pub fn start_timer_inner(
    db: &Db,
    issue_key: &str,
    started_at_ms: i64,
) -> Result<ActiveTimerState, String> {
    let started_at_s = started_at_ms / 1000;
    cache::timer::start(db, issue_key, started_at_s).map_err(|e| e.to_string())?;
    Ok(ActiveTimerState {
        issue_key: issue_key.to_string(),
        started_at: started_at_s.saturating_mul(1000),
        elapsed_seconds: 0,
    })
}

/// Pure logic for `update_timer_start`. Errors if there is no active timer.
pub fn update_timer_start_inner(
    db: &Db,
    started_at_ms: i64,
    now_ms: i64,
) -> Result<ActiveTimerState, String> {
    let current = cache::timer::get(db)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no active timer".to_string())?;
    let started_at_s = started_at_ms / 1000;
    cache::timer::start(db, &current.issue_key, started_at_s).map_err(|e| e.to_string())?;
    Ok(ActiveTimerState::from_timer(
        &ActiveTimer {
            issue_key: current.issue_key,
            started_at: started_at_s,
        },
        now_ms,
    ))
}

/// Result of `stop_timer_inner`. The Jira worklog id is `None` when we couldn't
/// (or didn't) reach Jira — the row is still recorded locally so the UI can
/// surface it.
#[derive(Debug, Clone)]
pub struct StoppedTimer {
    pub row: WorklogRow,
}

/// Record a stop locally without talking to Jira. Used by tests and as a
/// helper when a client isn't configured.
pub fn record_local_stop(
    db: &Db,
    timer: &ActiveTimer,
    now_ms: i64,
    comment: Option<&str>,
    jira_worklog_id: Option<&str>,
) -> Result<WorklogRow, String> {
    let now_s = now_ms / 1000;
    let started_at_s = timer.started_at;
    let duration_s = (now_s - started_at_s).max(0);

    // Look up summary/issue_id from cache (best-effort).
    let (issue_id, summary) = match cache::issues::get_by_key(db, &timer.issue_key)
        .map_err(|e| e.to_string())?
    {
        Some(row) => (row.issue_id, Some(row.summary)),
        None => (None, None),
    };

    let mut row = WorklogRow {
        id: None,
        issue_key: timer.issue_key.clone(),
        issue_id,
        summary,
        duration_s,
        started_at: started_at_s,
        logged_at: now_s,
        comment: comment.map(|s| s.to_string()),
        jira_worklog_id: jira_worklog_id.map(|s| s.to_string()),
        author_account_id: None,
        source: "local".to_string(),
        updated_at_jira: None,
    };
    let id = cache::worklogs::record(db, &row).map_err(|e| e.to_string())?;
    row.id = Some(id);
    cache::timer::stop(db).map_err(|e| e.to_string())?;
    Ok(row)
}

// -----------------------------------------------------------------------------
// Tauri commands.
// -----------------------------------------------------------------------------

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

#[tauri::command]
pub async fn get_timer_state(
    state: tauri::State<'_, AppState>,
) -> Result<Option<ActiveTimerState>, String> {
    get_timer_state_inner(&state.db, now_ms())
}

#[tauri::command]
pub async fn start_timer(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
    started_at_ms: Option<i64>,
) -> Result<ActiveTimerState, String> {
    let started = started_at_ms.unwrap_or_else(now_ms);
    let res = start_timer_inner(&state.db, &issue_key, started)?;
    let _ = app.emit("timer-started", &res);
    Ok(res)
}

#[tauri::command]
pub async fn update_timer_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    started_at_ms: i64,
) -> Result<ActiveTimerState, String> {
    let res = update_timer_start_inner(&state.db, started_at_ms, now_ms())?;
    let _ = app.emit("timer-updated", &res);
    Ok(res)
}

/// Stop the running timer, push the elapsed duration to Jira (if a client is
/// configured), then record a row in `recent_worklogs`.
///
/// Returns `None` if no timer was running. If Jira is unreachable or no client
/// is configured we still record locally; `jira_worklog_id` will be `None`.
#[tauri::command]
pub async fn stop_timer_inner(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    comment: Option<String>,
) -> Result<Option<WorklogRow>, String> {
    // Resolve current timer; bail out cleanly if none.
    let timer = match cache::timer::get(&state.db).map_err(|e| e.to_string())? {
        Some(t) => t,
        None => return Ok(None),
    };

    let client = state.jira_client_cloned();
    let now = now_ms();
    let now_s = now / 1000;
    let duration_s = (now_s - timer.started_at).max(0);

    // Talk to Jira if possible.
    let jira_worklog_id = if let Some(client) = client {
        let started_dt = Utc
            .timestamp_opt(timer.started_at, 0)
            .single()
            .ok_or_else(|| "invalid started_at".to_string())?;
        match client
            .add_worklog(
                &timer.issue_key,
                started_dt,
                duration_s,
                comment.as_deref(),
            )
            .await
        {
            Ok(resp) => Some(resp.id),
            Err(e) => {
                // Surface as a soft error: record locally but still bubble up via event.
                let _ = app.emit("worklog-error", e.to_string());
                None
            }
        }
    } else {
        None
    };

    let row = record_local_stop(
        &state.db,
        &timer,
        now,
        comment.as_deref(),
        jira_worklog_id.as_deref(),
    )?;
    let _ = app.emit("worklog-saved", &row);
    let _ = app.emit("timer-stopped", &row);
    Ok(Some(row))
}
