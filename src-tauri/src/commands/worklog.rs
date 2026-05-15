//! Worklog history and mutation commands.
//!
//! This module owns the full Phase-15 mutation surface:
//! - `create_manual_worklog`: appended via timer-stop OR the AddEntry panel.
//! - `update_worklog`: PUT changes to started/duration/comment in Jira.
//! - `delete_worklog`: soft-delete with 5s undo, then real Jira DELETE.
//! - `undo_delete_worklog`: clears the pending-delete flag.
//! - `move_worklog`: POST new + DELETE old composite (see `worklog_ops.rs`).
//! - `get_audit_log`: read-only access to the per-mutation audit trail.
//!
//! Every command writes to `cache::audit` so the user can forensically
//! reconstruct exactly what happened to their data.

use chrono::{Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

use crate::cache::{self, audit::AuditOp, worklogs::WorklogRow};
use crate::commands::rounding;
use crate::freelo;
use crate::jira;
use crate::jira::worklog_ops::{MoveWorklogArgs, MoveWorklogError};
use crate::jira::JiraError;
use crate::state::{AppState, ProviderClient};

const DEFAULT_LIMIT: u32 = 50;

/// How long the frontend's "Vrátit" (undo) banner is live; the background
/// task waits this long before firing the actual Jira DELETE.
const UNDO_WINDOW_MS: u64 = 5_000;

/// Maximum number of characters allowed in a worklog comment. Jira's hard
/// limit is much higher, but we cap conservatively to avoid pathological
/// payloads.
const MAX_COMMENT_CHARS: usize = 5_000;

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
    cache::worklogs::for_date_range(&state.db, from_unix_s, to_unix_s, with_author.as_deref())
        .map_err(|e| e.to_string())
}

/// Result payload of [`refresh_all`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshAllResult {
    pub issues: usize,
    pub worklogs: usize,
}

/// Sync both issues AND worklogs (for the last `from_days` days) in one go,
/// across all enabled connections (Jira + Freelo).
///
/// Errors per connection are tolerated — we keep going so a misconfigured
/// Freelo connection doesn't block the Jira sync (and vice versa). The
/// totals returned cover all connections that succeeded.
#[tauri::command]
pub async fn refresh_all(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    from_days: Option<u32>,
) -> Result<RefreshAllResult, String> {
    let days = from_days.unwrap_or(30);
    let today = Local::now().date_naive();
    let from = today - Duration::days(days as i64);

    let mut total_issues = 0usize;
    let mut total_worklogs = 0usize;

    let active = state.connections.read().unwrap().clone();
    for conn in active {
        match conn.client {
            ProviderClient::Jira(client) => {
                if let Ok(n) = jira::sync_issues_from_jira(&client, &state.db).await {
                    total_issues += n;
                }
                if let Ok(me) = client.myself().await {
                    if let Ok(n) = jira::worklog_sync::sync_worklogs_for_range(
                        &client,
                        &state.db,
                        &me.account_id,
                        from,
                        today,
                    )
                    .await
                    {
                        total_worklogs += n;
                    }
                }
            }
            ProviderClient::Freelo(client, cfg) => {
                if let Ok(n) = freelo::sync::sync_issues_for_connection(
                    &client,
                    &state.db,
                    &cfg.selected_project_ids,
                )
                .await
                {
                    total_issues += n;
                }
                if let Some(user_id) = cfg.sync_user_id {
                    if let Ok(n) = freelo::sync::sync_worklogs_for_range(
                        &client,
                        &state.db,
                        user_id,
                        from,
                        today,
                        &cfg.selected_project_ids,
                    )
                    .await
                    {
                        total_worklogs += n;
                    }
                }
            }
        }
    }

    // Fallback for the legacy single-Jira shim if no connections are
    // configured but a legacy client is set up.
    if state.connections.read().unwrap().is_empty() {
        if let Some(client) = state.jira_client_cloned() {
            if let Ok(n) = jira::sync_issues_from_jira(&client, &state.db).await {
                total_issues += n;
            }
            if let Ok(me) = client.myself().await {
                if let Ok(n) = jira::worklog_sync::sync_worklogs_for_range(
                    &client,
                    &state.db,
                    &me.account_id,
                    from,
                    today,
                )
                .await
                {
                    total_worklogs += n;
                }
            }
        }
    }

    let result = RefreshAllResult {
        issues: total_issues,
        worklogs: total_worklogs,
    };
    let _ = app.emit("cache-refreshed", total_issues);
    let _ = app.emit("worklogs-refreshed", total_worklogs);
    Ok(result)
}

// -----------------------------------------------------------------------------
// Phase 15 mutation commands
// -----------------------------------------------------------------------------

fn validate_comment(s: Option<&str>) -> Result<(), String> {
    if let Some(text) = s {
        if text.chars().count() > MAX_COMMENT_CHARS {
            return Err(format!(
                "Komentář je příliš dlouhý (max {MAX_COMMENT_CHARS} znaků)"
            ));
        }
        if text.contains('\0') {
            return Err("Komentář obsahuje neplatný znak (NUL)".into());
        }
    }
    Ok(())
}

fn audit_failure(
    db: &cache::Db,
    op: AuditOp,
    issue_key: Option<&str>,
    worklog_id: Option<&str>,
    before: Option<&WorklogRow>,
    err: &str,
) -> i64 {
    cache::audit::record(
        db,
        cache::audit::AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op,
            issue_key,
            worklog_id,
            before,
            after: None,
            success: false,
            error: Some(err),
            source_audit_id: None,
        },
    )
    .unwrap_or(0)
}

fn audit_success(
    db: &cache::Db,
    op: AuditOp,
    issue_key: Option<&str>,
    worklog_id: Option<&str>,
    before: Option<&WorklogRow>,
    after: Option<&WorklogRow>,
) -> i64 {
    cache::audit::record(
        db,
        cache::audit::AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op,
            issue_key,
            worklog_id,
            before,
            after,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap_or(0)
}

/// Look up the active client for the connection that owns `issue_key`.
/// Falls back to the first matching provider client if the issues table
/// doesn't have a `connection_id` recorded (legacy rows).
fn resolve_client_for_issue(
    state: &AppState,
    issue_key: &str,
) -> Result<(i64, ProviderClient), String> {
    let conn_id =
        cache::issues::get_connection_id_by_key(&state.db, issue_key).map_err(|e| e.to_string())?;
    let conns = state.connections.read().unwrap();
    // If we know the connection id, prefer that.
    if let Some(cid) = conn_id {
        if let Some(active) = conns.iter().find(|c| c.id == cid && c.enabled) {
            return Ok((active.id, active.client.clone()));
        }
    }
    // Fallback: pick the first connection whose provider can plausibly
    // handle this key (FRL- prefix → Freelo, anything else → Jira).
    let want_freelo = freelo::is_freelo_key(issue_key);
    for c in conns.iter().filter(|c| c.enabled) {
        match (&c.client, want_freelo) {
            (ProviderClient::Freelo(_, _), true) => return Ok((c.id, c.client.clone())),
            (ProviderClient::Jira(_), false) => return Ok((c.id, c.client.clone())),
            _ => {}
        }
    }
    Err("Žádné aktivní připojení pro tento úkol".into())
}

/// Create a new worklog manually (the AddEntry panel) and push it to the
/// provider. Dispatches by `issue_key` prefix:
///   - `FRL-…` → Freelo `add_work_report`
///   - anything else → Jira `add_worklog`
///
/// Strategy: call the provider FIRST (so the local row gets the upstream id
/// populated correctly), then insert/upsert the row. If the provider fails
/// the local DB is untouched and we return the error to the UI.
#[tauri::command]
pub async fn create_manual_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
    started_at_ms: i64,
    duration_seconds: i64,
    comment: Option<String>,
) -> Result<WorklogRow, String> {
    validate_comment(comment.as_deref())?;
    if duration_seconds <= 0 {
        return Err("Trvání musí být kladné".into());
    }
    if duration_seconds > 24 * 3600 {
        return Err("Trvání nesmí přesáhnout 24 hodin".into());
    }
    crate::validation::validate_issue_key(&issue_key)?;

    // Phase 18A — Item 27: apply rounding before talking to the provider.
    let duration_seconds = rounding::apply_active_rounding(&state.db, duration_seconds);

    // Dispatch by provider.
    if freelo::is_freelo_key(&issue_key) {
        return create_freelo_worklog(app, state, issue_key, started_at_ms, duration_seconds, comment).await;
    }

    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;

    let started_dt = Utc
        .timestamp_millis_opt(started_at_ms)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;

    let resp = match client
        .add_worklog(&issue_key, started_dt, duration_seconds, comment.as_deref())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_failure(
                &state.db,
                AuditOp::Create,
                Some(&issue_key),
                None,
                None,
                &e.to_string(),
            );
            return Err(format!("Jira: {e}"));
        }
    };

    // Pull author + summary from local caches (best-effort).
    let author = state
        .jira_config_cloned()
        .map(|c| c.email)
        .unwrap_or_default();
    let (issue_id, summary) =
        match cache::issues::get_by_key(&state.db, &issue_key).map_err(|e| e.to_string())? {
            Some(row) => (row.issue_id, Some(row.summary)),
            None => (resp.issue_id.clone(), None),
        };

    let started_at_s = started_at_ms / 1000;
    let now_s = Utc::now().timestamp();
    let row = WorklogRow {
        id: None,
        issue_key: issue_key.clone(),
        issue_id,
        summary,
        duration_s: duration_seconds,
        started_at: started_at_s,
        logged_at: now_s,
        comment: comment.clone(),
        jira_worklog_id: Some(resp.id.clone()),
        author_account_id: if author.is_empty() {
            None
        } else {
            Some(author)
        },
        source: "jira".to_string(),
        updated_at_jira: Some(now_s),
        pending_delete_at: None,
        tombstoned_at: None,
        pending_assignment: false,
    };
    let local_id = cache::worklogs::upsert_from_jira(&state.db, &row).map_err(|e| e.to_string())?;
    let mut saved = row.clone();
    saved.id = Some(local_id);

    audit_success(
        &state.db,
        AuditOp::Create,
        Some(&issue_key),
        Some(&resp.id),
        None,
        Some(&saved),
    );

    let _ = app.emit("worklog-created", &saved);
    Ok(saved)
}

/// Freelo branch of [`create_manual_worklog`]. Extracted to keep the main
/// function readable.
async fn create_freelo_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
    started_at_ms: i64,
    duration_seconds: i64,
    comment: Option<String>,
) -> Result<WorklogRow, String> {
    // Reject 0-minute entries (Freelo requires ≥ 1 minute and surfaces it as
    // a generic 400 — give the user a clearer message up front).
    if duration_seconds < 60 {
        return Err("Doba musí být alespoň minuta".into());
    }

    let (conn_id, client) = resolve_client_for_issue(&state, &issue_key)?;
    let (client, cfg) = match client {
        ProviderClient::Freelo(c, cfg) => (c, cfg),
        _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
    };
    let user_id = cfg
        .sync_user_id
        .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;

    let saved = match freelo::ops::add_work_report(
        &client,
        &state.db,
        &issue_key,
        started_at_ms,
        duration_seconds,
        comment.as_deref(),
        conn_id,
        user_id,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_failure(
                &state.db,
                AuditOp::Create,
                Some(&issue_key),
                None,
                None,
                &e.to_string(),
            );
            return Err(format!("Freelo: {e}"));
        }
    };

    audit_success(
        &state.db,
        AuditOp::Create,
        Some(&issue_key),
        saved.jira_worklog_id.as_deref(),
        None,
        Some(&saved),
    );

    let _ = app.emit("worklog-created", &saved);
    Ok(saved)
}

/// Update a local-only worklog row (no upstream remote id yet).
///
/// Used by the TimeLog inline edit when the row's `jira_worklog_id` is
/// null — the worklog exists only in our SQLite cache, so we just patch the
/// cache columns and emit `worklog-updated`. No Jira/Freelo HTTP call is
/// attempted. Once the row eventually syncs upstream the regular
/// [`update_worklog`] path takes over.
///
/// Args take **local rowid** (`id` from `recent_worklogs`), unlike
/// [`update_worklog`] which takes the upstream id string.
#[tauri::command]
pub async fn update_local_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    local_id: i64,
    new_issue_key: Option<String>,
    new_started_at_ms: Option<i64>,
    new_duration_seconds: Option<i64>,
    new_comment: Option<String>,
) -> Result<WorklogRow, String> {
    validate_comment(new_comment.as_deref())?;
    if let Some(ref k) = new_issue_key {
        if !k.is_empty() {
            crate::validation::validate_issue_key(k)?;
        }
    }
    if let Some(d) = new_duration_seconds {
        if d <= 0 {
            return Err("Trvání musí být kladné".into());
        }
        if d > 24 * 3600 {
            return Err("Trvání nesmí přesáhnout 24 hodin".into());
        }
    }

    let before = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;

    let next_started_at = match new_started_at_ms {
        Some(ms) => ms / 1000,
        None => before.started_at,
    };
    let next_duration = new_duration_seconds.unwrap_or(before.duration_s);
    let next_comment = match new_comment {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => before.comment.clone(),
    };
    let next_issue_key = new_issue_key.unwrap_or_else(|| before.issue_key.clone());

    cache::worklogs::update_fields(
        &state.db,
        local_id,
        &next_issue_key,
        before.issue_id.as_deref(),
        before.summary.as_deref(),
        next_duration,
        next_started_at,
        next_comment.as_deref(),
        before.updated_at_jira,
    )
    .map_err(|e| e.to_string())?;

    let after = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po aktualizaci".to_string())?;

    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

/// Update an existing worklog. Updates the provider first, then the local
/// DB so an upstream failure leaves the cache untouched. Dispatches by
/// `issue_key` prefix (FRL- → Freelo, else Jira).
#[tauri::command]
pub async fn update_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: String,
    issue_key: String,
    new_started_at_ms: Option<i64>,
    new_duration_seconds: Option<i64>,
    new_comment: Option<String>,
) -> Result<WorklogRow, String> {
    validate_comment(new_comment.as_deref())?;
    crate::validation::validate_issue_key(&issue_key)?;
    if let Some(d) = new_duration_seconds {
        if d <= 0 {
            return Err("Trvání musí být kladné".into());
        }
        if d > 24 * 3600 {
            return Err("Trvání nesmí přesáhnout 24 hodin".into());
        }
    }

    if freelo::is_freelo_key(&issue_key) {
        return update_freelo_worklog(
            app,
            state,
            worklog_id,
            issue_key,
            new_started_at_ms,
            new_duration_seconds,
            new_comment,
        )
        .await;
    }

    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;

    let before = cache::worklogs::get_by_jira_id(&state.db, &worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())?;

    let started_dt = match new_started_at_ms {
        Some(ms) => Some(
            Utc.timestamp_millis_opt(ms)
                .single()
                .ok_or_else(|| "Neplatný čas začátku".to_string())?,
        ),
        None => None,
    };

    // Phase 18A — Item 27: round the new duration before talking to Jira.
    let new_duration_seconds = new_duration_seconds.map(|d| {
        if d > 24 * 3600 {
            d
        } else {
            rounding::apply_active_rounding(&state.db, d)
        }
    });

    // PUT to Jira.
    let resp = match client
        .update_worklog(
            &issue_key,
            &worklog_id,
            started_dt,
            new_duration_seconds,
            new_comment.as_deref(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_failure(
                &state.db,
                AuditOp::Update,
                Some(&issue_key),
                Some(&worklog_id),
                Some(&before),
                &e.to_string(),
            );
            return Err(format!("Jira: {e}"));
        }
    };

    // Build the new row from before + new fields.
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;
    let new_started = new_started_at_ms
        .map(|ms| ms / 1000)
        .unwrap_or(before.started_at);
    let new_duration = new_duration_seconds.unwrap_or(before.duration_s);
    let new_comment_for_db = match &new_comment {
        Some(s) if s.trim().is_empty() => None,
        Some(s) => Some(s.clone()),
        None => before.comment.clone(),
    };
    let now_s = Utc::now().timestamp();

    cache::worklogs::update_fields(
        &state.db,
        local_id,
        &issue_key,
        before.issue_id.as_deref(),
        before.summary.as_deref(),
        new_duration,
        new_started,
        new_comment_for_db.as_deref(),
        Some(now_s),
    )
    .map_err(|e| e.to_string())?;

    let after = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po aktualizaci".to_string())?;

    audit_success(
        &state.db,
        AuditOp::Update,
        Some(&issue_key),
        Some(&resp.id),
        Some(&before),
        Some(&after),
    );

    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

/// Freelo branch of [`update_worklog`].
async fn update_freelo_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: String,
    issue_key: String,
    new_started_at_ms: Option<i64>,
    new_duration_seconds: Option<i64>,
    new_comment: Option<String>,
) -> Result<WorklogRow, String> {
    let before = cache::worklogs::get_by_jira_id(&state.db, &worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())?;
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;

    // Parse the freelo:N synthetic id back into the numeric work_report_id.
    let wr_id = freelo::parse_worklog_id(&worklog_id)
        .ok_or_else(|| format!("Neplatné Freelo id záznamu: {worklog_id}"))?;

    let (_, client) = resolve_client_for_issue(&state, &issue_key)?;
    let client = match client {
        ProviderClient::Freelo(c, _) => c,
        _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
    };

    let after = match freelo::ops::update_work_report(
        &client,
        &state.db,
        local_id,
        wr_id,
        new_started_at_ms,
        new_duration_seconds,
        new_comment.as_deref(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_failure(
                &state.db,
                AuditOp::Update,
                Some(&issue_key),
                Some(&worklog_id),
                Some(&before),
                &e.to_string(),
            );
            return Err(format!("Freelo: {e}"));
        }
    };

    audit_success(
        &state.db,
        AuditOp::Update,
        Some(&issue_key),
        Some(&worklog_id),
        Some(&before),
        Some(&after),
    );

    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

/// Soft-delete a worklog (Phase 15 safety net).
///
/// 1. Marks `pending_delete_at = now` so the UI can hide the row optimistically.
/// 2. Returns immediately.
/// 3. Schedules a background task that, after [`UNDO_WINDOW_MS`], checks
///    whether the row is still pending-delete. If so → call `Jira DELETE`
///    and mark `tombstoned_at`. If not (user pressed undo), no-op.
///
/// The audit log records the user-intent moment (mark_pending) and the
/// commit moment separately.
#[tauri::command]
pub async fn delete_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: String,
    issue_key: String,
) -> Result<(), String> {
    let before = cache::worklogs::get_by_jira_id(&state.db, &worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())?;
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;

    let now_s = Utc::now().timestamp();
    cache::worklogs::mark_pending_delete(&state.db, local_id, now_s).map_err(|e| e.to_string())?;

    audit_success(
        &state.db,
        AuditOp::Delete,
        Some(&issue_key),
        Some(&worklog_id),
        Some(&before),
        None,
    );

    let _ = app.emit("worklog-deleted", &before);

    // Schedule the background commit. We clone everything the task needs.
    let app_h = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(UNDO_WINDOW_MS)).await;
        let state = app_h.state::<AppState>();
        commit_pending_delete(&app_h, &state, local_id, &issue_key, &worklog_id).await;
    });

    Ok(())
}

/// Clear the pending-delete flag (user pressed undo within the 5s window).
#[tauri::command]
pub async fn undo_delete_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: String,
) -> Result<(), String> {
    let before = cache::worklogs::get_by_jira_id(&state.db, &worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())?;
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;

    cache::worklogs::clear_pending_delete(&state.db, local_id).map_err(|e| e.to_string())?;

    audit_success(
        &state.db,
        AuditOp::Undo,
        Some(&before.issue_key),
        Some(&worklog_id),
        Some(&before),
        None,
    );

    let _ = app.emit("worklog-undo-deleted", &before);
    Ok(())
}

/// Move a worklog from one issue to another. Calls into
/// [`crate::jira::worklog_ops::move_worklog`] (POST new + DELETE old).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn move_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    old_issue_key: String,
    old_worklog_id: String,
    new_issue_key: String,
    started_at_ms: i64,
    duration_seconds: i64,
    comment: Option<String>,
) -> Result<MoveWorklogResultDto, String> {
    validate_comment(comment.as_deref())?;
    if duration_seconds <= 0 {
        return Err("Trvání musí být kladné".into());
    }
    if duration_seconds > 24 * 3600 {
        return Err("Trvání nesmí přesáhnout 24 hodin".into());
    }
    crate::validation::validate_issue_key(&old_issue_key)?;
    crate::validation::validate_issue_key(&new_issue_key)?;

    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;

    let started_dt = Utc
        .timestamp_millis_opt(started_at_ms)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;

    let before =
        cache::worklogs::get_by_jira_id(&state.db, &old_worklog_id).map_err(|e| e.to_string())?;

    let account_id = before.as_ref().and_then(|b| b.author_account_id.clone());

    let args = MoveWorklogArgs {
        old_issue_key: &old_issue_key,
        old_worklog_id: &old_worklog_id,
        new_issue_key: &new_issue_key,
        started: started_dt,
        time_spent_seconds: duration_seconds,
        comment: comment.as_deref(),
        author_account_id: account_id.as_deref(),
    };

    match jira::worklog_ops::move_worklog(&client, &state.db, args).await {
        Ok(res) => {
            audit_success(
                &state.db,
                AuditOp::Move,
                Some(&new_issue_key),
                Some(&res.new_worklog_id),
                before.as_ref(),
                Some(&res.new_row),
            );
            let _ = app.emit("worklog-moved", &res.new_row);
            Ok(MoveWorklogResultDto {
                new_worklog_id: res.new_worklog_id,
                new_row: res.new_row,
                original_still_exists: false,
            })
        }
        Err(MoveWorklogError::CreateFailed(e)) => {
            audit_failure(
                &state.db,
                AuditOp::Move,
                Some(&old_issue_key),
                Some(&old_worklog_id),
                before.as_ref(),
                &e.to_string(),
            );
            Err(format!("Jira: {e}"))
        }
        Err(MoveWorklogError::DeleteAfterCreate {
            new_worklog_id,
            old_issue_key,
            source,
        }) => {
            audit_failure(
                &state.db,
                AuditOp::Move,
                Some(&old_issue_key),
                Some(&old_worklog_id),
                before.as_ref(),
                &format!("delete after create failed (new id {new_worklog_id}): {source}"),
            );
            // Preserve the original Tracker error string so the UI can show
            // "Original worklog still exists on {key}" + a manual retry
            // affordance. The new worklog id is captured in the audit log.
            Err(format!(
                "Original worklog still exists on {old_issue_key}: {source}"
            ))
        }
        Err(MoveWorklogError::Db(e)) => Err(e.to_string()),
    }
}

/// Wire shape returned by `move_worklog`. `original_still_exists` is set to
/// true only on the `DeleteAfterCreate` partial-success path (we don't reach
/// here in the current implementation because that case returns Err — kept
/// here for forward compatibility).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoveWorklogResultDto {
    pub new_worklog_id: String,
    pub new_row: WorklogRow,
    pub original_still_exists: bool,
}

/// Return audit entries newest-first, with optional pagination + filters.
///
/// - `limit`: max rows to return (defaults to 50).
/// - `before_id`: when paginating, pass the last `id` from the previous page.
/// - `ops`: restrict to specific op kinds (e.g. `["delete", "update"]`).
/// - `only_failed`: when true, only return rows where `success = 0`.
#[tauri::command]
pub async fn get_audit_log(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
    before_id: Option<i64>,
    ops: Option<Vec<String>>,
    only_failed: Option<bool>,
) -> Result<Vec<cache::audit::AuditEntry>, String> {
    cache::audit::list(
        &state.db,
        limit.unwrap_or(50),
        before_id,
        ops.as_deref(),
        only_failed.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}

/// Phase 16 — purge audit rows older than `older_than_days` days. Returns the
/// number of rows actually deleted.
#[tauri::command]
pub async fn purge_audit_log(
    state: tauri::State<'_, AppState>,
    older_than_days: u32,
) -> Result<u32, String> {
    let cutoff = Utc::now().timestamp() - (older_than_days as i64) * 86_400;
    let n = cache::audit::purge_older_than(&state.db, cutoff).map_err(|e| e.to_string())?;
    Ok(n as u32)
}

// -----------------------------------------------------------------------------
// Phase 16 reconstruction commands
//
// The heavy lifting (Jira I/O, snapshot parsing, audit linkage) lives in
// `jira::reconstruct`; these wrappers just look up the `JiraClient` from
// application state and translate the typed errors into UI strings.
// -----------------------------------------------------------------------------

fn reconstruct_err_to_string(e: jira::reconstruct::ReconstructError) -> String {
    match e {
        jira::reconstruct::ReconstructError::Jira(je) => format!("Jira: {je}"),
        other => other.to_string(),
    }
}

/// Phase 16 — re-create a worklog in Jira from a previous audit entry's
/// `before_json` snapshot.
///
/// Accepts audit entries of op = `delete` (we explicitly soft-deleted via the
/// Tracker UI) or `sync_tombstone` (the row was detected as deleted in Jira by
/// the mark-and-sweep pass). Both carry the full pre-deletion `WorklogRow`
/// snapshot, which has everything we need to POST a fresh worklog:
/// `issue_key`, `started_at`, `duration_s`, `comment`.
///
/// Note: the new worklog gets a fresh `jira_worklog_id`. The original deleted
/// id stays gone — Jira does not support resurrecting by id, only POSTing a
/// new replacement. The audit entry's `source_audit_id` preserves the link
/// back to the original delete so the UI can show "Obnoveno" badge against
/// the right history row.
#[tauri::command]
pub async fn restore_deleted_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    audit_id: i64,
) -> Result<WorklogRow, String> {
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
    let saved = jira::reconstruct::restore_deleted_worklog(&client, &state.db, audit_id)
        .await
        .map_err(reconstruct_err_to_string)?;
    let _ = app.emit("worklog-created", &saved);
    Ok(saved)
}

/// Phase 16 — revert an `update` by pushing the old `before_json` values back
/// to Jira as a fresh update.
///
/// Returns an error if the worklog has been deleted in Jira since the update
/// happened — there's nothing to update in that case (the user should use
/// "Obnovit v Jira" against the delete audit entry instead).
#[tauri::command]
pub async fn revert_worklog_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    audit_id: i64,
) -> Result<WorklogRow, String> {
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
    let after = jira::reconstruct::revert_worklog_update(&client, &state.db, audit_id)
        .await
        .map_err(reconstruct_err_to_string)?;
    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

/// Phase 16 — replay a previously-failed audit action.
///
/// The strategy depends on the original op:
/// - `create` → POST a new worklog using the `after_json` snapshot.
/// - `update` → PUT using `after_json`.
/// - `delete` / `sync_tombstone` → re-issue the Jira DELETE.
/// - other ops → return an error.
///
/// Records a new audit entry with op = `retry` linked to the source.
#[tauri::command]
pub async fn retry_failed_audit_action(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    audit_id: i64,
) -> Result<serde_json::Value, String> {
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
    let result = jira::reconstruct::retry_failed_audit_action(&client, &state.db, audit_id)
        .await
        .map_err(reconstruct_err_to_string)?;
    // Emit the corresponding event so the UI invalidates the right queries.
    match result.get("op").and_then(|v| v.as_str()) {
        Some("create") => {
            let _ = app.emit("worklog-created", &result);
        }
        Some("update") => {
            let _ = app.emit("worklog-updated", &result);
        }
        Some("delete") => {
            let _ = app.emit("worklog-delete-committed", &result);
        }
        _ => {}
    }
    Ok(result)
}

// -----------------------------------------------------------------------------
// Phase 18A — unassigned timer + local-only delete (Items 4, 7)
// -----------------------------------------------------------------------------

/// Assign an issue to a previously-unassigned worklog (one that was stopped
/// without a selected issue). Pushes a fresh POST to the provider so the
/// worklog becomes "real", links the provider id locally, and clears
/// `pending_assignment`. Dispatches by issue key prefix.
#[tauri::command]
pub async fn assign_worklog_issue(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: i64,
    issue_key: String,
) -> Result<WorklogRow, String> {
    if issue_key.trim().is_empty() {
        return Err("Klíč úkolu nesmí být prázdný".into());
    }
    let before = cache::worklogs::get_by_id(&state.db, worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;
    if !before.pending_assignment {
        return Err("Záznam již má přiřazený úkol".into());
    }

    if freelo::is_freelo_key(&issue_key) {
        // Freelo path: build a work-report and link its id locally.
        let (_, client) = resolve_client_for_issue(&state, &issue_key)?;
        let (client, cfg) = match client {
            ProviderClient::Freelo(c, cfg) => (c, cfg),
            _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
        };
        let user_id = cfg
            .sync_user_id
            .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;
        let saved = match freelo::ops::add_work_report(
            &client,
            &state.db,
            &issue_key,
            before.started_at.saturating_mul(1000),
            before.duration_s,
            before.comment.as_deref(),
            0,
            user_id,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                audit_failure(
                    &state.db,
                    AuditOp::Update,
                    Some(&issue_key),
                    None,
                    Some(&before),
                    &e.to_string(),
                );
                return Err(format!("Freelo: {e}"));
            }
        };
        // The freelo upsert created a NEW row keyed by freelo:N. We now need
        // to remove the old (local-only) pending-assignment row to avoid
        // having two rows for the same logical entry.
        let _ = cache::worklogs::delete_local_only(&state.db, worklog_id);
        audit_success(
            &state.db,
            AuditOp::Update,
            Some(&issue_key),
            saved.jira_worklog_id.as_deref(),
            Some(&before),
            Some(&saved),
        );
        let _ = app.emit("worklog-updated", &saved);
        return Ok(saved);
    }

    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;

    let started_dt = Utc
        .timestamp_opt(before.started_at, 0)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;

    let resp = match client
        .add_worklog(
            &issue_key,
            started_dt,
            before.duration_s,
            before.comment.as_deref(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_failure(
                &state.db,
                AuditOp::Update,
                Some(&issue_key),
                None,
                Some(&before),
                &e.to_string(),
            );
            return Err(format!("Jira: {e}"));
        }
    };

    let (issue_id, summary) =
        match cache::issues::get_by_key(&state.db, &issue_key).map_err(|e| e.to_string())? {
            Some(row) => (row.issue_id, Some(row.summary)),
            None => (resp.issue_id.clone(), None),
        };

    cache::worklogs::assign_issue(
        &state.db,
        worklog_id,
        &issue_key,
        issue_id.as_deref(),
        summary.as_deref(),
        Some(&resp.id),
    )
    .map_err(|e| e.to_string())?;

    let after = cache::worklogs::get_by_id(&state.db, worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po přiřazení".to_string())?;

    audit_success(
        &state.db,
        AuditOp::Update,
        Some(&issue_key),
        Some(&resp.id),
        Some(&before),
        Some(&after),
    );

    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

/// Delete a worklog that exists only locally (no `jira_worklog_id`). Used by
/// the UI for two cases:
/// 1. Pending-assignment rows the user no longer wants to assign.
/// 2. Rows that failed to sync to Jira (e.g. < 60s rejection) so there's
///    nothing to delete remotely.
///
/// Refuses to delete rows that DO have a `jira_worklog_id` — those must go
/// through the full `delete_worklog` flow.
#[tauri::command]
pub async fn delete_local_only_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    worklog_id: i64,
) -> Result<(), String> {
    let before = cache::worklogs::get_by_id(&state.db, worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;
    if before.jira_worklog_id.is_some() {
        return Err(
            "Tento záznam je synchronizovaný s Jirou — použijte standardní smazání.".into(),
        );
    }
    cache::worklogs::delete_local_only(&state.db, worklog_id).map_err(|e| e.to_string())?;

    audit_success(
        &state.db,
        AuditOp::Delete,
        Some(&before.issue_key),
        None,
        Some(&before),
        None,
    );

    let _ = app.emit("worklog-deleted", &before);
    Ok(())
}

/// Background task body: commit a pending delete if it's still pending.
///
/// Public so the startup recovery in `lib.rs` can call the same code path
/// for orphaned pending deletes left behind after a crash. Dispatches by
/// issue key prefix (Freelo vs Jira).
pub async fn commit_pending_delete(
    app: &tauri::AppHandle,
    state: &AppState,
    local_id: i64,
    issue_key: &str,
    worklog_id: &str,
) {
    // Re-read the row; if pending_delete_at is cleared (user undid), no-op.
    let row = match cache::worklogs::get_by_id(&state.db, local_id) {
        Ok(Some(r)) => r,
        _ => return,
    };
    if row.pending_delete_at.is_none() {
        return; // User pressed undo.
    }
    if row.tombstoned_at.is_some() {
        return; // Already committed by an earlier task.
    }

    // Freelo branch.
    if freelo::is_freelo_key(issue_key) {
        let wr_id = match freelo::parse_worklog_id(worklog_id) {
            Some(id) => id,
            None => {
                let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
                audit_failure(
                    &state.db,
                    AuditOp::Delete,
                    Some(issue_key),
                    Some(worklog_id),
                    Some(&row),
                    "Neplatné Freelo id záznamu",
                );
                return;
            }
        };
        // Resolve the live freelo client.
        let client = {
            let conns = state.connections.read().unwrap();
            conns.iter().find_map(|c| match &c.client {
                ProviderClient::Freelo(client, _) => Some(client.clone()),
                _ => None,
            })
        };
        let client = match client {
            Some(c) => c,
            None => {
                let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
                audit_failure(
                    &state.db,
                    AuditOp::Delete,
                    Some(issue_key),
                    Some(worklog_id),
                    Some(&row),
                    "Freelo klient není nakonfigurován",
                );
                return;
            }
        };
        let now_s = Utc::now().timestamp();
        match freelo::ops::delete_work_report(&client, wr_id).await {
            Ok(()) => {
                let _ = cache::worklogs::mark_tombstoned(&state.db, local_id, now_s);
                audit_success(
                    &state.db,
                    AuditOp::Delete,
                    Some(issue_key),
                    Some(worklog_id),
                    Some(&row),
                    None,
                );
                let _ = app.emit("worklog-delete-committed", worklog_id.to_string());
            }
            Err(e) => {
                let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
                audit_failure(
                    &state.db,
                    AuditOp::Delete,
                    Some(issue_key),
                    Some(worklog_id),
                    Some(&row),
                    &e.to_string(),
                );
                let _ = app.emit("worklog-error", e.to_string());
            }
        }
        return;
    }

    // Jira branch (original behaviour).
    let client = match state.jira_client_cloned() {
        Some(c) => c,
        None => {
            // No client: clear the pending flag so the UI can recover.
            let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
            audit_failure(
                &state.db,
                AuditOp::Delete,
                Some(issue_key),
                Some(worklog_id),
                Some(&row),
                "Jira klient není nakonfigurován",
            );
            return;
        }
    };

    let now_s = Utc::now().timestamp();
    match client.delete_worklog(issue_key, worklog_id).await {
        Ok(()) | Err(JiraError::WorklogNotFound) => {
            // Treat 404 as "already gone, OK".
            let _ = cache::worklogs::mark_tombstoned(&state.db, local_id, now_s);
            audit_success(
                &state.db,
                AuditOp::Delete,
                Some(issue_key),
                Some(worklog_id),
                Some(&row),
                None,
            );
            let _ = app.emit("worklog-delete-committed", worklog_id.to_string());
        }
        Err(e) => {
            // Clear the pending flag so the row reappears in the UI and the
            // user can retry. Audit the failure.
            let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
            audit_failure(
                &state.db,
                AuditOp::Delete,
                Some(issue_key),
                Some(worklog_id),
                Some(&row),
                &e.to_string(),
            );
            let _ = app.emit("worklog-error", e.to_string());
        }
    }
}
