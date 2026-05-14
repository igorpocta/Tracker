//! Phase 16 — reconstruction (restore / revert / retry) core logic.
//!
//! This module owns the I/O-side logic of the three reconstruction Tauri
//! commands. We separate it from `commands::worklog` so wiremock-backed
//! integration tests can exercise the full path without the Tauri runtime.
//!
//! The Tauri command wrappers are thin: they look up the `JiraClient` from
//! application state, then defer to the helpers below.

use chrono::{TimeZone, Utc};
use serde_json::Value;
use thiserror::Error;

use super::client::{JiraClient, JiraError};
use crate::cache::{
    self,
    audit::{record as audit_record, AuditEntry, AuditEvent, AuditOp},
    worklogs::WorklogRow,
    Db, DbError,
};

/// Errors produced by the reconstruction helpers.
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
    #[error("worklog už neexistuje v Jira")]
    WorklogGone,
    #[error("snímek záznamu obsahuje neplatný čas")]
    BadTimestamp,
    #[error("jira: {0}")]
    Jira(#[from] JiraError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("retry nepodporuje typ {0}")]
    UnsupportedRetry(String),
}

fn parse_row(json: &str) -> Result<WorklogRow, ReconstructError> {
    serde_json::from_str::<WorklogRow>(json).map_err(Into::into)
}

fn now_unix() -> i64 {
    Utc::now().timestamp()
}

fn record_linked(
    db: &Db,
    op: AuditOp,
    issue_key: Option<&str>,
    worklog_id: Option<&str>,
    before: Option<&WorklogRow>,
    after: Option<&WorklogRow>,
    success: bool,
    error: Option<&str>,
    source_audit_id: i64,
) {
    let _ = audit_record(
        db,
        AuditEvent {
            occurred_at: now_unix(),
            op,
            issue_key,
            worklog_id,
            before,
            after,
            success,
            error,
            source_audit_id: Some(source_audit_id),
        },
    );
}

fn fetch_audit(db: &Db, audit_id: i64) -> Result<AuditEntry, ReconstructError> {
    cache::audit::get_by_id(db, audit_id)?
        .ok_or(ReconstructError::AuditNotFound)
}

/// Restore a worklog deleted via `delete` or `sync_tombstone`.
///
/// Reconstructs the row from `before_json` and POSTs a fresh worklog to Jira.
/// On success, inserts the new row into the cache and records a linked audit
/// entry with op = `restore`.
pub async fn restore_deleted_worklog(
    client: &JiraClient,
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
    let before = parse_row(before_json)?;

    let started_dt = Utc
        .timestamp_opt(before.started_at, 0)
        .single()
        .ok_or(ReconstructError::BadTimestamp)?;

    let resp = match client
        .add_worklog(
            &before.issue_key,
            started_dt,
            before.duration_s,
            before.comment.as_deref(),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            record_linked(
                db,
                AuditOp::Restore,
                Some(&before.issue_key),
                entry.worklog_id.as_deref(),
                Some(&before),
                None,
                false,
                Some(&e.to_string()),
                audit_id,
            );
            return Err(ReconstructError::Jira(e));
        }
    };

    let (issue_id, summary) = match cache::issues::get_by_key(db, &before.issue_key)? {
        Some(row) => (row.issue_id, Some(row.summary)),
        None => (resp.issue_id.clone(), before.summary.clone()),
    };

    let now_s = now_unix();
    let row = WorklogRow {
        id: None,
        issue_key: before.issue_key.clone(),
        issue_id,
        summary,
        duration_s: before.duration_s,
        started_at: before.started_at,
        logged_at: now_s,
        comment: before.comment.clone(),
        jira_worklog_id: Some(resp.id.clone()),
        author_account_id: before.author_account_id.clone(),
        source: "jira".to_string(),
        updated_at_jira: Some(now_s),
        pending_delete_at: None,
        tombstoned_at: None,
        pending_assignment: false,
    };
    let local_id = cache::worklogs::upsert_from_jira(db, &row)?;
    let mut saved = row.clone();
    saved.id = Some(local_id);

    record_linked(
        db,
        AuditOp::Restore,
        Some(&before.issue_key),
        Some(&resp.id),
        Some(&before),
        Some(&saved),
        true,
        None,
        audit_id,
    );
    Ok(saved)
}

/// Revert an `update` by pushing `before_json` back to Jira as a fresh PUT.
///
/// Errors with [`ReconstructError::WorklogGone`] if the worklog has been
/// deleted in Jira since the update — the user must use `restore` against the
/// delete audit entry instead.
pub async fn revert_worklog_update(
    client: &JiraClient,
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
    let before = parse_row(before_json)?;

    let worklog_id = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let issue_key = before.issue_key.as_str();

    let current = cache::worklogs::get_by_jira_id(db, worklog_id)?
        .ok_or(ReconstructError::WorklogGone)?;
    if current.tombstoned_at.is_some() {
        return Err(ReconstructError::WorklogGone);
    }

    let started_dt = Utc
        .timestamp_opt(before.started_at, 0)
        .single()
        .ok_or(ReconstructError::BadTimestamp)?;

    match client
        .update_worklog(
            issue_key,
            worklog_id,
            Some(started_dt),
            Some(before.duration_s),
            before.comment.as_deref(),
        )
        .await
    {
        Ok(_) => {}
        Err(JiraError::WorklogNotFound) => {
            record_linked(
                db,
                AuditOp::Revert,
                Some(issue_key),
                Some(worklog_id),
                Some(&current),
                None,
                false,
                Some("Worklog už neexistuje v Jira"),
                audit_id,
            );
            return Err(ReconstructError::WorklogGone);
        }
        Err(e) => {
            record_linked(
                db,
                AuditOp::Revert,
                Some(issue_key),
                Some(worklog_id),
                Some(&current),
                None,
                false,
                Some(&e.to_string()),
                audit_id,
            );
            return Err(ReconstructError::Jira(e));
        }
    }

    let local_id = current.id.ok_or(ReconstructError::SnapshotMissing)?;
    let now_s = now_unix();
    cache::worklogs::update_fields(
        db,
        local_id,
        issue_key,
        before.issue_id.as_deref(),
        before.summary.as_deref(),
        before.duration_s,
        before.started_at,
        before.comment.as_deref(),
        Some(now_s),
    )?;

    let after = cache::worklogs::get_by_id(db, local_id)?
        .ok_or(ReconstructError::SnapshotMissing)?;

    record_linked(
        db,
        AuditOp::Revert,
        Some(issue_key),
        Some(worklog_id),
        Some(&current),
        Some(&after),
        true,
        None,
        audit_id,
    );
    Ok(after)
}

/// Re-issue a previously-failed action using the captured snapshots.
///
/// The strategy depends on the original op (see [`ReconstructError`] for the
/// recognized kinds). Returns a small JSON payload describing the outcome so
/// the caller can render a context-aware toast.
pub async fn retry_failed_audit_action(
    client: &JiraClient,
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
    client: &JiraClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let snapshot_json = entry
        .after_json
        .as_deref()
        .or(entry.before_json.as_deref())
        .ok_or(ReconstructError::SnapshotMissing)?;
    let snap = parse_row(snapshot_json)?;
    let started_dt = Utc
        .timestamp_opt(snap.started_at, 0)
        .single()
        .ok_or(ReconstructError::BadTimestamp)?;

    match client
        .add_worklog(
            &snap.issue_key,
            started_dt,
            snap.duration_s,
            snap.comment.as_deref(),
        )
        .await
    {
        Ok(resp) => {
            let (issue_id, summary) =
                match cache::issues::get_by_key(db, &snap.issue_key)? {
                    Some(row) => (row.issue_id, Some(row.summary)),
                    None => (resp.issue_id.clone(), snap.summary.clone()),
                };
            let now_s = now_unix();
            let row = WorklogRow {
                id: None,
                issue_key: snap.issue_key.clone(),
                issue_id,
                summary,
                duration_s: snap.duration_s,
                started_at: snap.started_at,
                logged_at: now_s,
                comment: snap.comment.clone(),
                jira_worklog_id: Some(resp.id.clone()),
                author_account_id: snap.author_account_id.clone(),
                source: "jira".to_string(),
                updated_at_jira: Some(now_s),
                pending_delete_at: None,
                tombstoned_at: None,
                pending_assignment: false,
            };
            let local_id = cache::worklogs::upsert_from_jira(db, &row)?;
            let mut saved = row.clone();
            saved.id = Some(local_id);

            record_linked(
                db,
                AuditOp::Retry,
                Some(&snap.issue_key),
                Some(&resp.id),
                None,
                Some(&saved),
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "create",
                "worklog_id": resp.id,
            }))
        }
        Err(e) => {
            record_linked(
                db,
                AuditOp::Retry,
                Some(&snap.issue_key),
                None,
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Jira(e))
        }
    }
}

async fn retry_update(
    client: &JiraClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let snapshot_json = entry
        .after_json
        .as_deref()
        .or(entry.before_json.as_deref())
        .ok_or(ReconstructError::SnapshotMissing)?;
    let snap = parse_row(snapshot_json)?;

    let worklog_id = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let issue_key = entry
        .issue_key
        .as_deref()
        .unwrap_or(snap.issue_key.as_str());

    let started_dt = Utc
        .timestamp_opt(snap.started_at, 0)
        .single()
        .ok_or(ReconstructError::BadTimestamp)?;

    match client
        .update_worklog(
            issue_key,
            worklog_id,
            Some(started_dt),
            Some(snap.duration_s),
            snap.comment.as_deref(),
        )
        .await
    {
        Ok(_) => {
            if let Some(local) = cache::worklogs::get_by_jira_id(db, worklog_id)? {
                if let Some(lid) = local.id {
                    let now_s = now_unix();
                    cache::worklogs::update_fields(
                        db,
                        lid,
                        issue_key,
                        local.issue_id.as_deref(),
                        local.summary.as_deref(),
                        snap.duration_s,
                        snap.started_at,
                        snap.comment.as_deref(),
                        Some(now_s),
                    )?;
                }
            }
            let after = cache::worklogs::get_by_jira_id(db, worklog_id)?;
            record_linked(
                db,
                AuditOp::Retry,
                Some(issue_key),
                Some(worklog_id),
                None,
                after.as_ref(),
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "update",
                "worklog_id": worklog_id,
            }))
        }
        Err(e) => {
            record_linked(
                db,
                AuditOp::Retry,
                Some(issue_key),
                Some(worklog_id),
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Jira(e))
        }
    }
}

async fn retry_delete(
    client: &JiraClient,
    db: &Db,
    entry: &AuditEntry,
) -> Result<Value, ReconstructError> {
    let worklog_id = entry
        .worklog_id
        .as_deref()
        .ok_or(ReconstructError::SnapshotMissing)?;
    let issue_key = entry.issue_key.as_deref().unwrap_or("");
    match client.delete_worklog(issue_key, worklog_id).await {
        Ok(()) | Err(JiraError::WorklogNotFound) => {
            let now_s = now_unix();
            cache::worklogs::mark_tombstoned_by_jira_id(db, worklog_id, now_s)?;
            record_linked(
                db,
                AuditOp::Retry,
                Some(issue_key),
                Some(worklog_id),
                None,
                None,
                true,
                None,
                entry.id,
            );
            Ok(serde_json::json!({
                "op": "delete",
                "worklog_id": worklog_id,
            }))
        }
        Err(e) => {
            record_linked(
                db,
                AuditOp::Retry,
                Some(issue_key),
                Some(worklog_id),
                None,
                None,
                false,
                Some(&e.to_string()),
                entry.id,
            );
            Err(ReconstructError::Jira(e))
        }
    }
}
