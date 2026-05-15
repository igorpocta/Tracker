//! Audit log queries + Phase-16 reconstruction commands.
//!
//! The heavy lifting (Jira I/O, snapshot parsing, audit linkage) lives in
//! `jira::reconstruct` / `freelo::reconstruct`; the wrappers here just pick
//! the right provider client out of `AppState`, translate typed errors into
//! UI strings, and emit the corresponding `worklog-*` events.

use chrono::Utc;
use tauri::Emitter;

use crate::cache;
use crate::freelo;
use crate::jira;
use crate::state::{AppState, ProviderClient};

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
            ProviderClient::Freelo(svc) => Some(svc.client.clone()),
            _ => None,
        })
        .ok_or_else(|| "Freelo klient není nakonfigurován".to_string())
}

/// Resolve the Jira client a given audit entry was originally posted from,
/// without depending on the legacy `state.jira_client_cloned()` shim.
///
/// Audit rows pre-date the multi-tenant era and don't store `connection_id`
/// directly, so we have to recover it the hard way:
///
///   1. Parse `before_json` / `after_json` (a serialised `WorklogRow`) and
///      pick up `connection_id` from the snapshot. This is the strongest
///      signal — it's exactly the tenant the original mutation hit.
///   2. If snapshots don't carry a usable id (legacy WorklogRow shape, or
///      `connection_id` was `NULL` at the time), fall back to mapping the
///      audit row's `issue_key` through `issues_v2.connection_id`.
///   3. As a last resort, return the first enabled Jira connection so the
///      single-tenant install (and the legacy bisect window) still works.
///
/// The fallback is intentionally lossy: in a multi-tenant install with two
/// Jiras configured, a restore of a worklog whose original tenant has been
/// removed will land on the surviving Jira. The audit row carries
/// `source_audit_id` linkage so the user sees the failure (mismatched issue
/// key) up front rather than a silent cross-tenant write.
fn resolve_jira_client_for_audit(
    state: &tauri::State<'_, AppState>,
    audit_id: i64,
) -> Result<crate::jira::JiraClient, String> {
    let entry = cache::audit::get_by_id(&state.db, audit_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Audit záznam nenalezen".to_string())?;

    let conns = state
        .connections
        .read()
        .expect("AppState.connections RwLock poisoned");
    let find_jira_by_id = |cid: i64| -> Option<crate::jira::JiraClient> {
        conns
            .iter()
            .find(|c| c.id == cid && c.enabled)
            .and_then(|c| match &c.client {
                ProviderClient::Jira(j) => Some(j.clone()),
                _ => None,
            })
    };

    // (1) connection_id from a snapshot.
    for src in [entry.before_json.as_deref(), entry.after_json.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(src) {
            if let Some(cid) = v.get("connection_id").and_then(|x| x.as_i64()) {
                if let Some(j) = find_jira_by_id(cid) {
                    return Ok(j);
                }
            }
        }
    }

    // (2) connection_id derived from the row's issue_key via the cache.
    if let Some(key) = entry.issue_key.as_deref() {
        if let Ok(Some(cid)) = cache::issues::get_connection_id_by_key(&state.db, key) {
            if let Some(j) = find_jira_by_id(cid) {
                return Ok(j);
            }
        }
    }

    // (3) Fallback: first enabled Jira.
    conns
        .iter()
        .filter(|c| c.enabled)
        .find_map(|c| match &c.client {
            ProviderClient::Jira(j) => Some(j.clone()),
            _ => None,
        })
        .ok_or_else(|| "Jira klient není nakonfigurován".to_string())
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
) -> Result<cache::worklogs::WorklogRow, String> {
    let saved = if audit_is_freelo(&state.db, audit_id) {
        let client = first_freelo_client(&state)?;
        freelo::reconstruct::restore_deleted_worklog(&client, &state.db, audit_id)
            .await
            .map_err(freelo_reconstruct_err_to_string)?
    } else {
        let client = resolve_jira_client_for_audit(&state, audit_id)?;
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
) -> Result<cache::worklogs::WorklogRow, String> {
    let after = if audit_is_freelo(&state.db, audit_id) {
        let client = first_freelo_client(&state)?;
        freelo::reconstruct::revert_worklog_update(&client, &state.db, audit_id)
            .await
            .map_err(freelo_reconstruct_err_to_string)?
    } else {
        let client = resolve_jira_client_for_audit(&state, audit_id)?;
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
        let client = resolve_jira_client_for_audit(&state, audit_id)?;
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
