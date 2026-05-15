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
use crate::freelo;
use crate::state::AppState;

// -----------------------------------------------------------------------------
// Phase A5 — pure stop-timer math, extracted for unit testability.
//
// `stop_timer_inner` mixes Tauri State, DB writes, and HTTP calls. The pure
// arithmetic (raw duration, optional up-rounding, computed end time, UTC
// day-rollover flag) is split into [`compute_stop_outcome`] so it can be
// covered by unit tests without spinning up an AppState.
// -----------------------------------------------------------------------------

/// Pure result of stopping the timer: the raw and rounded durations, the
/// resulting `ended_at` (= `started_at + rounded`), and a flag indicating
/// whether the rounded span crosses a UTC midnight relative to the start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopOutcome {
    pub started_at: i64,
    pub ended_at: i64,
    pub raw_duration_s: i64,
    pub rounded_duration_s: i64,
    pub rolled_over_to_next_day: bool,
}

/// Pure logic of "what does stopping the timer produce?" — no DB, no Tauri
/// State, no I/O.
///
/// - `rounding_minutes == 0` (or `raw_duration_s == 0`) → no rounding;
///   `rounded == raw`.
/// - `rounding_minutes > 0` → ceiling (round-up) to the next multiple of
///   `rounding_minutes * 60` seconds. Matches the legacy
///   `apply_rounding(_, "up", interval)` shape from `commands::rounding`.
/// - Negative duration (clock skew / a stale timer past `now_s`) is clamped
///   to 0 — preserves the pre-extraction behavior in `record_local_stop`
///   and `stop_timer_inner`.
/// - `undo_window_seconds` is documentation-only here; the actual undo
///   timing lives in `delete_worklog` (`UNDO_WINDOW_MS`).
pub fn compute_stop_outcome(
    started_at_s: i64,
    now_s: i64,
    rounding_minutes: u32,
    _undo_window_seconds: u32,
) -> StopOutcome {
    let raw_duration_s = (now_s - started_at_s).max(0);
    let rounded_duration_s = if rounding_minutes == 0 || raw_duration_s == 0 {
        raw_duration_s
    } else {
        let step = (rounding_minutes as i64).saturating_mul(60);
        // Ceiling division.
        ((raw_duration_s + step - 1) / step) * step
    };
    let ended_at = started_at_s.saturating_add(rounded_duration_s);

    // UTC day-rollover detection: do the start and end fall on different UTC
    // calendar days? We only consult the date component so a 1-second span
    // straddling 23:59:59 → 00:00:00 counts as rolled over.
    let rolled_over_to_next_day = utc_date(started_at_s) != utc_date(ended_at);

    StopOutcome {
        started_at: started_at_s,
        ended_at,
        raw_duration_s,
        rounded_duration_s,
        rolled_over_to_next_day,
    }
}

/// UTC `NaiveDate` for a unix-second timestamp. Returns `None` for values
/// outside chrono's representable range — we map those to a sentinel that
/// equals itself but no real date so `utc_date(start) != utc_date(end)`
/// stays sane.
fn utc_date(unix_s: i64) -> Option<chrono::NaiveDate> {
    Utc.timestamp_opt(unix_s, 0)
        .single()
        .map(|d| d.date_naive())
}

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
    remote_id: Option<&str>,
    override_duration_s: Option<i64>,
) -> Result<WorklogRow, String> {
    let now_s = now_ms / 1000;
    let started_at_s = timer.started_at;
    let raw_duration_s = (now_s - started_at_s).max(0);
    let duration_s = override_duration_s.unwrap_or(raw_duration_s);
    let ended_at_s = started_at_s.saturating_add(duration_s);

    // Resolve the connection that owns this issue, if any. Lookup uses the
    // issues cache populated by the sync jobs.
    let connection_id = if timer.issue_key.is_empty() {
        None
    } else {
        cache::issues::get_connection_id_by_key(db, &timer.issue_key).map_err(|e| e.to_string())?
    };

    let is_synced = remote_id.is_some();
    let mut row = WorklogRow {
        id: None,
        connection_id,
        issue_key: if timer.issue_key.is_empty() {
            None
        } else {
            Some(timer.issue_key.clone())
        },
        description: comment.map(|s| s.to_string()),
        started_at: started_at_s,
        ended_at: ended_at_s,
        logged_at: now_s,
        updated_at: now_s,
        is_synced,
        synced_at: if is_synced { Some(now_s) } else { None },
        remote_id: remote_id.map(|s| s.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
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

    // Best-effort Jira auto-transition. Volá se na pozadí — selhání nikdy
    // nesmí přerušit start timeru. Pro Freelo úkoly přeskočí, protože
    // `resolve_jira_pair_for_issue` vrátí None.
    if !issue_key.is_empty() && !crate::freelo::is_freelo_key(&issue_key) {
        let key = issue_key.clone();
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            try_auto_transition(&handle, &key).await;
        });
    }

    Ok(res)
}

/// Pokusí se přejít issue do nakonfigurovaného stavu po startu timeru.
/// Tichý fallback — všechny chyby se logují přes tracing a nezvedají se
/// ven do UI.
async fn try_auto_transition(app: &tauri::AppHandle, issue_key: &str) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    // Najít Jira connection, která ten issue vlastní (pokud žádná → end).
    let conn_id = match crate::cache::issues::get_connection_id_by_key(&state.db, issue_key) {
        Ok(Some(id)) => id,
        _ => return,
    };
    let (client, cfg_json) = {
        let conns = state
            .connections
            .read()
            .expect("AppState.connections RwLock poisoned");
        let Some(c) = conns.iter().find(|c| c.id == conn_id) else {
            return;
        };
        let cfg_json = match crate::cache::connections::get_by_id(&state.db, conn_id) {
            Ok(Some(row)) => row.config_json,
            _ => return,
        };
        match &c.client {
            crate::state::ProviderClient::Jira(j) => (j.clone(), cfg_json),
            _ => return,
        }
    };
    let cfg: crate::commands::connections::JiraConnectionConfig =
        match serde_json::from_str(&cfg_json) {
            Ok(c) => c,
            Err(_) => return,
        };
    let from = match cfg.auto_transition_from.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };
    let to_name = match cfg.auto_transition_to_name.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };

    // Stávající status?
    let status = match client.get_issue_status(issue_key).await {
        Ok(Some(s)) => s,
        _ => return,
    };
    if !status.eq_ignore_ascii_case(&from) {
        return; // Issue není v očekávaném stavu — nic neuděláme.
    }

    // Najít transition_id pro `to_name`.
    let trans = match client.list_transitions(issue_key).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let Some((id, _)) = trans
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(&to_name))
    else {
        tracing::info!(
            target: "auto_transition",
            "issue {issue_key}: target status {to_name:?} není v list_transitions"
        );
        return;
    };
    if let Err(e) = client.transition_issue(issue_key, id).await {
        tracing::warn!(
            target: "auto_transition",
            "issue {issue_key}: transition selhalo: {e}"
        );
    }
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

/// Discard the running timer WITHOUT creating any worklog (local or remote).
///
/// The original Trcker exposed this as a `discard_timer` Tauri command. Use
/// cases:
///   - User started a timer by mistake.
///   - User worked on something unrelated and doesn't want to log it.
///   - User picked the wrong issue and prefers to start fresh.
///
/// Returns `true` if a timer was actually cleared, `false` if there was
/// nothing running. Emits `timer-discarded` so the UI (StartTrackingBar,
/// popover, tray) can refetch state.
#[tauri::command]
pub async fn discard_timer(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    let had_timer = cache::timer::get(&state.db)
        .map_err(|e| e.to_string())?
        .is_some();
    cache::timer::stop(&state.db).map_err(|e| e.to_string())?;
    let _ = app.emit("timer-discarded", had_timer);
    // Also emit timer-stopped so listeners that just watch for "any stop"
    // (popover, tray) don't need to subscribe to a second event name.
    let _ = app.emit::<Option<WorklogRow>>("timer-stopped", None);
    Ok(had_timer)
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

    let now = now_ms();
    // Phase A5 — pure raw-duration math lives in `compute_stop_outcome`.
    // We pass `rounding_minutes = 0` here because the user-configured
    // rounding may be `down` or `none` (the pure helper only knows
    // `up`); `apply_active_rounding` honours all three modes and is the
    // authoritative production rounding call. The helper's other outputs
    // (raw_duration_s, rolled_over_to_next_day) are still useful for
    // upcoming features but don't change behavior here.
    let outcome = compute_stop_outcome(timer.started_at, now / 1000, 0, 0);
    let raw_duration_s = outcome.raw_duration_s;

    // Phase 18A — Item 27: apply user-configured rounding.
    let duration_s = rounding::apply_active_rounding(&state.db, raw_duration_s);

    // Phase 18A — Item 4: an unassigned timer (issue_key == "") must NOT be
    // pushed to a provider; we save locally with `pending_assignment = 1`.
    let is_unassigned = timer.issue_key.is_empty();

    // Phase 18B — Item 6: fall back to the timer's in-flight comment when the
    // StopDialog didn't provide an override.
    let effective_comment: Option<String> = match (comment.as_deref(), timer.comment.as_deref()) {
        (Some(c), _) if !c.trim().is_empty() => Some(c.to_string()),
        (_, Some(c)) if !c.trim().is_empty() => Some(c.to_string()),
        _ => None,
    };

    // Dispatch by issue key prefix. Freelo: post a work_report and skip
    // `record_local_stop` (the freelo ops already insert the row). Jira:
    // legacy path through `state.jira_client_cloned()` for backwards compat.
    let mut freelo_saved: Option<WorklogRow> = None;
    let remote_id = if is_unassigned {
        None
    } else if freelo::is_freelo_key(&timer.issue_key) {
        // Route by `issues_v2.connection_id` for this specific Freelo task,
        // not by "first Freelo connection in the list". The resolver also
        // verifies `sync_user_id` is set — a None there used to fall through
        // as `.unwrap_or(0)`, which makes Freelo reject the work-report POST
        // with a generic 400 rather than a clear "finish setup first" error.
        match crate::commands::worklog::crud::resolve_freelo_client_with_user_for_issue(
            &state,
            &timer.issue_key,
        ) {
            Ok((conn_id, client, user_id)) => {
                match freelo::ops::add_work_report(
                    &client,
                    &state.db,
                    &timer.issue_key,
                    timer.started_at.saturating_mul(1000),
                    duration_s,
                    effective_comment.as_deref(),
                    conn_id,
                    user_id,
                )
                .await
                {
                    Ok(row) => {
                        let id = row.remote_id.clone();
                        freelo_saved = Some(row);
                        id
                    }
                    Err(e) => {
                        let _ = app.emit("worklog-error", e.to_string());
                        None
                    }
                }
            }
            Err(e) => {
                let _ = app.emit("worklog-error", e);
                None
            }
        }
    } else if let Some(client) = {
        // Route to the Jira connection that owns this issue (per
        // `issues_v2.connection_id`), falling back to the first enabled
        // Jira connection only when no row matches. Mirrors the lookup
        // shape the Freelo branch above already uses, and replaces the
        // legacy `state.jira_client_cloned()` shim that always picked
        // the FIRST Jira regardless of tenant.
        let issue_conn_id = cache::issues::get_connection_id_by_key(&state.db, &timer.issue_key)
            .ok()
            .flatten();
        let conns = state
            .connections
            .read()
            .expect("AppState.connections RwLock poisoned");
        issue_conn_id
            .and_then(|cid| {
                conns
                    .iter()
                    .find(|c| c.id == cid && c.enabled)
                    .and_then(|c| match &c.client {
                        crate::state::ProviderClient::Jira(j) => Some(j.clone()),
                        _ => None,
                    })
            })
            .or_else(|| {
                conns
                    .iter()
                    .filter(|c| c.enabled)
                    .find_map(|c| match &c.client {
                        crate::state::ProviderClient::Jira(j) => Some(j.clone()),
                        _ => None,
                    })
            })
    } {
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

    let row = if let Some(saved) = freelo_saved {
        // The Freelo ops already inserted the row via upsert_from_jira; we
        // just need to stop the timer to keep behaviour identical.
        cache::timer::stop(&state.db).map_err(|e| e.to_string())?;
        saved
    } else {
        record_local_stop(
            &state.db,
            &timer,
            now,
            effective_comment.as_deref(),
            remote_id.as_deref(),
            Some(duration_s),
        )?
    };
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

    let total = match cache::worklogs::total_seconds_for_range(db, from, to - 1) {
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
