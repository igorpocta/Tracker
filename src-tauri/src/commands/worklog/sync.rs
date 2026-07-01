//! Sync orchestration: pulls issues + worklogs from each enabled connection,
//! tracks per-connection error state, and exposes the audit history of past
//! sync runs.

use chrono::{Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache;
use crate::jira;
use crate::state::AppState;
use crate::worklog_service::SyncOutcome;

/// Outcome of syncing a single connection. P2-2: callers must be able to tell
/// a clean run apart from a partial failure (some phases failed/skipped) and a
/// full failure (the issues phase never completed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncRunStatus {
    Success,
    Partial,
    Failed,
}

/// Result payload of [`refresh_all`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshAllResult {
    pub issues: usize,
    pub worklogs: usize,
    /// P2-2: per-connection outcome counts + an aggregate status so the UI can
    /// surface "vše OK" / "část selhala" / "vše selhalo" instead of always
    /// reporting success.
    pub succeeded: usize,
    pub partial: usize,
    pub failed: usize,
    pub status: SyncRunStatus,
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

/// Apply a worklog [`SyncOutcome`] to persistent error state and return
/// the count for the run stats. Pure helper so the readiness logic stays
/// unit-testable without spinning up Tauri.
///
/// Contract:
///   - `Ok { count }` — leave persistent error state alone; the
///     end-of-run cleanup in [`sync_one_connection`] (gated by
///     `any_error == false`) handles clearing.
///   - `Skipped { reason }` — persist as a `worklogs_skipped` phase tag
///     under the same `last_sync_error:N` key so the UI's "last error"
///     panel surfaces the skip as a warning. Returns 0.
fn apply_worklog_outcome(db: &cache::Db, conn_id: i64, outcome: SyncOutcome) -> usize {
    match outcome {
        SyncOutcome::Ok { count } => count,
        SyncOutcome::Skipped { reason } => {
            store_sync_error(db, conn_id, "worklogs_skipped", &reason);
            0
        }
    }
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
) -> (usize, usize, SyncRunStatus) {
    let conn_id = conn.id;
    let conn_name = conn.name.clone();
    // Phase B4: dispatch through the WorklogService trait. The provider
    // tag and the two sync calls both go through `&dyn WorklogService` so
    // adding Toggl/Clockify/… is one new variant + one new `impl
    // WorklogService` away.
    let svc = conn.client.as_service();
    let provider = svc.provider_name();
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

    let issues_n: usize;
    let mut worklogs_n = 0usize;
    let mut any_error = false;

    // ---- Issues ---------------------------------------------------------
    emit("issues", None, None);
    match svc.sync_issues(db, conn_id).await {
        Ok(n) => {
            issues_n = n;
            emit("issues", Some(n), None);
        }
        Err(e) => {
            let msg = e.to_string();
            store_sync_error(db, conn_id, "issues", &msg);
            emit("issues", None, Some(&msg));
            // Worklog phase is meaningless without a fresh issue catalog, so
            // we still short-circuit. P2-2: but record the failed run in
            // `sync_runs` (it reads the persisted error) so the history and
            // the aggregate status reflect that this connection failed —
            // previously an issues-phase failure left no audit trail at all.
            record_sync_run(
                db,
                conn_id,
                &conn_name,
                provider,
                mode,
                started_at,
                Utc::now().timestamp(),
                0,
                0,
            );
            return (0, 0, SyncRunStatus::Failed);
        }
    }

    // ---- Worklogs -------------------------------------------------------
    // The per-provider impl is responsible for its own readiness gate
    // (Jira: `myself()` round-trip; Freelo: cached `sync_user_id`). When
    // the gate fails the impl returns `SyncOutcome::Skipped { reason }`
    // — the orchestrator MUST treat that as a warning, NOT as Ok(0).
    // Treating it as Ok used to clear the persisted error and emit a
    // green "0 imported" event for a sync that never ran.
    emit("worklogs", None, None);
    match svc.sync_worklogs(db, conn_id, from, today).await {
        Ok(SyncOutcome::Ok { count }) => {
            worklogs_n = count;
            emit("worklogs", Some(count), None);
        }
        Ok(SyncOutcome::Skipped { reason }) => {
            // UI musí poznat skip jako varování, ne jako "OK 0":
            emit("worklogs", None, Some(&format!("skipped: {reason}")));
            worklogs_n = apply_worklog_outcome(db, conn_id, SyncOutcome::Skipped { reason });
            any_error = true;
        }
        Err(e) => {
            let msg = e.to_string();
            store_sync_error(db, conn_id, "worklogs", &msg);
            emit("worklogs", None, Some(&msg));
            any_error = true;
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

    let status = if any_error {
        SyncRunStatus::Partial
    } else {
        SyncRunStatus::Success
    };
    (issues_n, worklogs_n, status)
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
    let mut succeeded = 0usize;
    let mut partial = 0usize;
    let mut failed = 0usize;

    // Drain the backlog of locally-recorded-but-never-pushed worklogs
    // BEFORE the pull phase. Otherwise mark-and-sweep on a fresh pull
    // might race with rows that are about to land upstream.
    let flushed = crate::commands::worklog::flush_unsynced_worklogs(&app, &state).await;
    if flushed > 0 {
        tracing::info!("refresh_all: flushed {flushed} backlogged worklog(s)");
    }

    let active = state
        .connections
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let total_conns = active.len();

    for (idx, conn) in active.into_iter().enumerate() {
        let (i_n, w_n, status) =
            sync_one_connection(&app, &state.db, conn, idx, total_conns, mode).await;
        total_issues += i_n;
        total_worklogs += w_n;
        match status {
            SyncRunStatus::Success => succeeded += 1,
            SyncRunStatus::Partial => partial += 1,
            SyncRunStatus::Failed => failed += 1,
        }
    }

    // Legacy single-Jira shim: bez multi-connection rows, ale s legacy
    // klientem v paměti — použijeme connection_id = 0 jako sentinel.
    if state
        .connections
        .read()
        .unwrap_or_else(|e| e.into_inner())
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

    // P2-2: aggregate status. Any failure with no success at all → failure;
    // any failure/partial mixed with success → partial; otherwise success.
    let status = if failed > 0 && succeeded == 0 && partial == 0 {
        SyncRunStatus::Failed
    } else if failed > 0 || partial > 0 {
        SyncRunStatus::Partial
    } else {
        SyncRunStatus::Success
    };

    let result = RefreshAllResult {
        issues: total_issues,
        worklogs: total_worklogs,
        succeeded,
        partial,
        failed,
        status,
    };
    let _ = app.emit(
        "auto-sync-complete",
        serde_json::json!({
            "issues": total_issues,
            "worklogs": total_worklogs,
            "succeeded": succeeded,
            "partial": partial,
            "failed": failed,
            "status": status,
        }),
    );
    let _ = app.emit("cache-refreshed", total_issues);
    let _ = app.emit("worklogs-refreshed", total_worklogs);
    Ok(result)
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

/// Vrátí seznam connections s persistovaným posledním sync errorem
/// **nebo skipem** (`phase = "worklogs_skipped"`). Položka může
/// reprezentovat tvrdou chybu nebo "fáze se nespustila kvůli chybějící
/// konfiguraci". Klient si rozliší podle `phase`.
///
/// Po úspěšném syncu (žádný error, žádný skip) connection z výsledku
/// zmizí — `sync_one_connection` `clear_sync_error` zavolá jen když
/// všechny fáze proběhly bez problému.
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
        let conns = state.connections.read().unwrap_or_else(|e| e.into_inner());
        conns
            .iter()
            .find(|c| c.id == connection_id && c.enabled)
            .cloned()
            .ok_or_else(|| "Připojení nenalezeno nebo není aktivní".to_string())?
    };

    let (issues_n, worklogs_n, status) =
        sync_one_connection(&app, &state.db, conn, 0, 1, mode).await;

    let result = RefreshAllResult {
        issues: issues_n,
        worklogs: worklogs_n,
        succeeded: usize::from(status == SyncRunStatus::Success),
        partial: usize::from(status == SyncRunStatus::Partial),
        failed: usize::from(status == SyncRunStatus::Failed),
        status,
    };
    let _ = app.emit(
        "auto-sync-complete",
        serde_json::json!({
            "issues": issues_n,
            "worklogs": worklogs_n,
            "succeeded": result.succeeded,
            "partial": result.partial,
            "failed": result.failed,
            "status": status,
        }),
    );
    let _ = app.emit("cache-refreshed", issues_n);
    let _ = app.emit("worklogs-refreshed", worklogs_n);
    Ok(result)
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use crate::cache::Db;

    fn temp_db() -> Db {
        let tmp = tempfile::tempdir().unwrap();
        Db::open(&tmp.path().join("t.db")).unwrap()
    }

    #[test]
    fn skipped_outcome_persists_phase_and_reason() {
        let db = temp_db();
        store_sync_error(&db, 42, "worklogs", "boom");
        apply_worklog_outcome(
            &db,
            42,
            SyncOutcome::Skipped {
                reason: "freelo: chybí user id".into(),
            },
        );
        let (phase, err) = read_sync_error(&db, 42);
        assert_eq!(
            phase.as_deref(),
            Some("worklogs_skipped"),
            "skipped overwrites phase tag to worklogs_skipped"
        );
        assert_eq!(
            err.as_deref(),
            Some("freelo: chybí user id"),
            "skipped persists its own reason, the seeded 'boom' is gone"
        );
        // Critical: clear_sync_error was NOT called — the entry remains.
        assert!(
            err.is_some(),
            "skipped outcome must NOT leave the error state empty"
        );
    }

    #[test]
    fn apply_outcome_for_ok_does_not_itself_clear_error() {
        let db = temp_db();
        store_sync_error(&db, 42, "worklogs", "boom");
        let counted = apply_worklog_outcome(&db, 42, SyncOutcome::Ok { count: 0 });
        assert_eq!(counted, 0);
        // apply_worklog_outcome is a phase-level helper; end-of-run clearing
        // is sync_one_connection's responsibility (gated by `any_error == false`).
        // This test pins down that contract — the helper itself MUST NOT
        // touch the persisted error state on Ok.
        let (_, err) = read_sync_error(&db, 42);
        assert_eq!(
            err.as_deref(),
            Some("boom"),
            "apply_worklog_outcome is a phase-level helper; end-of-run \
             clearing is sync_one_connection's responsibility"
        );
    }
}
