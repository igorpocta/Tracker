//! Composite worklog operations (Phase 15).
//!
//! Some user actions don't map 1:1 to a Jira primitive. The canonical example
//! is **move**: there is no `POST /worklog/move` endpoint, so we model it as
//! `POST new + DELETE old`. The trick is the failure semantics:
//!
//! - If the `POST new` fails, we abort cleanly. The original worklog still
//!   exists in Jira; the user's data is untouched.
//! - If the `POST new` succeeds but the `DELETE old` fails, we return a
//!   dedicated error variant carrying the **new** worklog id so the caller can
//!   show the original Tracker error string
//!   (`"Original worklog still exists on {old_issue_key}"`) and offer a
//!   retry-the-delete affordance. The new worklog is already in Jira.
//!
//! The DB side mirrors this: we only delete the old local row after the full
//! sequence succeeds. On the "delete failed" case we leave the old row
//! intact and insert/upsert the new one so the user can see both copies.

use chrono::{DateTime, Utc};
use thiserror::Error;

use super::client::{JiraClient, JiraError};
use crate::cache::{self, worklogs::WorklogRow, Db, DbError};

/// Input arguments for [`move_worklog`].
#[derive(Debug, Clone)]
pub struct MoveWorklogArgs<'a> {
    pub old_issue_key: &'a str,
    pub old_worklog_id: &'a str,
    pub new_issue_key: &'a str,
    pub started: DateTime<Utc>,
    pub time_spent_seconds: i64,
    pub comment: Option<&'a str>,
    /// Connection the OLD worklog belongs to. Used when the new issue key
    /// isn't in the local issue cache yet — a move stays within the same Jira
    /// host, so the old row's connection is the correct owner for the new row.
    /// Without it the new row would get `connection_id = None`, which
    /// `upsert_from_remote` rejects, orphaning the move after the upstream
    /// POST+DELETE already succeeded.
    pub fallback_connection_id: Option<i64>,
}

/// Successful result of [`move_worklog`].
#[derive(Debug, Clone)]
pub struct MoveWorklogResult {
    pub new_worklog_id: String,
    pub new_row: WorklogRow,
}

/// Errors produced by [`move_worklog`].
#[derive(Debug, Error)]
pub enum MoveWorklogError {
    /// The new worklog could not be created. The old worklog is untouched.
    #[error("create failed: {0}")]
    CreateFailed(#[source] JiraError),
    /// The new worklog was created but the delete of the old worklog failed.
    /// The new worklog id is included so the caller can retry the delete (or
    /// show the original Tracker error message including `old_issue_key`).
    #[error("delete-after-create failed (new worklog {new_worklog_id} exists, old still on {old_issue_key}): {source}")]
    DeleteAfterCreate {
        new_worklog_id: String,
        old_issue_key: String,
        #[source]
        source: JiraError,
    },
    #[error("db: {0}")]
    Db(#[from] DbError),
}

/// Move a worklog from `old_issue_key` to `new_issue_key`.
///
/// Strategy:
/// 1. POST new worklog. If it fails → return `CreateFailed`; nothing changes.
/// 2. DELETE old worklog. If it fails → return `DeleteAfterCreate { new_id, .. }`;
///    the new worklog is in Jira, the old one still is too. The local DB is
///    updated to reflect both (the new row is inserted; the old row stays).
/// 3. Full success: insert/upsert the new row, hard-delete the old local row.
pub async fn move_worklog(
    client: &JiraClient,
    db: &Db,
    args: MoveWorklogArgs<'_>,
) -> Result<MoveWorklogResult, MoveWorklogError> {
    // Step 1: POST the new worklog. Bail out cleanly on failure.
    let create_resp = client
        .add_worklog(
            args.new_issue_key,
            args.started,
            args.time_spent_seconds,
            args.comment,
        )
        .await
        .map_err(MoveWorklogError::CreateFailed)?;

    // Build the new row from the response.
    let started_ts = args.started.timestamp();
    let now_ts = Utc::now().timestamp();

    // Resolve owning connection from the issue cache; if the new issue isn't
    // cached yet, fall back to the OLD worklog's connection (a move stays on
    // the same Jira host). Without the fallback the new row's connection_id is
    // None, which upsert_from_remote rejects — orphaning the move after the
    // upstream POST+DELETE already committed.
    let connection_id = cache::issues::get_connection_id_by_key(db, args.new_issue_key)?
        .or(args.fallback_connection_id);

    let new_row = WorklogRow {
        id: None,
        connection_id,
        issue_key: Some(args.new_issue_key.to_string()),
        description: args.comment.map(|s| s.to_string()),
        started_at: started_ts,
        ended_at: started_ts.saturating_add(args.time_spent_seconds.max(0)),
        logged_at: now_ts,
        updated_at: now_ts,
        is_synced: true,
        synced_at: Some(now_ts),
        remote_id: Some(create_resp.id.clone()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };

    // Step 2: DELETE the old worklog.
    if let Err(e) = client
        .delete_worklog(args.old_issue_key, args.old_worklog_id)
        .await
    {
        if !matches!(e, JiraError::WorklogNotFound) {
            cache::worklogs::upsert_from_remote(db, &new_row)?;
            return Err(MoveWorklogError::DeleteAfterCreate {
                new_worklog_id: create_resp.id,
                old_issue_key: args.old_issue_key.to_string(),
                source: e,
            });
        }
    }

    // Step 3: full success. Insert/upsert the new row; remove the old.
    cache::worklogs::upsert_from_remote(db, &new_row)?;

    // Hard-delete the old local row by remote id. The connection that owned
    // it is the same one we just resolved (issue is moving within the same
    // Jira instance). Best-effort — sync may not have caught it yet.
    if let Some(conn_id) = connection_id {
        if let Some(old_row) = cache::worklogs::get_by_remote_id(db, conn_id, args.old_worklog_id)?
        {
            if let Some(id) = old_row.id {
                cache::worklogs::delete_row(db, id)?;
            }
        }
    }

    Ok(MoveWorklogResult {
        new_worklog_id: create_resp.id,
        new_row,
    })
}
