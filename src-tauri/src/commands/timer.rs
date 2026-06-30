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

/// Shared "what should be recorded when this timer stops?" plan used by both
/// the main Tauri command and the local HTTP bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopRecordPlan {
    pub duration_s: i64,
    pub effective_comment: Option<String>,
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
    /// Issue title (`issues_v2.name`) joined for display in the Running bar.
    /// `None` when the timer is unassigned or the cache doesn't know the key.
    #[serde(default)]
    pub summary: Option<String>,
}

/// Resolve the human-readable title for an issue key from the local cache.
/// Returns `None` for an empty key or when the cache has no match — both are
/// expected (unassigned timer, freshly added key not yet synced).
fn lookup_summary(db: &Db, issue_key: &str) -> Option<String> {
    let key = issue_key.trim();
    if key.is_empty() {
        return None;
    }
    cache::issues::get_by_key(db, key)
        .ok()
        .flatten()
        .map(|i| i.name)
}

impl ActiveTimerState {
    fn from_timer(db: &Db, t: &ActiveTimer, now_ms: i64) -> Self {
        let started_at_ms = t.started_at.saturating_mul(1000);
        let elapsed_seconds = ((now_ms - started_at_ms).max(0)) / 1000;
        Self {
            issue_key: t.issue_key.clone(),
            started_at: started_at_ms,
            elapsed_seconds,
            comment: t.comment.clone(),
            summary: lookup_summary(db, &t.issue_key),
        }
    }
}

// -----------------------------------------------------------------------------
// Inner (Tauri-free) helpers — unit testable.
// -----------------------------------------------------------------------------

/// Pure logic for `get_timer_state`. Returns `Ok(None)` if no timer is running.
pub fn get_timer_state_inner(db: &Db, now_ms: i64) -> Result<Option<ActiveTimerState>, String> {
    match cache::timer::get(db).map_err(|e| e.to_string())? {
        Some(t) => Ok(Some(ActiveTimerState::from_timer(db, &t, now_ms))),
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
    // P1-1: refuse to overwrite a running timer. This is the single chokepoint
    // for every start path (popover, tray, HTTP API, main window), so the guard
    // here protects all of them. Switching tasks must go through an explicit
    // stop first.
    let started =
        cache::timer::try_start_with_comment(db, issue_key, started_at_s, comment_norm.as_deref())
            .map_err(|e| e.to_string())?;
    if !started {
        return Err("Časomíra už běží. Nejdřív ji zastavte.".to_string());
    }
    Ok(ActiveTimerState {
        issue_key: issue_key.to_string(),
        started_at: started_at_s.saturating_mul(1000),
        elapsed_seconds: 0,
        comment: comment_norm,
        summary: lookup_summary(db, issue_key),
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
        db,
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
        db,
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
        db,
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

fn resolve_stop_comment(
    override_comment: Option<&str>,
    timer_comment: Option<&str>,
) -> Option<String> {
    match (override_comment, timer_comment) {
        (Some(c), _) if !c.trim().is_empty() => Some(c.to_string()),
        (_, Some(c)) if !c.trim().is_empty() => Some(c.to_string()),
        _ => None,
    }
}

/// Compute the final local stop-recording plan: user-configured rounding plus
/// the shared comment fallback semantics.
pub fn build_stop_record_plan(
    db: &Db,
    timer: &ActiveTimer,
    now_ms: i64,
    override_comment: Option<&str>,
) -> StopRecordPlan {
    let outcome = compute_stop_outcome(timer.started_at, now_ms / 1000, 0, 0);
    StopRecordPlan {
        duration_s: rounding::apply_active_rounding(db, outcome.raw_duration_s),
        effective_comment: resolve_stop_comment(override_comment, timer.comment.as_deref()),
    }
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
            .unwrap_or_else(|e| e.into_inner());
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
    let plan = build_stop_record_plan(&state.db, &timer, now, comment.as_deref());
    let duration_s = plan.duration_s;

    // Phase 18A — Item 4: an unassigned timer (issue_key == "") must NOT be
    // pushed to a provider; we save locally with `pending_assignment = 1`.
    let is_unassigned = timer.issue_key.is_empty();

    let effective_comment = plan.effective_comment;

    // P1-2: determine the provider from the connection that OWNS this issue
    // (`issues_v2.connection_id`), NOT from the "FREELO-" text prefix. A Jira
    // project whose key happens to start with FREELO must still route to Jira.
    // The prefix is only a fallback for issues we have never cached locally.
    enum OwnerRoute {
        Jira(crate::jira::JiraClient),
        Freelo,
    }
    let owner: Option<OwnerRoute> = {
        let cid = cache::issues::get_connection_id_by_key(&state.db, &timer.issue_key)
            .ok()
            .flatten();
        let conns = state
            .connections
            .read()
            .unwrap_or_else(|e| e.into_inner());
        cid.and_then(|id| conns.iter().find(|c| c.id == id && c.enabled))
            .map(|c| match &c.client {
                crate::state::ProviderClient::Jira(j) => OwnerRoute::Jira(j.clone()),
                crate::state::ProviderClient::Freelo(_) => OwnerRoute::Freelo,
            })
    };

    // Freelo when the owning connection is Freelo, or — for an uncached issue
    // with no known owner — when the key uses the Freelo prefix.
    let route_to_freelo = matches!(owner, Some(OwnerRoute::Freelo))
        || (owner.is_none() && freelo::is_freelo_key(&timer.issue_key));
    // The Jira client to post to: ONLY the connection that owns this issue.
    // P1-2: never fall back to "the first enabled Jira" — that could send the
    // worklog to the wrong tenant. An unknown owner is handled below.
    let owning_jira = match owner {
        Some(OwnerRoute::Jira(j)) => Some(j),
        _ => None,
    };

    // Serialize the POST+record with the flush task (which also takes this
    // lock per row): without it, a stop racing flush_unsynced_worklogs could
    // both push the same logical worklog. Held until the timer row is stopped
    // and the local row written. stop_timer_inner does not call any other
    // push-lock holder, so there is no re-entrant deadlock.
    let _push_guard = state.worklog_push_lock.lock().await;

    let mut freelo_saved: Option<WorklogRow> = None;
    let remote_id = if is_unassigned {
        None
    } else if route_to_freelo {
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
    } else if let Some(client) = owning_jira {
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
        // P1-2: we couldn't identify which connection owns this issue and the
        // key isn't a Freelo key, so there is no safe remote target. Save the
        // worklog locally instead of guessing a Jira tenant, and surface a
        // clear error so the user can assign it to the right account.
        let _ = app.emit(
            "worklog-error",
            format!(
                "Nepodařilo se určit účet pro úkol {} — záznam uložen lokálně. Přiřaďte ho k připojení.",
                timer.issue_key
            ),
        );
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

/// Wall-clock `[start_of_day, start_of_next_day)` range for `today` in the
/// given timezone, expressed as Unix seconds. Computes BOTH bounds via
/// `from_local_datetime`, so the returned interval matches the actual
/// wall-clock day on DST transitions (Europe/Prague spring-forward →
/// 23 h, fall-back → 25 h) instead of being naively `start + 86_400`.
///
/// Returns `None` when either local midnight is non-representable
/// (chrono's date arithmetic overflows at the i64 unix-second limits,
/// nominally year ±292 billion) or ambiguous in a way `single()` can't
/// resolve (no real-world TZ has DST flips at midnight, so this is
/// belt-and-suspenders only).
///
/// Generic over `TZ` so the unit test below can pin the contract with
/// `chrono::Utc` (no DST) without pulling in `chrono-tz`. The real DST
/// guarantee comes from the underlying `chrono::Local`
/// implementation — we don't ship a TZ database of our own.
fn local_day_bounds<TZ: chrono::TimeZone>(tz: &TZ, today: chrono::NaiveDate) -> Option<(i64, i64)> {
    let start_local = today.and_hms_opt(0, 0, 0)?;
    let from = tz.from_local_datetime(&start_local).single()?.timestamp();
    let tomorrow = today.succ_opt()?;
    let end_local = tomorrow.and_hms_opt(0, 0, 0)?;
    let to = tz
        .from_local_datetime(&end_local)
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(from + 86_400);
    Some((from, to))
}

/// Phase 18B — Item 12: best-effort "you've hit your daily goal" notification.
///
/// Fires at most once per local calendar day. The dedupe state lives in
/// `app_settings` (key: `today_goal_notified_at`).
fn maybe_notify_daily_goal_reached(app: &tauri::AppHandle, db: &Db) {
    // Snapshot today's local-day range, expressed in unix seconds.
    let now_local = Local::now();
    let today = now_local.date_naive();
    let (from, to) = match local_day_bounds(&Local, today) {
        Some(b) => b,
        None => return,
    };

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

#[cfg(test)]
mod tests {
    use super::local_day_bounds;
    use chrono::{NaiveDate, Utc};

    #[test]
    fn local_day_bounds_utc_is_exactly_24h() {
        // In UTC there is no DST, so the wall-clock day is always 86_400 s.
        // This pins the basic shape of the helper without depending on a
        // tz database. The real DST guarantee comes from `chrono::Local`
        // calling into the OS tz, exercised in manual / staging testing.
        let today = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let (from, to) = local_day_bounds(&Utc, today).expect("bounds");
        assert_eq!(to - from, 86_400);
    }

    #[test]
    fn local_day_bounds_is_inclusive_exclusive() {
        // `[from, to)` — `from` is local midnight, `to` is local midnight of
        // tomorrow. We use `total_seconds_for_range(from, to - 1)` for the
        // daily sum, so an off-by-one here would either drop or double-count
        // the final second of the day.
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let (from, to) = local_day_bounds(&Utc, today).expect("bounds");
        // `from` is start-of-day at 00:00:00 UTC; `to` is start-of-next-day.
        // Round-trip to a NaiveDateTime to assert the wall clock is exactly
        // midnight on each side.
        let from_dt = chrono::DateTime::from_timestamp(from, 0).unwrap();
        let to_dt = chrono::DateTime::from_timestamp(to, 0).unwrap();
        assert_eq!(from_dt.naive_utc().time(), chrono::NaiveTime::MIN);
        assert_eq!(to_dt.naive_utc().time(), chrono::NaiveTime::MIN);
        assert_eq!(to_dt.naive_utc().date(), today.succ_opt().unwrap());
    }
}
