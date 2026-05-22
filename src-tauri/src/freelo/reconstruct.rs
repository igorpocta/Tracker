//! Freelo protějšek `jira/reconstruct.rs` — restore / revert / retry pro
//! Freelo audit záznamy.
//!
//! Vzhled je úmyslně shodný s Jira variantou, aby `commands::worklog`
//! mohl dispatchnout jen podle `issue_key` prefixu (`FREELO-` →
//! Freelo, jinak Jira). Klienti, error type i návratové hodnoty jsou
//! ekvivalentní.

use chrono::{Local, NaiveDate, TimeZone};
use serde_json::Value;
use thiserror::Error;

use super::client::{FreeloClient, FreeloError};
use super::ops::{ms_to_date, seconds_to_minutes};
use crate::audit_helpers;
use crate::cache::{
    self,
    audit::{AuditEntry, AuditOp},
    worklogs::WorklogRow,
    Db, DbError,
};

#[derive(Debug, Error)]
pub enum ReconstructError {
    #[error("audit záznam nenalezen")]
    AuditNotFound,
    #[error("nesprávný typ operace pro tuto akci")]
    WrongOp,
    #[error("audit záznam je {0} — zkuste \"Zkusit znovu\"")]
    AuditUnsuccessful(&'static str),
    #[error("audit záznam nemá kompletní snímek záznamu")]
    SnapshotMissing,
    #[error("worklog už neexistuje ve Freelu")]
    WorklogGone,
    #[error("snímek záznamu obsahuje neplatný čas")]
    BadTimestamp,
    #[error("freelo: {0}")]
    Freelo(#[from] FreeloError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("retry nepodporuje typ {0}")]
    UnsupportedRetry(String),
    #[error("freelo specifická chyba: {0}")]
    Generic(String),
}

fn fetch_audit(db: &Db, audit_id: i64) -> Result<AuditEntry, ReconstructError> {
    cache::audit::get_by_id(db, audit_id)?.ok_or(ReconstructError::AuditNotFound)
}

/// Restore Freelo worklog smazaný přes `delete` / `sync_tombstone`. Vytvoří
/// **nový** work-report ve Freelu z `before_json` snapshotu (Freelo API
/// neumí resurrect by id) a zapíše ho do cache + audit.
pub async fn restore_deleted_worklog(
    client: &FreeloClient,
    db: &Db,
    audit_id: i64,
) -> Result<WorklogRow, ReconstructError> {
    let entry = fetch_audit(db, audit_id)?;
    if entry.op != "delete" && entry.op != "sync_tombstone" {
        return Err(ReconstructError::WrongOp);
    }
    if !entry.success {
        return Err(ReconstructError::AuditUnsuccessful("neúspěšný"));
    }
    let before_json = entry
        .before_json
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let before = audit_helpers::parse_row::<ReconstructError>(before_json)?;

    let issue_key = before.issue_key.clone().unwrap_or_default();
    let task_id = super::parse_task_key(&issue_key)
        .ok_or_else(|| ReconstructError::Generic(format!("issue_key není Freelo: {issue_key}")))?;

    let duration_s = before.duration_s();
    let minutes =
        seconds_to_minutes(duration_s).map_err(|e| ReconstructError::Generic(e.to_string()))?;
    let date = date_from_started_at(before.started_at)?;

    let resp = match client
        .create_work_report(task_id, date, minutes, before.description.as_deref())
        .await
    {
        Ok(r) => r,
        Err(e) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Restore,
                Some(&issue_key),
                entry.worklog_id.as_deref(),
                Some(&before),
                None,
                false,
                Some(&e.to_string()),
                audit_id,
            );
            return Err(ReconstructError::Freelo(e));
        }
    };

    let connection_id = cache::issues::get_connection_id_by_key(db, &issue_key)?;
    let now = audit_helpers::now_unix();
    let row = WorklogRow {
        id: None,
        connection_id,
        issue_key: Some(issue_key.clone()),
        description: before.description.clone(),
        started_at: before.started_at,
        ended_at: before.started_at.saturating_add(duration_s.max(0)),
        logged_at: now,
        updated_at: now,
        is_synced: true,
        synced_at: Some(now),
        remote_id: Some(resp.id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    let local_id = cache::worklogs::upsert_from_remote(db, &row)?;
    let mut saved = row.clone();
    saved.id = Some(local_id);

    audit_helpers::record_linked(
        db,
        AuditOp::Restore,
        Some(&issue_key),
        Some(&resp.id.to_string()),
        Some(&before),
        Some(&saved),
        true,
        None,
        audit_id,
    );
    Ok(saved)
}

/// Vrátit update — pošle `before_json` zpátky na Freelo přes
/// `POST /work-reports/{id}`. Pokud Freelo vrátí 404 (work-report už
/// neexistuje), vrací [`ReconstructError::WorklogGone`].
pub async fn revert_worklog_update(
    client: &FreeloClient,
    db: &Db,
    audit_id: i64,
) -> Result<WorklogRow, ReconstructError> {
    let entry = fetch_audit(db, audit_id)?;
    if entry.op != "update" {
        return Err(ReconstructError::WrongOp);
    }
    if !entry.success {
        return Err(ReconstructError::AuditUnsuccessful("neúspěšný"));
    }
    let before_json = entry
        .before_json
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let before = audit_helpers::parse_row::<ReconstructError>(before_json)?;

    let worklog_id_str = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let work_report_id: i64 = worklog_id_str
        .parse()
        .map_err(|_| ReconstructError::Generic(format!("neplatný worklog_id {worklog_id_str}")))?;
    let issue_key = before.issue_key.clone().unwrap_or_default();

    let current = cache::worklogs::get_by_remote_id_any(db, worklog_id_str)?
        .ok_or(ReconstructError::WorklogGone)?;
    if current.tombstoned_at.is_some() {
        return Err(ReconstructError::WorklogGone);
    }

    let duration_s = before.duration_s();
    let minutes =
        seconds_to_minutes(duration_s).map_err(|e| ReconstructError::Generic(e.to_string()))?;
    let date = date_from_started_at(before.started_at)?;

    match client
        .update_work_report(
            work_report_id,
            Some(minutes),
            Some(date),
            before.description.as_deref(),
        )
        .await
    {
        Ok(_) => {}
        Err(FreeloError::WorkReportNotFound) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Revert,
                Some(&issue_key),
                Some(worklog_id_str),
                Some(&current),
                None,
                false,
                Some("Work-report už neexistuje ve Freelu"),
                audit_id,
            );
            return Err(ReconstructError::WorklogGone);
        }
        Err(e) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Revert,
                Some(&issue_key),
                Some(worklog_id_str),
                Some(&current),
                None,
                false,
                Some(&e.to_string()),
                audit_id,
            );
            return Err(ReconstructError::Freelo(e));
        }
    }

    let local_id = current.id.ok_or(ReconstructError::SnapshotMissing)?;
    let now = audit_helpers::now_unix();
    let ended_at = before.started_at.saturating_add(duration_s.max(0));
    cache::worklogs::update_fields(
        db,
        local_id,
        Some(&issue_key),
        before.description.as_deref(),
        before.started_at,
        ended_at,
        Some(now),
    )?;
    let after =
        cache::worklogs::get_by_id(db, local_id)?.ok_or(ReconstructError::SnapshotMissing)?;

    audit_helpers::record_linked(
        db,
        AuditOp::Revert,
        Some(&issue_key),
        Some(worklog_id_str),
        Some(&current),
        Some(&after),
        true,
        None,
        audit_id,
    );
    Ok(after)
}

/// Replay neúspěšného create/update/delete proti Freelo API.
pub async fn retry_failed_audit_action(
    client: &FreeloClient,
    db: &Db,
    audit_id: i64,
) -> Result<Value, ReconstructError> {
    let entry = fetch_audit(db, audit_id)?;
    if entry.success {
        return Err(ReconstructError::AuditUnsuccessful("úspěšný"));
    }
    match entry.op.as_str() {
        "create" => retry_create(client, db, &entry).await,
        "update" => retry_update(client, db, &entry).await,
        "delete" | "sync_tombstone" => retry_delete(client, db, &entry).await,
        other => Err(ReconstructError::UnsupportedRetry(other.to_string())),
    }
}

async fn retry_create(
    client: &FreeloClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let snapshot_json = entry
        .after_json
        .as_deref()
        .or(entry.before_json.as_deref())
        .ok_or(ReconstructError::SnapshotMissing)?;
    let snap = audit_helpers::parse_row::<ReconstructError>(snapshot_json)?;
    let issue_key = snap.issue_key.clone().unwrap_or_default();
    let task_id = super::parse_task_key(&issue_key)
        .ok_or_else(|| ReconstructError::Generic(format!("issue_key není Freelo: {issue_key}")))?;
    let duration_s = snap.duration_s();
    let minutes =
        seconds_to_minutes(duration_s).map_err(|e| ReconstructError::Generic(e.to_string()))?;
    let date = date_from_started_at(snap.started_at)?;

    match client
        .create_work_report(task_id, date, minutes, snap.description.as_deref())
        .await
    {
        Ok(resp) => {
            let connection_id = cache::issues::get_connection_id_by_key(db, &issue_key)?;
            let now = audit_helpers::now_unix();
            let row = WorklogRow {
                id: None,
                connection_id,
                issue_key: Some(issue_key.clone()),
                description: snap.description.clone(),
                started_at: snap.started_at,
                ended_at: snap.started_at.saturating_add(duration_s.max(0)),
                logged_at: now,
                updated_at: now,
                is_synced: true,
                synced_at: Some(now),
                remote_id: Some(resp.id.to_string()),
                pending_delete_at: None,
                tombstoned_at: None,
                summary: None,
            };
            let local_id = cache::worklogs::upsert_from_remote(db, &row)?;
            let mut saved = row.clone();
            saved.id = Some(local_id);

            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                Some(&resp.id.to_string()),
                None,
                Some(&saved),
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "create",
                "worklog_id": resp.id.to_string(),
            }))
        }
        Err(e) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                None,
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Freelo(e))
        }
    }
}

async fn retry_update(
    client: &FreeloClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let snapshot_json = entry
        .after_json
        .as_deref()
        .or(entry.before_json.as_deref())
        .ok_or(ReconstructError::SnapshotMissing)?;
    let snap = audit_helpers::parse_row::<ReconstructError>(snapshot_json)?;
    let worklog_id_str = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let work_report_id: i64 = worklog_id_str
        .parse()
        .map_err(|_| ReconstructError::Generic(format!("neplatný worklog_id {worklog_id_str}")))?;
    let issue_key = entry
        .issue_key
        .clone()
        .or_else(|| snap.issue_key.clone())
        .unwrap_or_default();
    let duration_s = snap.duration_s();
    let minutes =
        seconds_to_minutes(duration_s).map_err(|e| ReconstructError::Generic(e.to_string()))?;
    let date = date_from_started_at(snap.started_at)?;

    match client
        .update_work_report(
            work_report_id,
            Some(minutes),
            Some(date),
            snap.description.as_deref(),
        )
        .await
    {
        Ok(_) => {
            if let Some(local) = cache::worklogs::get_by_remote_id_any(db, worklog_id_str)? {
                if let Some(lid) = local.id {
                    let now = audit_helpers::now_unix();
                    let ended_at = snap.started_at.saturating_add(duration_s.max(0));
                    cache::worklogs::update_fields(
                        db,
                        lid,
                        Some(&issue_key),
                        snap.description.as_deref(),
                        snap.started_at,
                        ended_at,
                        Some(now),
                    )?;
                }
            }
            let after = cache::worklogs::get_by_remote_id_any(db, worklog_id_str)?;
            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                Some(worklog_id_str),
                None,
                after.as_ref(),
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "update",
                "worklog_id": worklog_id_str,
            }))
        }
        Err(e) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                Some(worklog_id_str),
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Freelo(e))
        }
    }
}

async fn retry_delete(
    client: &FreeloClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let worklog_id_str = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let work_report_id: i64 = worklog_id_str
        .parse()
        .map_err(|_| ReconstructError::Generic(format!("neplatný worklog_id {worklog_id_str}")))?;
    let issue_key = entry.issue_key.clone().unwrap_or_default();
    match client.delete_work_report(work_report_id).await {
        Ok(()) | Err(FreeloError::WorkReportNotFound) => {
            let now = audit_helpers::now_unix();
            if let Some(row) = cache::worklogs::get_by_remote_id_any(db, worklog_id_str)? {
                if let Some(local_id) = row.id {
                    cache::worklogs::mark_tombstoned(db, local_id, now)?;
                }
            }
            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                Some(worklog_id_str),
                None,
                None,
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "delete",
                "worklog_id": worklog_id_str,
            }))
        }
        Err(e) => {
            audit_helpers::record_linked(
                db,
                AuditOp::Retry,
                Some(&issue_key),
                Some(worklog_id_str),
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Freelo(e))
        }
    }
}

fn date_from_started_at(unix_s: i64) -> Result<NaiveDate, ReconstructError> {
    let _ = ms_to_date; // ujištění, že funkce existuje a je v scope
    Local
        .timestamp_opt(unix_s, 0)
        .single()
        .map(|dt| dt.date_naive())
        .ok_or(ReconstructError::BadTimestamp)
}
