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
    // `with_author` is no longer honoured at the SQL layer — the application
    // is single-user and every row in the DB belongs to "me". The argument
    // is kept on the IPC surface for backwards compatibility with the FE.
    let _ = with_author;
    cache::worklogs::for_date_range(&state.db, from_unix_s, to_unix_s).map_err(|e| e.to_string())
}

/// Result payload of [`refresh_all`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshAllResult {
    pub issues: usize,
    pub worklogs: usize,
}

/// Co se má syncovat. `Full` stáhne dlouhou historii — typicky první
/// spuštění nebo manuální "Stáhnout celou historii". `Incremental` jede
/// rolling 30denní okno worklogů — bleskové a stačí na běžný provoz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Full,
    Incremental,
}

impl SyncMode {
    pub fn from_optional_str(s: Option<&str>) -> Self {
        match s {
            Some("full") => Self::Full,
            // Default i pro neznámý string — incremental je levný a
            // safe-by-default; full musí být explicitně vyžádán.
            _ => Self::Incremental,
        }
    }

    fn worklog_window_days(self) -> i64 {
        match self {
            // Full = 10 let zpět = v praxi vše. Incremental = rolling 30 dní,
            // dle data záznamu (`started_at` / `worklogDate`).
            Self::Full => 3650,
            Self::Incremental => 30,
        }
    }
}

/// Klíč v `app_settings` pro persistovaný posledně viděný error per connection.
/// Hodnota je JSON `{ phase, error, at }`; `at` je unix sec.
fn sync_error_key(connection_id: i64) -> String {
    format!("last_sync_error:{connection_id}")
}

fn store_sync_error(db: &cache::Db, connection_id: i64, phase: &str, error: &str) {
    let payload = serde_json::json!({
        "phase": phase,
        "error": error,
        "at": Utc::now().timestamp(),
    });
    let _ = cache::settings::set(db, &sync_error_key(connection_id), &payload.to_string());
}

fn clear_sync_error(db: &cache::Db, connection_id: i64) {
    let _ = cache::settings::remove(db, &sync_error_key(connection_id));
}

/// Sync issues + worklogs pro **jedno** připojení a stáhne progress eventy.
/// Vrací `(issues_count, worklogs_count)`. Chyby fáze emituje s `error: <msg>`
/// a vrací 0 pro tu fázi (worklog sync se přeskočí, když issues fail).
///
/// Veřejné, protože ho používá i background auto-sync v `lib.rs` —
/// předtím měl vlastní inline loop, který neuměl vyčistit
/// `last_sync_error`. Sdílením této funkce držíme store/clear logiku na
/// jednom místě.
/// Zaznamenat audit záznam o dokončeném syncu do `sync_runs`. Volá se na
/// konci `sync_one_connection`; jeden řádek per dokončený běh.
#[allow(clippy::too_many_arguments)]
fn record_sync_run(
    db: &cache::Db,
    connection_id: i64,
    connection_name: &str,
    provider: &str,
    mode: SyncMode,
    started_at: i64,
    finished_at: i64,
    issues_count: usize,
    worklogs_count: usize,
) {
    // Error pro tenhle běh — pokud existuje `last_sync_error:{id}` PO běhu
    // (tj. něco padlo a my jsme to nevyčistili), znamená to že běh failed.
    let (error_phase, error_message) = read_sync_error(db, connection_id);
    let row = cache::sync_log::SyncRunRow {
        id: None,
        connection_id: Some(connection_id),
        connection_name: Some(connection_name.to_string()),
        provider: Some(provider.to_string()),
        mode: match mode {
            SyncMode::Full => "full".into(),
            SyncMode::Incremental => "incremental".into(),
        },
        started_at,
        finished_at,
        issues_count: issues_count as i64,
        worklogs_count: worklogs_count as i64,
        error_phase,
        error_message,
    };
    let _ = cache::sync_log::record(db, &row);
}

fn read_sync_error(db: &cache::Db, connection_id: i64) -> (Option<String>, Option<String>) {
    let raw = match cache::settings::get(db, &sync_error_key(connection_id)) {
        Ok(Some(v)) => v,
        _ => return (None, None),
    };
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    let phase = v
        .get("phase")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    let error = v
        .get("error")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());
    (phase, error)
}

pub async fn sync_one_connection(
    app: &tauri::AppHandle,
    db: &cache::Db,
    conn: crate::state::ActiveConnection,
    idx: usize,
    total_conns: usize,
    mode: SyncMode,
) -> (usize, usize) {
    let conn_id = conn.id;
    let conn_name = conn.name.clone();
    let provider = match &conn.client {
        ProviderClient::Jira(_) => "jira",
        ProviderClient::Freelo(_, _) => "freelo",
    };
    let today = Local::now().date_naive();
    let from = today - Duration::days(mode.worklog_window_days());
    let started_at = Utc::now().timestamp();

    let emit = |phase: &str, count: Option<usize>, error: Option<&str>| {
        let _ = app.emit(
            "auto-sync-progress",
            serde_json::json!({
                "phase": phase,
                "current": idx + 1,
                "total": total_conns,
                "connection_id": conn_id,
                "connection_name": conn_name,
                "provider": provider,
                "count": count,
                "error": error,
                "mode": match mode {
                    SyncMode::Full => "full",
                    SyncMode::Incremental => "incremental",
                },
            }),
        );
    };

    emit("connection", None, None);

    // `issues_n` se nastaví v každé větvi match-e (oba providers); init = 0
    // jen pro `record_sync_run` v chybové cestě, kdy match vrátí brzy.
    #[allow(unused_assignments)]
    let mut issues_n = 0usize;
    let mut worklogs_n = 0usize;
    let mut any_error = false;

    match conn.client {
        ProviderClient::Jira(client) => {
            emit("issues", None, None);
            match jira::sync_issues_from_jira(&client, db, conn_id).await {
                Ok(n) => {
                    issues_n = n;
                    emit("issues", Some(n), None);
                }
                Err(e) => {
                    let msg = e.to_string();
                    store_sync_error(db, conn_id, "issues", &msg);
                    emit("issues", None, Some(&msg));
                    return (0, 0);
                }
            }

            if client.myself().await.is_ok() {
                emit("worklogs", None, None);
                match jira::worklog_sync::sync_worklogs_for_range(&client, db, conn_id, from, today)
                    .await
                {
                    Ok(n) => {
                        worklogs_n = n;
                        emit("worklogs", Some(n), None);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        store_sync_error(db, conn_id, "worklogs", &msg);
                        emit("worklogs", None, Some(&msg));
                        any_error = true;
                    }
                }
            }
        }
        ProviderClient::Freelo(client, cfg) => {
            emit("issues", None, None);
            match freelo::sync::sync_issues_for_connection(
                &client,
                db,
                conn_id,
                &cfg.selected_project_ids,
            )
            .await
            {
                Ok(n) => {
                    issues_n = n;
                    emit("issues", Some(n), None);
                }
                Err(e) => {
                    let msg = e.to_string();
                    store_sync_error(db, conn_id, "issues", &msg);
                    emit("issues", None, Some(&msg));
                    return (0, 0);
                }
            }

            if let Some(user_id) = cfg.sync_user_id {
                emit("worklogs", None, None);
                match freelo::sync::sync_worklogs_for_range(
                    &client,
                    db,
                    conn_id,
                    user_id,
                    from,
                    today,
                    &cfg.selected_project_ids,
                )
                .await
                {
                    Ok(n) => {
                        worklogs_n = n;
                        emit("worklogs", Some(n), None);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        store_sync_error(db, conn_id, "worklogs", &msg);
                        emit("worklogs", None, Some(&msg));
                        any_error = true;
                    }
                }
            }
        }
    }

    // Když všechny fáze proběhly bez chyby, vyčistíme persistovaný error
    // — od teď je connection "zdravá". Pokud cokoli padlo, persistujeme to
    // (už jsme to udělali výš) a necháváme tam.
    if !any_error {
        clear_sync_error(db, conn_id);
    }

    // Zapsat audit záznam do sync_runs (pro UI „Historie synchronizací").
    record_sync_run(
        db,
        conn_id,
        &conn_name,
        provider,
        mode,
        started_at,
        Utc::now().timestamp(),
        issues_n,
        worklogs_n,
    );

    (issues_n, worklogs_n)
}

/// Sync issues + worklogs across all enabled connections.
///
/// `mode = "full"` táhne 10 let historie; cokoli jiného (`"incremental"` nebo
/// nezadáno) jede 30denní rolling okno. Per-connection chyby tolerujeme.
#[tauri::command]
pub async fn refresh_all(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: Option<String>,
) -> Result<RefreshAllResult, String> {
    let mode = SyncMode::from_optional_str(mode.as_deref());

    let mut total_issues = 0usize;
    let mut total_worklogs = 0usize;

    let active = state
        .connections
        .read()
        .expect("AppState.connections RwLock poisoned")
        .clone();
    let total_conns = active.len();

    for (idx, conn) in active.into_iter().enumerate() {
        let (i_n, w_n) = sync_one_connection(&app, &state.db, conn, idx, total_conns, mode).await;
        total_issues += i_n;
        total_worklogs += w_n;
    }

    // Legacy single-Jira shim: bez multi-connection rows, ale s legacy
    // klientem v paměti — použijeme connection_id = 0 jako sentinel.
    if state
        .connections
        .read()
        .expect("AppState.connections RwLock poisoned")
        .is_empty()
    {
        if let Some(client) = state.jira_client_cloned() {
            let conn_id: i64 = 0;
            let today = Local::now().date_naive();
            let from = today - Duration::days(mode.worklog_window_days());
            if let Ok(n) = jira::sync_issues_from_jira(&client, &state.db, conn_id).await {
                total_issues += n;
            }
            if client.myself().await.is_ok() {
                if let Ok(n) = jira::worklog_sync::sync_worklogs_for_range(
                    &client, &state.db, conn_id, from, today,
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
    let _ = app.emit(
        "auto-sync-complete",
        serde_json::json!({
            "issues": total_issues,
            "worklogs": total_worklogs,
        }),
    );
    let _ = app.emit("cache-refreshed", total_issues);
    let _ = app.emit("worklogs-refreshed", total_worklogs);
    Ok(result)
}

/// Split worklog: existující záznam rozdělit na dvě části — první kus
/// zůstane na původním úkolu, druhý kus dostane nový `new_issue_key`.
///
/// Limitace MVP: funguje **jen pro lokální worklogy** (žádný `remote_id`).
/// Pro synced záznamy by bylo nutné DELETE + 2× POST s rollbackem, což je
/// větší úkol než inline split.
#[tauri::command]
pub async fn split_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    local_id: i64,
    split_at_ms: i64,
    new_issue_key: Option<String>,
) -> Result<Vec<WorklogRow>, String> {
    let before = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;
    if before.remote_id.is_some() || before.is_synced {
        return Err("Split je zatím podporován jen pro lokální (nesyncované) záznamy.".into());
    }

    let split_at_s = split_at_ms / 1000;
    if split_at_s <= before.started_at || split_at_s >= before.ended_at {
        return Err("Bod rozdělení musí být uvnitř záznamu".into());
    }

    // 1) zkrátíme původní záznam.
    cache::worklogs::update_fields(
        &state.db,
        local_id,
        before.issue_key.as_deref(),
        before.description.as_deref(),
        before.started_at,
        split_at_s,
        None,
    )
    .map_err(|e| e.to_string())?;

    // 2) vytvoříme druhý kus.
    let new_key = new_issue_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let now = Utc::now().timestamp();
    let connection_id = match new_key.as_deref() {
        Some(k) => {
            cache::issues::get_connection_id_by_key(&state.db, k).map_err(|e| e.to_string())?
        }
        None => before.connection_id,
    };
    let second = WorklogRow {
        id: None,
        connection_id,
        issue_key: new_key,
        description: before.description.clone(),
        started_at: split_at_s,
        ended_at: before.ended_at,
        logged_at: now,
        updated_at: now,
        is_synced: false,
        synced_at: None,
        remote_id: None,
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    let new_id = cache::worklogs::record(&state.db, &second).map_err(|e| e.to_string())?;

    let first_after = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po split".to_string())?;
    let second_after = cache::worklogs::get_by_id(&state.db, new_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Druhý záznam zmizel po split".to_string())?;

    let _ = app.emit(
        "worklog-split",
        serde_json::json!({
            "first_id": local_id,
            "second_id": new_id,
        }),
    );
    Ok(vec![first_after, second_after])
}

/// Historie syncov — read-only seznam pro UI "Historie synchronizací".
#[tauri::command]
pub async fn list_sync_runs(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<cache::sync_log::SyncRunRow>, String> {
    let limit = limit.unwrap_or(100).clamp(1, 1000);
    cache::sync_log::list_recent(&state.db, limit).map_err(|e| e.to_string())
}

/// DTO: poslední neúspěšná fáze syncu pro danou connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncErrorEntry {
    pub connection_id: i64,
    pub phase: String,
    pub error: String,
    pub at: i64,
}

/// Vrátí seznam connections s persistovaným posledním sync errorem.
/// Když je seznam prázdný, nic nepadlo. Po úspěšném syncu connection
/// automaticky zmizí z výsledku — `sync_one_connection` ji při úspěchu
/// vyčistí.
#[tauri::command]
pub async fn get_sync_errors(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<SyncErrorEntry>, String> {
    let rows = cache::settings::list_with_prefix(&state.db, "last_sync_error:")
        .map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for (key, value) in rows {
        let connection_id: i64 = match key.strip_prefix("last_sync_error:") {
            Some(s) => s.parse().unwrap_or(0),
            None => continue,
        };
        let parsed: serde_json::Value = match serde_json::from_str(&value) {
            Ok(v) => v,
            Err(_) => continue,
        };
        out.push(SyncErrorEntry {
            connection_id,
            phase: parsed
                .get("phase")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            error: parsed
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            at: parsed.get("at").and_then(|v| v.as_i64()).unwrap_or(0),
        });
    }
    Ok(out)
}

/// Sync issues + worklogs jen pro jednu vybranou connection.
///
/// Z UI volá tlačítko „Stáhnout celou historii" v nastavení integrace
/// (`mode = "full"`). Hodí se i pro per-account incremental refresh když
/// uživatel chce ručně zatáhnout změny jen v jedné Jiře/Freelu, aniž by
/// dráždil ostatní providery.
#[tauri::command]
pub async fn refresh_connection(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: i64,
    mode: Option<String>,
) -> Result<RefreshAllResult, String> {
    let mode = SyncMode::from_optional_str(mode.as_deref());

    let conn = {
        let conns = state
            .connections
            .read()
            .expect("AppState.connections RwLock poisoned");
        conns
            .iter()
            .find(|c| c.id == connection_id && c.enabled)
            .cloned()
            .ok_or_else(|| "Připojení nenalezeno nebo není aktivní".to_string())?
    };

    let (issues_n, worklogs_n) = sync_one_connection(&app, &state.db, conn, 0, 1, mode).await;

    let result = RefreshAllResult {
        issues: issues_n,
        worklogs: worklogs_n,
    };
    let _ = app.emit(
        "auto-sync-complete",
        serde_json::json!({
            "issues": issues_n,
            "worklogs": worklogs_n,
        }),
    );
    let _ = app.emit("cache-refreshed", issues_n);
    let _ = app.emit("worklogs-refreshed", worklogs_n);
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
    let conns = state
        .connections
        .read()
        .expect("AppState.connections RwLock poisoned");
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
        return create_freelo_worklog(
            app,
            state,
            issue_key,
            started_at_ms,
            duration_seconds,
            comment,
        )
        .await;
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

    let connection_id = cache::issues::get_connection_id_by_key(&state.db, &issue_key)
        .map_err(|e| e.to_string())?;

    let started_at_s = started_at_ms / 1000;
    let now_s = Utc::now().timestamp();
    let row = WorklogRow {
        id: None,
        connection_id,
        issue_key: Some(issue_key.clone()),
        description: comment.clone(),
        started_at: started_at_s,
        ended_at: started_at_s.saturating_add(duration_seconds.max(0)),
        logged_at: now_s,
        updated_at: now_s,
        is_synced: true,
        synced_at: Some(now_s),
        remote_id: Some(resp.id.clone()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    let local_id =
        cache::worklogs::upsert_from_remote(&state.db, &row).map_err(|e| e.to_string())?;
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
        saved.remote_id.as_deref(),
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
    let next_duration = new_duration_seconds.unwrap_or(before.duration_s());
    let next_description = match new_comment {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => before.description.clone(),
    };
    let next_issue_key = match new_issue_key {
        Some(k) if k.is_empty() => None,
        Some(k) => Some(k),
        None => before.issue_key.clone(),
    };
    let next_ended_at = next_started_at.saturating_add(next_duration.max(0));

    cache::worklogs::update_fields(
        &state.db,
        local_id,
        next_issue_key.as_deref(),
        next_description.as_deref(),
        next_started_at,
        next_ended_at,
        None,
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

    let before = cache::worklogs::get_by_remote_id_any(&state.db, &worklog_id)
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
    let new_duration = new_duration_seconds.unwrap_or(before.duration_s());
    let new_description_for_db = match &new_comment {
        Some(s) if s.trim().is_empty() => None,
        Some(s) => Some(s.clone()),
        None => before.description.clone(),
    };
    let now_s = Utc::now().timestamp();
    let new_ended = new_started.saturating_add(new_duration.max(0));

    cache::worklogs::update_fields(
        &state.db,
        local_id,
        Some(&issue_key),
        new_description_for_db.as_deref(),
        new_started,
        new_ended,
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
    let before = cache::worklogs::get_by_remote_id_any(&state.db, &worklog_id)
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
    let before = cache::worklogs::get_by_remote_id_any(&state.db, &worklog_id)
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
    let before = cache::worklogs::get_by_remote_id_any(&state.db, &worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())?;
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;

    cache::worklogs::clear_pending_delete(&state.db, local_id).map_err(|e| e.to_string())?;

    audit_success(
        &state.db,
        AuditOp::Undo,
        before.issue_key.as_deref(),
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

    let before = cache::worklogs::get_by_remote_id_any(&state.db, &old_worklog_id)
        .map_err(|e| e.to_string())?;

    let args = MoveWorklogArgs {
        old_issue_key: &old_issue_key,
        old_worklog_id: &old_worklog_id,
        new_issue_key: &new_issue_key,
        started: started_dt,
        time_spent_seconds: duration_seconds,
        comment: comment.as_deref(),
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

fn freelo_reconstruct_err_to_string(e: freelo::reconstruct::ReconstructError) -> String {
    match e {
        freelo::reconstruct::ReconstructError::Freelo(fe) => format!("Freelo: {fe}"),
        other => other.to_string(),
    }
}

/// Audit entries jsou cross-provider. Rozlišíme jen podle issue_key prefixu —
/// `FREELO-` → Freelo. Kdyby v audit entry chyběl issue_key, padáme zpět na
/// snapshot (`before_json` / `after_json`) a snažíme se odtud vyčíst klíč.
fn audit_is_freelo(db: &cache::Db, audit_id: i64) -> bool {
    let Ok(Some(entry)) = cache::audit::get_by_id(db, audit_id) else {
        return false;
    };
    if let Some(k) = entry.issue_key.as_deref() {
        if freelo::is_freelo_key(k) {
            return true;
        }
        if !k.is_empty() {
            return false;
        }
    }
    // Fallback — vytáhni issue_key z JSON snapshotu.
    for src in [entry.before_json.as_deref(), entry.after_json.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(src) {
            if let Some(k) = v.get("issue_key").and_then(|x| x.as_str()) {
                return freelo::is_freelo_key(k);
            }
        }
    }
    false
}

/// Vrátí first-active Freelo klient ze state pro audit reconstruct calls.
fn first_freelo_client(
    state: &tauri::State<'_, AppState>,
) -> Result<crate::freelo::client::FreeloClient, String> {
    let conns = state
        .connections
        .read()
        .expect("AppState.connections RwLock poisoned");
    conns
        .iter()
        .find_map(|c| match &c.client {
            ProviderClient::Freelo(client, _) => Some(client.clone()),
            _ => None,
        })
        .ok_or_else(|| "Freelo klient není nakonfigurován".to_string())
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
    let saved = if audit_is_freelo(&state.db, audit_id) {
        let client = first_freelo_client(&state)?;
        freelo::reconstruct::restore_deleted_worklog(&client, &state.db, audit_id)
            .await
            .map_err(freelo_reconstruct_err_to_string)?
    } else {
        let client = state
            .jira_client_cloned()
            .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
        jira::reconstruct::restore_deleted_worklog(&client, &state.db, audit_id)
            .await
            .map_err(reconstruct_err_to_string)?
    };
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
    let after = if audit_is_freelo(&state.db, audit_id) {
        let client = first_freelo_client(&state)?;
        freelo::reconstruct::revert_worklog_update(&client, &state.db, audit_id)
            .await
            .map_err(freelo_reconstruct_err_to_string)?
    } else {
        let client = state
            .jira_client_cloned()
            .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
        jira::reconstruct::revert_worklog_update(&client, &state.db, audit_id)
            .await
            .map_err(reconstruct_err_to_string)?
    };
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
    let result = if audit_is_freelo(&state.db, audit_id) {
        let client = first_freelo_client(&state)?;
        freelo::reconstruct::retry_failed_audit_action(&client, &state.db, audit_id)
            .await
            .map_err(freelo_reconstruct_err_to_string)?
    } else {
        let client = state
            .jira_client_cloned()
            .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
        jira::reconstruct::retry_failed_audit_action(&client, &state.db, audit_id)
            .await
            .map_err(reconstruct_err_to_string)?
    };
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

/// Push a local-only worklog upstream (Jira or Freelo, dispatched by issue
/// key prefix). Used by the "Synchronizovat" action on rows that already
/// have an `issue_key` but no upstream `jira_worklog_id` — typically because
/// the original POST failed (network blip, 429, sub-minute duration, etc.).
///
/// Differs from [`assign_worklog_issue`] in that it does NOT require
/// `pending_assignment`; it operates on any row whose remote id is null.
/// On success the row's `jira_worklog_id` is filled and `worklog-updated` is
/// emitted so the UI removes the "⚠ lokální" chip.
#[tauri::command]
pub async fn push_local_worklog(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    local_id: i64,
) -> Result<WorklogRow, String> {
    let before = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;
    if before.is_synced || before.remote_id.is_some() {
        return Err("Záznam je již synchronizovaný".into());
    }
    let issue_key = before
        .issue_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| "Záznam nemá přiřazený úkol — nejprve ho přiřaďte".to_string())?;

    if freelo::is_freelo_key(&issue_key) {
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
            before.started_at.saturating_mul(1000),
            before.duration_s(),
            before.description.as_deref(),
            conn_id,
            user_id,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => return Err(format!("Freelo: {e}")),
        };
        let _ = cache::worklogs::delete_local_only(&state.db, local_id);
        let _ = app.emit("worklog-updated", &saved);
        return Ok(saved);
    }

    // Jira path.
    let client = state
        .jira_client_cloned()
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())?;
    let started_dt = Utc
        .timestamp_opt(before.started_at, 0)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;
    let resp = client
        .add_worklog(
            &issue_key,
            started_dt,
            before.duration_s(),
            before.description.as_deref(),
        )
        .await
        .map_err(|e| format!("Jira: {e}"))?;

    let connection_id = cache::issues::get_connection_id_by_key(&state.db, &issue_key)
        .map_err(|e| e.to_string())?;
    cache::worklogs::assign_issue(
        &state.db,
        local_id,
        connection_id,
        &issue_key,
        Some(&resp.id),
    )
    .map_err(|e| e.to_string())?;
    let after = cache::worklogs::get_by_id(&state.db, local_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po synchronizaci".to_string())?;
    let _ = app.emit("worklog-updated", &after);
    Ok(after)
}

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
    if before.issue_key.is_some() {
        return Err("Záznam již má přiřazený úkol".into());
    }

    if freelo::is_freelo_key(&issue_key) {
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
            before.started_at.saturating_mul(1000),
            before.duration_s(),
            before.description.as_deref(),
            conn_id,
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
        let _ = cache::worklogs::delete_local_only(&state.db, worklog_id);
        audit_success(
            &state.db,
            AuditOp::Update,
            Some(&issue_key),
            saved.remote_id.as_deref(),
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
            before.duration_s(),
            before.description.as_deref(),
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

    let connection_id = cache::issues::get_connection_id_by_key(&state.db, &issue_key)
        .map_err(|e| e.to_string())?;

    cache::worklogs::assign_issue(
        &state.db,
        worklog_id,
        connection_id,
        &issue_key,
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
    if before.remote_id.is_some() || before.is_synced {
        return Err(
            "Tento záznam je synchronizovaný s providerem — použijte standardní smazání.".into(),
        );
    }
    cache::worklogs::delete_local_only(&state.db, worklog_id).map_err(|e| e.to_string())?;

    audit_success(
        &state.db,
        AuditOp::Delete,
        before.issue_key.as_deref(),
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
            let conns = state
                .connections
                .read()
                .expect("AppState.connections RwLock poisoned");
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
