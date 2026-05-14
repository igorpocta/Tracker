//! Active timer commands.
//!
//! The timer state is single-row (`active_timer` table, fixed `id = 1`). All
//! timestamps stored in the DB are **seconds** since the Unix epoch. The
//! Tauri-facing API accepts and returns **milliseconds** because that's what
//! JavaScript's `Date.now()` natively produces.

use chrono::{Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

use crate::cache::{self, timer::ActiveTimer, worklogs::WorklogRow, Db};
use crate::commands::{prefs, rounding};
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
    /// Phase 18B — Item 6: in-progress comment (null when blank).
    #[serde(default)]
    pub comment: Option<String>,
}

impl ActiveTimerState {
    fn from_timer(t: &ActiveTimer, now_ms: i64) -> Self {
        let started_at_ms = t.started_at.saturating_mul(1000);
        let elapsed_seconds = ((now_ms - started_at_ms).max(0)) / 1000;
        Self {
            issue_key: t.issue_key.clone(),
            started_at: started_at_ms,
            elapsed_seconds,
            comment: t.comment.clone(),
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
///
/// Phase 18A — Item 4: `issue_key` may be empty (or whitespace), in which case
/// the timer is "unassigned" and the UI surfaces a red ⚠ banner until the user
/// picks an issue. An unassigned timer stops into a `pending_assignment` row.
///
/// Phase 18B — Item 6: an optional starting `comment` is persisted so it can
/// be used when the timer is stopped (unless the StopDialog overrides it).
pub fn start_timer_inner(
    db: &Db,
    issue_key: &str,
    started_at_ms: i64,
    comment: Option<&str>,
) -> Result<ActiveTimerState, String> {
    // Normalise: trim whitespace; truly empty key is allowed (unassigned).
    let issue_key = issue_key.trim();
    if !issue_key.is_empty() {
        crate::validation::validate_issue_key(issue_key)?;
    }
    let started_at_s = started_at_ms / 1000;
    let comment_norm = comment
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());
    cache::timer::start_with_comment(db, issue_key, started_at_s, comment_norm.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(ActiveTimerState {
        issue_key: issue_key.to_string(),
        started_at: started_at_s.saturating_mul(1000),
        elapsed_seconds: 0,
        comment: comment_norm,
    })
}

/// Assign an issue to the currently running (previously unassigned) timer.
/// Returns the updated state, or an error if no timer is running.
pub fn assign_active_timer_inner(
    db: &Db,
    issue_key: &str,
    now_ms: i64,
) -> Result<ActiveTimerState, String> {
    crate::validation::validate_issue_key(issue_key)?;
    let issue_key = issue_key.trim();
    let current = cache::timer::get(db)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no active timer".to_string())?;
    // Preserve the in-flight comment when assigning an issue.
    cache::timer::start_with_comment(
        db,
        issue_key,
        current.started_at,
        current.comment.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    Ok(ActiveTimerState::from_timer(
        &ActiveTimer {
            issue_key: issue_key.to_string(),
            started_at: current.started_at,
            comment: current.comment,
        },
        now_ms,
    ))
}

/// Phase 18B — Item 6: update the comment on the currently-running timer
/// without otherwise modifying the state.
pub fn update_timer_comment_inner(
    db: &Db,
    comment: Option<&str>,
    now_ms: i64,
) -> Result<ActiveTimerState, String> {
    let current = cache::timer::get(db)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no active timer".to_string())?;
    let comment_norm = comment
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());
    cache::timer::set_comment(db, comment_norm.as_deref()).map_err(|e| e.to_string())?;
    Ok(ActiveTimerState::from_timer(
        &ActiveTimer {
            issue_key: current.issue_key,
            started_at: current.started_at,
            comment: comment_norm,
        },
        now_ms,
    ))
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
    cache::timer::start_with_comment(
        db,
        &current.issue_key,
        started_at_s,
        current.comment.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    Ok(ActiveTimerState::from_timer(
        &ActiveTimer {
            issue_key: current.issue_key,
            started_at: started_at_s,
            comment: current.comment,
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
///
/// `override_duration_s`: if `Some`, use that duration instead of (now - started).
/// Used by `stop_timer_inner` to apply rounding so the local row matches what
/// was POSTed to Jira.
pub fn record_local_stop(
    db: &Db,
    timer: &ActiveTimer,
    now_ms: i64,
    comment: Option<&str>,
    jira_worklog_id: Option<&str>,
    override_duration_s: Option<i64>,
) -> Result<WorklogRow, String> {
    let now_s = now_ms / 1000;
    let started_at_s = timer.started_at;
    let raw_duration_s = (now_s - started_at_s).max(0);
    let duration_s = override_duration_s.unwrap_or(raw_duration_s);

    // Look up summary/issue_id from cache (best-effort).
    let (issue_id, summary) =
        match cache::issues::get_by_key(db, &timer.issue_key).map_err(|e| e.to_string())? {
            Some(row) => (row.issue_id, Some(row.summary)),
            None => (None, None),
        };

    let pending_assignment = timer.issue_key.is_empty();
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
        pending_delete_at: None,
        tombstoned_at: None,
        pending_assignment,
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
    issue_key: Option<String>,
    started_at_ms: Option<i64>,
    comment: Option<String>,
) -> Result<ActiveTimerState, String> {
    // Phase 18A — Item 10: ALWAYS use server-side now() as the timer start
    // unless the user explicitly passed an override (e.g. backdating via
    // `update_timer_start`). The previous behaviour accepted whatever ms the
    // frontend supplied, which lagged because the displayed clock only ticks
    // every minute — producing a ~58s drift between wall clock and timer.
    let started = started_at_ms.unwrap_or_else(now_ms);
    let issue_key = issue_key.unwrap_or_default();
    let res = start_timer_inner(&state.db, &issue_key, started, comment.as_deref())?;
    let _ = app.emit("timer-started", &res);
    Ok(res)
}

/// Phase 18A — Item 4: assign an issue to the currently-running unassigned
/// timer. Does NOT post a worklog to Jira (the timer is still running).
#[tauri::command]
pub async fn assign_active_timer(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
) -> Result<ActiveTimerState, String> {
    let res = assign_active_timer_inner(&state.db, &issue_key, now_ms())?;
    let _ = app.emit("timer-updated", &res);
    Ok(res)
}

/// Phase 18B — Item 6: update the in-progress comment on the running timer.
#[tauri::command]
pub async fn update_timer_comment(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    comment: Option<String>,
) -> Result<ActiveTimerState, String> {
    let res = update_timer_comment_inner(&state.db, comment.as_deref(), now_ms())?;
    let _ = app.emit("timer-updated", &res);
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
    let raw_duration_s = (now_s - timer.started_at).max(0);

    // Phase 18A — Item 27: apply user-configured rounding.
    let duration_s = rounding::apply_active_rounding(&state.db, raw_duration_s);

    // Phase 18A — Item 4: an unassigned timer (issue_key == "") must NOT be
    // pushed to Jira; we save locally with `pending_assignment = 1`.
    let is_unassigned = timer.issue_key.is_empty();

    // Phase 18B — Item 6: fall back to the timer's in-flight comment when the
    // StopDialog didn't provide an override.
    let effective_comment: Option<String> = match (comment.as_deref(), timer.comment.as_deref()) {
        (Some(c), _) if !c.trim().is_empty() => Some(c.to_string()),
        (_, Some(c)) if !c.trim().is_empty() => Some(c.to_string()),
        _ => None,
    };

    // Talk to Jira if possible (and we have an issue key).
    let jira_worklog_id = if is_unassigned {
        None
    } else if let Some(client) = client {
        let started_dt = Utc
            .timestamp_opt(timer.started_at, 0)
            .single()
            .ok_or_else(|| "invalid started_at".to_string())?;
        match client
            .add_worklog(
                &timer.issue_key,
                started_dt,
                duration_s,
                effective_comment.as_deref(),
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
        effective_comment.as_deref(),
        jira_worklog_id.as_deref(),
        Some(duration_s),
    )?;
    let _ = app.emit("worklog-saved", &row);
    let _ = app.emit("timer-stopped", &row);

    // Phase 18B — Item 12: fire an OS notification when today's total just
    // crossed the daily goal for the first time today.
    maybe_notify_daily_goal_reached(&app, &state.db);

    Ok(Some(row))
}

/// Phase 18B — Item 12: best-effort "you've hit your daily goal" notification.
///
/// Fires at most once per local calendar day. The dedupe state lives in
/// `app_settings` (key: `today_goal_notified_at`).
fn maybe_notify_daily_goal_reached(app: &tauri::AppHandle, db: &Db) {
    // Snapshot today's local-day range, expressed in unix seconds.
    let now_local = Local::now();
    let today = now_local.date_naive();
    let start_local = today.and_hms_opt(0, 0, 0).unwrap_or_default();
    let from = match Local.from_local_datetime(&start_local).single() {
        Some(d) => d.timestamp(),
        None => return,
    };
    let to = from + 86_400;

    let total = match cache::worklogs::total_seconds_for_range(db, from, to - 1, None) {
        Ok(t) => t,
        Err(_) => return,
    };

    let goal = match prefs::get_daily_goal_inner(db) {
        Ok(g) if g > 0 => g,
        _ => return,
    };

    if total < goal {
        return;
    }

    // Dedupe via the settings key.
    let today_iso = today.format("%Y-%m-%d").to_string();
    match cache::settings::get(db, prefs::KEY_TODAY_GOAL_NOTIFIED_AT) {
        Ok(Some(v)) if v == today_iso => return,
        _ => {}
    }
    if cache::settings::set(db, prefs::KEY_TODAY_GOAL_NOTIFIED_AT, &today_iso).is_err() {
        return;
    }

    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let body = format!("🎉 Dnešní cíl splněn! Dnes máš {hours}h {minutes}m.");
    let _ = app
        .notification()
        .builder()
        .title("Tracker")
        .body(&body)
        .show();
}
