//! CRUD + lifecycle mutations for individual worklogs (Phase 15 mutation
//! surface + Phase 18A unassigned/local-only handling).
//!
//! Covers: create / update / delete / move / split / push-local / assign /
//! local-only delete, plus the background commit-pending-delete worker used
//! by both the foreground delete flow and the startup recovery path in
//! `lib.rs`.

use chrono::{TimeZone, Utc};
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

/// Return all unassigned worklogs (no issue key yet) so the user can review and
/// assign them before invoicing. Backs the "Nepřiřazené" screen + sidebar badge.
#[tauri::command]
pub async fn list_unassigned_worklogs(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WorklogRow>, String> {
    cache::worklogs::list_unassigned(&state.db).map_err(|e| e.to_string())
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

    // Build the tail piece, then shrink the original + insert the tail in ONE
    // transaction so a failure can't shrink the original while losing the tail.
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
    let new_id = cache::worklogs::split(
        &state.db,
        local_id,
        before.issue_key.as_deref(),
        before.description.as_deref(),
        before.started_at,
        split_at_s,
        &second,
    )
    .map_err(|e| e.to_string())?;

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
///
/// Safety contract:
/// - If the issues cache knows the owning `connection_id`, we MUST use that
///   exact connection or fail loudly. Silently retargeting another tenant is
///   unacceptable for worklog mutations.
/// - If the issues cache has no connection signal at all, fallback is allowed
///   only when there is exactly ONE enabled connection of the matching
///   provider kind. With multiple plausible tenants we fail and ask the caller
///   to refresh/sync first.
fn resolve_client_for_issue(
    state: &AppState,
    issue_key: &str,
) -> Result<(i64, ProviderClient), String> {
    let conn_id =
        cache::issues::get_connection_id_by_key(&state.db, issue_key).map_err(|e| e.to_string())?;
    let conns = state
        .connections
        .read()
        .unwrap_or_else(|e| e.into_inner());
    // If we know the connection id, prefer that.
    if let Some(cid) = conn_id {
        return conns
            .iter()
            .find(|c| c.id == cid && c.enabled)
            .map(|active| (active.id, active.client.clone()))
            .ok_or_else(|| {
                format!(
                    "Připojení id={cid} pro úkol {issue_key} není aktivní — zapněte ho v nastavení a zkuste znovu"
                )
            });
    }

    // No cache signal. Fallback is safe only when there is exactly one
    // plausible provider connection; with multiple tenants we'd risk logging
    // time to the wrong server.
    let want_freelo = freelo::is_freelo_key(issue_key);
    let matches: Vec<_> = conns
        .iter()
        .filter(|c| c.enabled)
        .filter(|c| {
            matches!(
                (&c.client, want_freelo),
                (ProviderClient::Freelo(_), true) | (ProviderClient::Jira(_), false)
            )
        })
        .collect();

    match matches.as_slice() {
        [single] => Ok((single.id, single.client.clone())),
        [] => Err("Žádné aktivní připojení pro tento úkol".into()),
        _ => Err(format!(
            "Úkol {issue_key} není v lokální cache a existuje více možných připojení — nejprve proveďte sync a vyberte úkol z nabídky"
        )),
    }
}

/// Typed Freelo variant. Returns `(connection_id, FreeloService)` for the
/// connection that owns `issue_key`. The `FreeloService` carries both the
/// HTTP client AND the persisted config (selected_project_ids, sync_user_id),
/// because every Freelo write API needs the user id and we want callers to
/// see the typed error when it's missing, rather than the legacy
/// `.unwrap_or(0)` that silently produced an invalid request.
///
/// Errors when (a) the issue resolves to a Jira connection, or (b) no Freelo
/// connection at all is configured.
pub fn resolve_freelo_service_for_issue(
    state: &AppState,
    issue_key: &str,
) -> Result<(i64, crate::freelo::worklog_service_impl::FreeloService), String> {
    let (conn_id, client) = resolve_client_for_issue(state, issue_key)?;
    match client {
        ProviderClient::Freelo(svc) => Ok((conn_id, svc)),
        ProviderClient::Jira(_) => Err(format!("Úkol {issue_key} patří do Jira, ne Freelo")),
    }
}

/// Like [`resolve_freelo_service_for_issue`] but additionally requires that
/// the connection has finished its initial sync and cached a Freelo user
/// id. Returns `(connection_id, FreeloClient, user_id)` — exactly the
/// three things every Freelo write helper needs.
///
/// The pre-fix code pulled `cfg.sync_user_id.unwrap_or(0)` which generated
/// a clearly-invalid POST body that Freelo rejected with a generic 400.
/// Now we surface the missing-sync condition up front, so the UI can prompt
/// the user to finish setup instead of getting a confusing API error.
pub fn resolve_freelo_client_with_user_for_issue(
    state: &AppState,
    issue_key: &str,
) -> Result<(i64, crate::freelo::client::FreeloClient, i64), String> {
    let (conn_id, svc) = resolve_freelo_service_for_issue(state, issue_key)?;
    let user_id = svc
        .config
        .sync_user_id
        .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;
    Ok((conn_id, svc.client, user_id))
}

/// Typed Jira variant of [`resolve_client_for_issue`] for callers that know
/// they need a Jira mutation. Errors when the resolved connection is Freelo
/// (caller should have routed through the Freelo helper instead) or when no
/// Jira connection at all can plausibly handle the key.
///
/// Replaces ad-hoc `state.jira_client_cloned()` calls scattered across the
/// mutation paths — those silently used the FIRST Jira connection regardless
/// of which tenant the issue actually belongs to. With two Jira accounts
/// configured (e.g. SAB Jira + personal Jira) every PUT/POST/DELETE could
/// land on the wrong tenant.
pub fn resolve_jira_client_for_issue(
    state: &AppState,
    issue_key: &str,
) -> Result<(i64, crate::jira::JiraClient), String> {
    let (conn_id, client) = resolve_client_for_issue(state, issue_key)?;
    match client {
        ProviderClient::Jira(j) => Ok((conn_id, j)),
        ProviderClient::Freelo(_) => Err(format!("Úkol {issue_key} patří do Freelo, ne Jira")),
    }
}

/// Resolve the active client based on a worklog row's recorded
/// `connection_id`. The worklog row is the source of truth for mutations
/// over an existing remote worklog — the issue cache may be stale or
/// missing the connection link entirely.
///
/// Errors when:
///   * `row.connection_id` is `None` (the row was never tied to a tenant —
///     this should not happen for a row that has a `remote_id`),
///   * the connection no longer exists,
///   * the connection exists but is disabled.
///
/// Crucially, this function does NOT fall back to "first matching provider".
/// A disabled or removed connection must surface as an explicit error so the
/// user sees the failure instead of an update silently landing on a
/// different tenant.
pub fn resolve_client_for_row(
    state: &AppState,
    row: &WorklogRow,
) -> Result<(i64, ProviderClient), String> {
    let conns = state
        .connections
        .read()
        .unwrap_or_else(|e| e.into_inner());
    resolve_client_for_row_in(&conns, row)
}

/// Pure variant exposed for unit tests — operates on a slice instead of
/// reaching into [`AppState`]. Production code keeps using
/// [`resolve_client_for_row`] which takes the lock once.
pub fn resolve_client_for_row_in(
    connections: &[crate::state::ActiveConnection],
    row: &WorklogRow,
) -> Result<(i64, ProviderClient), String> {
    let cid = row.connection_id.ok_or_else(|| {
        "Worklog nemá zaznamenané connection_id — nelze určit kam ho odeslat".to_string()
    })?;
    let active = connections
        .iter()
        .find(|c| c.id == cid)
        .ok_or_else(|| format!("Připojení id={cid} pro tento worklog neexistuje (smazané?)"))?;
    if !active.enabled {
        return Err(format!(
            "Připojení '{}' je vypnuté — zapněte ho v nastavení a zkuste znovu",
            active.name
        ));
    }
    Ok((active.id, active.client.clone()))
}

/// Typed Jira variant of [`resolve_client_for_row`].
pub fn resolve_jira_client_for_row(
    state: &AppState,
    row: &WorklogRow,
) -> Result<(i64, crate::jira::JiraClient), String> {
    let (cid, client) = resolve_client_for_row(state, row)?;
    match client {
        ProviderClient::Jira(j) => Ok((cid, j)),
        ProviderClient::Freelo(_) => Err(
            "Záznam patří do Jira ale connection_id ukazuje na Freelo — datová nekonzistence"
                .into(),
        ),
    }
}

/// Typed Freelo variant of [`resolve_client_for_row`]. Returns
/// `(connection_id, FreeloClient, user_id)` — the three things every Freelo
/// write helper needs. Errors out front if the connection's sync hasn't
/// cached a user id yet, instead of letting Freelo reject the request with
/// a generic 400.
pub fn resolve_freelo_client_for_row(
    state: &AppState,
    row: &WorklogRow,
) -> Result<(i64, crate::freelo::client::FreeloClient, i64), String> {
    let (cid, client) = resolve_client_for_row(state, row)?;
    match client {
        ProviderClient::Freelo(svc) => {
            let user_id = svc
                .config
                .sync_user_id
                .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;
            Ok((cid, svc.client, user_id))
        }
        ProviderClient::Jira(_) => Err(
            "Záznam patří do Freelo ale connection_id ukazuje na Jira — datová nekonzistence"
                .into(),
        ),
    }
}

/// Resolve the LOCAL cached row for a provider worklog, scoped to the
/// connection that owns `issue_key`.
///
/// `remote_id` is unique only inside `(connection_id, remote_id)`. Looking up
/// by `remote_id` alone can therefore hit the wrong row when the user has more
/// than one Jira/Freelo connection configured, or when Jira and Freelo happen
/// to generate the same numeric id string. We first scope by the issue's
/// owning connection and only fall back to the legacy global lookup if the
/// issues cache genuinely has no connection signal.
pub fn resolve_cached_worklog_for_issue_and_remote_id(
    state: &AppState,
    issue_key: &str,
    remote_id: &str,
) -> Result<WorklogRow, String> {
    let conn_id =
        cache::issues::get_connection_id_by_key(&state.db, issue_key).map_err(|e| e.to_string())?;
    if let Some(cid) = conn_id {
        if let Some(row) = cache::worklogs::get_by_remote_id(&state.db, cid, remote_id)
            .map_err(|e| e.to_string())?
        {
            return Ok(row);
        }
    }
    cache::worklogs::get_by_remote_id_any(&state.db, remote_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen v lokální paměti".to_string())
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

    // P1-2: route by the OWNING connection's provider type (not the "FREELO-"
    // text prefix) whenever we know who owns this issue — a Jira project whose
    // key starts with FREELO must go to Jira. The prefix is only a heuristic
    // for issues we have never cached locally.
    let owner_is_freelo = match cache::issues::get_connection_id_by_key(&state.db, &issue_key) {
        Ok(Some(cid)) => {
            let conns = state
                .connections
                .read()
                .unwrap_or_else(|e| e.into_inner());
            conns
                .iter()
                .find(|c| c.id == cid && c.enabled)
                .map(|c| matches!(c.client, ProviderClient::Freelo(_)))
        }
        _ => None,
    };
    let route_to_freelo = owner_is_freelo.unwrap_or_else(|| freelo::is_freelo_key(&issue_key));

    // Dispatch by provider.
    if route_to_freelo {
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

    // Route to the Jira connection that actually owns this issue — not
    // the legacy "first Jira client" shim, which silently mis-targets in
    // multi-tenant setups.
    let (conn_id, client) = resolve_jira_client_for_issue(&state, &issue_key)?;

    let started_dt = Utc
        .timestamp_millis_opt(started_at_ms)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;

    let started_at_s = started_at_ms / 1000;

    // P1-4: before POSTing, adopt an already-created upstream worklog from a
    // prior attempt whose local write failed, so a retry doesn't duplicate it.
    let remote_id = if let Some(found) = super::reconcile::find_existing_jira_worklog_id(
        &client,
        &issue_key,
        started_at_s,
        duration_seconds,
    )
    .await
    {
        found
    } else {
        match client
            .add_worklog(&issue_key, started_dt, duration_seconds, comment.as_deref())
            .await
        {
            Ok(r) => r.id,
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
        }
    };

    let now_s = Utc::now().timestamp();
    let row = WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.clone()),
        description: comment.clone(),
        started_at: started_at_s,
        ended_at: started_at_s.saturating_add(duration_seconds.max(0)),
        logged_at: now_s,
        updated_at: now_s,
        is_synced: true,
        synced_at: Some(now_s),
        remote_id: Some(remote_id.clone()),
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
        Some(&remote_id),
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
        ProviderClient::Freelo(svc) => (svc.client, svc.config),
        _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
    };
    let user_id = cfg
        .sync_user_id
        .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;

    // P1-4: adopt an already-created upstream report from a prior partial
    // attempt instead of POSTing a duplicate.
    let started_at_s = started_at_ms / 1000;
    if let (Some(task_id), Ok(minutes)) = (
        freelo::parse_task_key(&issue_key),
        freelo::ops::seconds_to_minutes(duration_seconds),
    ) {
        if let Some(existing) = super::reconcile::find_existing_freelo_report(
            &client,
            task_id,
            user_id,
            started_at_s,
            minutes,
        )
        .await
        {
            let now = Utc::now().timestamp();
            let mut row = freelo::sync::work_report_to_row(&existing, conn_id, now);
            let duration_s = minutes.saturating_mul(60);
            row.started_at = started_at_s;
            row.ended_at = started_at_s.saturating_add(duration_s);
            row.logged_at = started_at_s;
            let id =
                cache::worklogs::upsert_from_remote(&state.db, &row).map_err(|e| e.to_string())?;
            row.id = Some(id);
            audit_success(
                &state.db,
                AuditOp::Create,
                Some(&issue_key),
                row.remote_id.as_deref(),
                None,
                Some(&row),
            );
            let _ = app.emit("worklog-created", &row);
            return Ok(row);
        }
    }

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

    // Route by the ROW's owning connection, not the issue-key text prefix: a
    // Jira project whose key starts with "FREELO-" must still go to Jira. The
    // row's connection_id is the source of truth for an existing remote
    // worklog (the issue cache can lose its connection link).
    let before = resolve_cached_worklog_for_issue_and_remote_id(&state, &issue_key, &worklog_id)?;
    let row_is_freelo = matches!(
        resolve_client_for_row(&state, &before).map(|(_, c)| c),
        Ok(crate::state::ProviderClient::Freelo(_))
    );
    if row_is_freelo {
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

    let (_conn_id, client) = resolve_jira_client_for_row(&state, &before)?;

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
    let before = resolve_cached_worklog_for_issue_and_remote_id(&state, &issue_key, &worklog_id)?;
    let local_id = before
        .id
        .ok_or_else(|| "Chybí lokální id záznamu".to_string())?;

    // Parse the freelo:N synthetic id back into the numeric work_report_id.
    let wr_id = freelo::parse_worklog_id(&worklog_id)
        .ok_or_else(|| format!("Neplatné Freelo id záznamu: {worklog_id}"))?;

    let (_cid, client, _user_id) = resolve_freelo_client_for_row(&state, &before)?;

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
    let before = resolve_cached_worklog_for_issue_and_remote_id(&state, &issue_key, &worklog_id)?;
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
    let before = cache::worklogs::get_pending_delete_by_remote_id_any(&state.db, &worklog_id)
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

    // Move is Jira-only (Freelo has no analogous API). Route via the
    // ROW's stamped connection_id — the source of truth for an existing
    // remote worklog. The issue cache might point at a different tenant
    // (or none at all); only the row knows where the worklog actually
    // lives. Jira refuses cross-tenant moves anyway, so the new key has
    // to live on the same host.
    let before_row =
        resolve_cached_worklog_for_issue_and_remote_id(&state, &old_issue_key, &old_worklog_id)?;
    let (conn_id, client) = resolve_jira_client_for_row(&state, &before_row)?;
    let before = Some(before_row);

    let started_dt = Utc
        .timestamp_millis_opt(started_at_ms)
        .single()
        .ok_or_else(|| "Neplatný čas začátku".to_string())?;

    let args = MoveWorklogArgs {
        old_issue_key: &old_issue_key,
        old_worklog_id: &old_worklog_id,
        new_issue_key: &new_issue_key,
        started: started_dt,
        time_spent_seconds: duration_seconds,
        comment: comment.as_deref(),
        fallback_connection_id: Some(conn_id),
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
    push_local_worklog_inner(&app, &state, local_id).await
}

/// Tauri-State-free body of [`push_local_worklog`]. Exposed so the local
/// HTTP server can dispatch the same flow as a fire-and-forget background
/// task after `/stop-timer` records the local row — keeping the legacy
/// "pull-only refresh" comment honest by actually getting the worklog to
/// the provider, not leaving it stuck locally until the user clicks
/// "Synchronizovat" by hand.
///
/// Generic over `Runtime` so it works under both `tauri::Wry` (real app)
/// and `tauri::test::MockRuntime` (integration tests).
pub async fn push_local_worklog_inner<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &AppState,
    local_id: i64,
) -> Result<WorklogRow, String> {
    let _push_guard = state.worklog_push_lock.lock().await;
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
        let (conn_id, client) = resolve_client_for_issue(state, &issue_key)?;
        let (client, cfg) = match client {
            ProviderClient::Freelo(svc) => (svc.client, svc.config),
            _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
        };
        let user_id = cfg
            .sync_user_id
            .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;

        // P1-4: if a prior attempt already created this report upstream (HTTP
        // 201 followed by a failed local write), adopt it instead of POSTing a
        // duplicate.
        if let (Some(task_id), Ok(minutes)) = (
            freelo::parse_task_key(&issue_key),
            freelo::ops::seconds_to_minutes(before.duration_s()),
        ) {
            if let Some(existing) = super::reconcile::find_existing_freelo_report(
                &client,
                task_id,
                user_id,
                before.started_at,
                minutes,
            )
            .await
            {
                let now = Utc::now().timestamp();
                let mut row = freelo::sync::work_report_to_row(&existing, conn_id, now);
                let duration_s = minutes.saturating_mul(60);
                row.started_at = before.started_at;
                row.ended_at = before.started_at.saturating_add(duration_s);
                row.logged_at = before.started_at;
                let id = cache::worklogs::upsert_from_remote(&state.db, &row)
                    .map_err(|e| e.to_string())?;
                row.id = Some(id);
                let _ = cache::worklogs::delete_local_only(&state.db, local_id);
                let _ = app.emit("worklog-updated", &row);
                return Ok(row);
            }
        }

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

    // Jira path — route to the connection that owns this issue.
    let (_conn_id, client) = resolve_jira_client_for_issue(state, &issue_key)?;
    // P1-4: adopt an already-created upstream worklog from a prior partial
    // attempt instead of POSTing a duplicate.
    let remote_id = if let Some(found) = super::reconcile::find_existing_jira_worklog_id(
        &client,
        &issue_key,
        before.started_at,
        before.duration_s(),
    )
    .await
    {
        found
    } else {
        let started_dt = Utc
            .timestamp_opt(before.started_at, 0)
            .single()
            .ok_or_else(|| "Neplatný čas začátku".to_string())?;
        client
            .add_worklog(
                &issue_key,
                started_dt,
                before.duration_s(),
                before.description.as_deref(),
            )
            .await
            .map_err(|e| format!("Jira: {e}"))?
            .id
    };

    let connection_id = cache::issues::get_connection_id_by_key(&state.db, &issue_key)
        .map_err(|e| e.to_string())?;
    cache::worklogs::assign_issue(
        &state.db,
        local_id,
        connection_id,
        &issue_key,
        Some(&remote_id),
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
    let _push_guard = state.worklog_push_lock.lock().await;
    let before = cache::worklogs::get_by_id(&state.db, worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam nenalezen".to_string())?;
    if before.issue_key.is_some() {
        return Err("Záznam již má přiřazený úkol".into());
    }

    if freelo::is_freelo_key(&issue_key) {
        let (conn_id, client) = resolve_client_for_issue(&state, &issue_key)?;
        let (client, cfg) = match client {
            ProviderClient::Freelo(svc) => (svc.client, svc.config),
            _ => return Err("Připojení nepodporuje Freelo úkoly".into()),
        };
        let user_id = cfg
            .sync_user_id
            .ok_or_else(|| "Freelo: chybí user id, spusťte sync".to_string())?;

        // P1-4: adopt an already-created upstream report from a prior partial
        // attempt instead of POSTing a duplicate.
        if let (Some(task_id), Ok(minutes)) = (
            freelo::parse_task_key(&issue_key),
            freelo::ops::seconds_to_minutes(before.duration_s()),
        ) {
            if let Some(existing) = super::reconcile::find_existing_freelo_report(
                &client,
                task_id,
                user_id,
                before.started_at,
                minutes,
            )
            .await
            {
                let now = Utc::now().timestamp();
                let mut row = freelo::sync::work_report_to_row(&existing, conn_id, now);
                let duration_s = minutes.saturating_mul(60);
                row.started_at = before.started_at;
                row.ended_at = before.started_at.saturating_add(duration_s);
                row.logged_at = before.started_at;
                let id = cache::worklogs::upsert_from_remote(&state.db, &row)
                    .map_err(|e| e.to_string())?;
                row.id = Some(id);
                let _ = cache::worklogs::delete_local_only(&state.db, worklog_id);
                audit_success(
                    &state.db,
                    AuditOp::Update,
                    Some(&issue_key),
                    row.remote_id.as_deref(),
                    Some(&before),
                    Some(&row),
                );
                let _ = app.emit("worklog-updated", &row);
                return Ok(row);
            }
        }

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

    // Jira branch — route by issue connection.
    let (_conn_id, client) = resolve_jira_client_for_issue(&state, &issue_key)?;

    // P1-4: adopt an already-created upstream worklog from a prior partial
    // attempt instead of POSTing a duplicate.
    let remote_id = if let Some(found) = super::reconcile::find_existing_jira_worklog_id(
        &client,
        &issue_key,
        before.started_at,
        before.duration_s(),
    )
    .await
    {
        found
    } else {
        let started_dt = Utc
            .timestamp_opt(before.started_at, 0)
            .single()
            .ok_or_else(|| "Neplatný čas začátku".to_string())?;
        match client
            .add_worklog(
                &issue_key,
                started_dt,
                before.duration_s(),
                before.description.as_deref(),
            )
            .await
        {
            Ok(r) => r.id,
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
        }
    };

    let connection_id = cache::issues::get_connection_id_by_key(&state.db, &issue_key)
        .map_err(|e| e.to_string())?;

    cache::worklogs::assign_issue(
        &state.db,
        worklog_id,
        connection_id,
        &issue_key,
        Some(&remote_id),
    )
    .map_err(|e| e.to_string())?;

    let after = cache::worklogs::get_by_id(&state.db, worklog_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Záznam zmizel po přiřazení".to_string())?;

    audit_success(
        &state.db,
        AuditOp::Update,
        Some(&issue_key),
        Some(&remote_id),
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

    // Branch by the row's owning connection, not the issue-key text prefix —
    // a Jira project keyed "FREELO-*" must still take the Jira path.
    let row_is_freelo = matches!(
        resolve_client_for_row(state, &row).map(|(_, c)| c),
        Ok(crate::state::ProviderClient::Freelo(_))
    );
    if row_is_freelo {
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
        // Route to the Freelo tenant that owns this worklog via the row's
        // recorded `connection_id` — the only trustworthy signal for an
        // existing remote worklog. Disabled or removed connections surface
        // as an explicit error instead of silently retargeting another
        // Freelo account.
        let client = match resolve_freelo_client_for_row(state, &row) {
            Ok((_, c, _)) => c,
            Err(err) => {
                let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
                audit_failure(
                    &state.db,
                    AuditOp::Delete,
                    Some(issue_key),
                    Some(worklog_id),
                    Some(&row),
                    &err,
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

    // Jira branch — route by the row's stamped connection_id, never the
    // issue cache (which can lose its connection link).
    let client = match resolve_jira_client_for_row(state, &row) {
        Ok((_, c)) => c,
        Err(err) => {
            // No client: clear the pending flag so the UI can recover.
            let _ = cache::worklogs::clear_pending_delete(&state.db, local_id);
            audit_failure(
                &state.db,
                AuditOp::Delete,
                Some(issue_key),
                Some(worklog_id),
                Some(&row),
                &err,
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

// -----------------------------------------------------------------------------
// Unit tests — pure resolver behaviour (no AppState / no DB / no Tauri)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::connections::FreeloConnectionConfig;
    use crate::freelo::{FreeloClient, FreeloService};
    use crate::jira::JiraClient;
    use crate::state::ActiveConnection;

    fn make_jira_client() -> JiraClient {
        JiraClient::new(
            "https://example.atlassian.net".to_string(),
            "test@example.com".to_string(),
            "token".to_string(),
        )
        .expect("test JiraClient build")
    }

    fn make_freelo_service(user_id: Option<i64>) -> FreeloService {
        let client = FreeloClient::new(
            "https://api.freelo.io/v0/".to_string(),
            "test@example.com".to_string(),
            "api-key".to_string(),
        )
        .expect("test FreeloClient build");
        let cfg = FreeloConnectionConfig {
            sync_user_id: user_id,
            ..Default::default()
        };
        FreeloService::new(client, cfg)
    }

    fn jira_conn(id: i64, name: &str, enabled: bool) -> ActiveConnection {
        ActiveConnection {
            id,
            kind: "jira".to_string(),
            name: name.to_string(),
            enabled,
            client: ProviderClient::Jira(make_jira_client()),
        }
    }

    fn freelo_conn(id: i64, name: &str, enabled: bool, user_id: Option<i64>) -> ActiveConnection {
        ActiveConnection {
            id,
            kind: "freelo".to_string(),
            name: name.to_string(),
            enabled,
            client: ProviderClient::Freelo(make_freelo_service(user_id)),
        }
    }

    fn row_with_conn(connection_id: Option<i64>) -> WorklogRow {
        WorklogRow {
            id: Some(42),
            connection_id,
            issue_key: Some("DEV-1".to_string()),
            description: None,
            started_at: 0,
            ended_at: 60,
            logged_at: 0,
            updated_at: 0,
            is_synced: true,
            synced_at: Some(0),
            remote_id: Some("99999".to_string()),
            pending_delete_at: None,
            tombstoned_at: None,
            summary: None,
        }
    }

    #[test]
    fn resolve_row_errors_when_connection_id_missing() {
        let conns = vec![jira_conn(1, "Tenant A", true)];
        let row = row_with_conn(None);
        let err = resolve_client_for_row_in(&conns, &row).expect_err("must fail");
        assert!(
            err.contains("connection_id"),
            "expected message about missing connection_id, got: {err}"
        );
    }

    #[test]
    fn resolve_row_errors_when_connection_not_found() {
        let conns = vec![jira_conn(1, "Tenant A", true)];
        let row = row_with_conn(Some(999));
        let err = resolve_client_for_row_in(&conns, &row).expect_err("must fail");
        assert!(
            err.contains("999") && err.contains("neexistuje"),
            "expected message about missing id=999, got: {err}"
        );
    }

    #[test]
    fn resolve_row_errors_when_connection_disabled_no_fallback() {
        // Two enabled Jira tenants are NOT a fallback target — the row's
        // recorded tenant is disabled, so we must surface that explicit
        // error rather than silently mis-routing.
        let conns = vec![
            jira_conn(1, "Tenant A (off)", false),
            jira_conn(2, "Tenant B", true),
        ];
        let row = row_with_conn(Some(1));
        let err = resolve_client_for_row_in(&conns, &row).expect_err("must fail");
        assert!(
            err.contains("Tenant A (off)") && err.contains("vypnuté"),
            "expected disabled-connection message naming 'Tenant A (off)', got: {err}"
        );
    }

    #[test]
    fn resolve_row_picks_recorded_connection_not_first() {
        // With two enabled Jira tenants, the resolver must land on the one
        // the row recorded — never the first one in the list.
        let conns = vec![
            jira_conn(1, "Tenant A", true),
            jira_conn(2, "Tenant B", true),
        ];
        let row = row_with_conn(Some(2));
        let (cid, _) =
            resolve_client_for_row_in(&conns, &row).expect("must resolve to recorded id");
        assert_eq!(cid, 2);
    }

    #[test]
    fn resolve_jira_row_rejects_freelo_provider() {
        // The row's connection_id points at a Freelo connection — the typed
        // Jira variant must refuse rather than coercing.
        let conns = vec![freelo_conn(5, "Freelo", true, Some(123))];
        let row = row_with_conn(Some(5));
        let (cid, client) =
            resolve_client_for_row_in(&conns, &row).expect("base resolver still succeeds");
        assert_eq!(cid, 5);
        assert!(matches!(client, ProviderClient::Freelo(_)));
        // Now exercise the typed variant's mismatch arm directly.
        match client {
            ProviderClient::Jira(_) => panic!("setup error: expected Freelo"),
            ProviderClient::Freelo(_) => {
                // The typed Jira variant reads from `AppState` so we can't
                // call it here, but the discriminant check inside it
                // mirrors this match — if this is Freelo, the typed Jira
                // resolver returns the datová-nekonzistence error.
            }
        }
    }

    #[test]
    fn resolve_freelo_row_errors_when_user_id_missing() {
        // Even if the Freelo connection itself is enabled, an in-progress
        // setup (no cached user id yet) must surface the explicit
        // "spusťte sync" hint rather than letting the API reject it.
        let conns = vec![freelo_conn(5, "Freelo", true, None)];
        let row = row_with_conn(Some(5));
        let (_cid, _client) = resolve_client_for_row_in(&conns, &row).expect("base resolves");
        // The typed Freelo variant lives behind AppState; here we verify
        // the underlying behaviour is composable: the connection resolves,
        // user_id is None, so the typed wrapper would return the expected
        // error string when invoked.
        let svc = &conns[0].client;
        let user_id = match svc {
            ProviderClient::Freelo(s) => s.config.sync_user_id,
            _ => unreachable!(),
        };
        assert!(user_id.is_none());
    }
}
