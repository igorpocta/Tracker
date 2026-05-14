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
//!   show the original Trcker error string
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
    /// Account id of the current user — used to populate the new row.
    pub author_account_id: Option<&'a str>,
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
    /// show the original Trcker error message including `old_issue_key`).
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

    // Build the new row from the response (with sensible fallbacks).
    let started_ts = args.started.timestamp();
    let now_ts = Utc::now().timestamp();
    let new_issue_id_fallback = create_resp.issue_id.clone();

    // Look up summary/issue_id from the issue cache, falling back to the
    // Jira response's issueId.
    let (issue_id, summary) = match cache::issues::get_by_key(db, args.new_issue_key)? {
        Some(row) => (row.issue_id.or(new_issue_id_fallback), Some(row.summary)),
        None => (new_issue_id_fallback, None),
    };

    let new_row = WorklogRow {
        id: None,
        issue_key: args.new_issue_key.to_string(),
        issue_id,
        summary,
        duration_s: args.time_spent_seconds,
        started_at: started_ts,
        logged_at: now_ts,
        comment: args.comment.map(|s| s.to_string()),
        jira_worklog_id: Some(create_resp.id.clone()),
        author_account_id: args.author_account_id.map(|s| s.to_string()),
        source: "jira".to_string(),
        updated_at_jira: Some(now_ts),
        pending_delete_at: None,
        tombstoned_at: None,
    };

    // Step 2: DELETE the old worklog.
    if let Err(e) = client
        .delete_worklog(args.old_issue_key, args.old_worklog_id)
        .await
    {
        // Treat 404 as "already gone, OK" — the old worklog is no longer in
        // Jira anyway, so we still want to proceed with the success path.
        if !matches!(e, JiraError::WorklogNotFound) {
            // The new worklog exists; persist it locally so the user sees it,
            // but leave the old row intact (still mapped to old_issue_key).
            cache::worklogs::upsert_from_jira(db, &new_row)?;
            return Err(MoveWorklogError::DeleteAfterCreate {
                new_worklog_id: create_resp.id,
                old_issue_key: args.old_issue_key.to_string(),
                source: e,
            });
        }
    }

    // Step 3: full success. Insert/upsert the new row; remove the old.
    cache::worklogs::upsert_from_jira(db, &new_row)?;

    // Hard-delete the old local row by jira id (it might or might not be in
    // the DB — sync may not have caught it yet). Best-effort.
    if let Some(old_row) = cache::worklogs::get_by_jira_id(db, args.old_worklog_id)? {
        if let Some(id) = old_row.id {
            cache::worklogs::delete_row(db, id)?;
        }
    }

    Ok(MoveWorklogResult {
        new_worklog_id: create_resp.id,
        new_row,
    })
}
