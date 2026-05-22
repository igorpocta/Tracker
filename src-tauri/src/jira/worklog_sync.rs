//! Worklog sync — pulls worklogs from Jira for a given date range and
//! upserts them into the local `worklogs` cache.
//!
//! Strategy:
//! 1. Resolve the current user's `accountId` via `client.myself()`.
//! 2. Run a JQL search for issues where that user logged worklogs in the
//!    range (uses `worklogAuthor` + `worklogDate` filters).
//! 3. For each issue, page over `/issue/{key}/worklog`.
//! 4. Filter entries: keep only the ones authored by the current user
//!    whose `started` falls inside the requested window.
//! 5. Upsert each surviving entry into `worklogs` via `upsert_from_remote`.
//! 6. Mark-and-sweep: any local row keyed by `(connection_id, remote_id)`
//!    inside the window that the sync did NOT return is tombstoned.
//!
//! Tombstoned rows stay in the database forever — they form the local
//! audit trail.

use chrono::{NaiveDate, Utc};
use thiserror::Error;

use super::adf::extract_adf_text;
use super::client::{JiraClient, JiraError};
use super::models::{map_issue_to_row, parse_jira_timestamp_public, JiraWorklog};
use crate::cache::{self, worklogs::WorklogRow, Db, DbError};

/// Errors produced by [`sync_worklogs_for_range`].
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("jira: {0}")]
    Jira(#[from] JiraError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("invalid date range: {0}")]
    InvalidRange(String),
}

/// Page size used when paginating per-issue worklog lists. Jira's API max
/// is effectively 1000 for this endpoint and time-tracking volumes never
/// get near that — we still paginate just in case.
const ISSUE_WORKLOG_PAGE_SIZE: u32 = 1000;

/// Returns `true` if `started_ts` (UTC unix seconds) falls inside the
/// LOCAL calendar day range `[from, to]`. Used by sync to decide whether
/// a worklog Jira returned should land in this window's slice of the
/// local cache.
#[allow(dead_code)]
fn is_in_local_day_window(started_ts: i64, from: NaiveDate, to: NaiveDate) -> bool {
    match crate::time::local_day_bounds(from, to) {
        Some((from_ts, to_ts)) => started_ts >= from_ts && started_ts <= to_ts,
        None => false,
    }
}

pub async fn sync_worklogs_for_range(
    client: &JiraClient,
    db: &Db,
    connection_id: i64,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> Result<usize, SyncError> {
    if to_date < from_date {
        return Err(SyncError::InvalidRange(format!(
            "to_date {to_date} is before from_date {from_date}"
        )));
    }

    let me = client.myself().await?;
    let me_account_id = me.account_id;

    let (from_ts, to_ts) = crate::time::local_day_bounds(from_date, to_date).ok_or_else(
        || SyncError::InvalidRange("local day bounds are ambiguous (DST?)".into()),
    )?;

    // JQL: account ids are [a-z0-9:-] and don't need quoting.
    let jql = format!(
        r#"worklogAuthor = "{me}" AND worklogDate >= "{from}" AND worklogDate <= "{to}""#,
        me = me_account_id,
        from = from_date,
        to = to_date,
    );

    let mut upserted = 0usize;
    let mut page_token: Option<String> = None;
    let mut seen_remote_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let now = Utc::now().timestamp();

    loop {
        let page = client
            .search_jql(
                &jql,
                page_token.as_deref(),
                &["summary", "status", "updated", "parent", "issuetype"],
                crate::jira::SYNC_PAGE_MAX_RESULTS,
            )
            .await?;

        for issue in &page.issues {
            // Make sure the issue exists in our cache.
            let issue_row = map_issue_to_row(issue, connection_id, now);
            cache::issues::upsert(db, &issue_row)?;

            // Page through this issue's worklogs.
            let mut start_at: u32 = 0;
            loop {
                let wl_page = client
                    .issue_worklogs(&issue.key, start_at, ISSUE_WORKLOG_PAGE_SIZE)
                    .await?;
                let returned = wl_page.worklogs.len() as u32;

                for wl in &wl_page.worklogs {
                    if wl.author.account_id != me_account_id {
                        continue;
                    }
                    let started_ts = match parse_jira_timestamp_public(&wl.started) {
                        Some(ts) => ts,
                        None => continue,
                    };
                    if started_ts < from_ts || started_ts > to_ts {
                        continue;
                    }

                    let row = jira_worklog_to_row(wl, &issue.key, started_ts, connection_id, now);
                    cache::worklogs::upsert_from_remote(db, &row)?;
                    seen_remote_ids.insert(wl.id.clone());
                    upserted += 1;
                }

                start_at += returned;
                if returned == 0 || start_at >= wl_page.total || returned < ISSUE_WORKLOG_PAGE_SIZE
                {
                    break;
                }
            }
        }

        if page.is_last || page.next_page_token.is_none() {
            break;
        }
        page_token = page.next_page_token;
    }

    // Mark-and-sweep: any local row for this connection inside the window
    // whose `remote_id` wasn't returned this pass is presumed deleted.
    // Tombstoned rows stay forever — the audit trail is complete.
    let local_ids = cache::worklogs::remote_ids_in_range(db, connection_id, from_ts, to_ts)?;
    for remote_id in &local_ids {
        if !seen_remote_ids.contains(remote_id) {
            cache::worklogs::mark_tombstoned_by_remote_id(db, connection_id, remote_id, now)?;
            let before = cache::worklogs::get_by_remote_id(db, connection_id, remote_id)
                .ok()
                .flatten();
            let _ = crate::cache::audit::record(
                db,
                crate::cache::audit::AuditEvent {
                    occurred_at: now,
                    op: crate::cache::audit::AuditOp::SyncTombstone,
                    issue_key: before.as_ref().and_then(|r| r.issue_key.as_deref()),
                    worklog_id: Some(remote_id.as_str()),
                    before: before.as_ref(),
                    after: None,
                    success: true,
                    error: None,
                    source_audit_id: None,
                },
            );
        }
    }

    Ok(upserted)
}

/// Convert a `JiraWorklog` into the multi-provider `WorklogRow`.
fn jira_worklog_to_row(
    wl: &JiraWorklog,
    issue_key: &str,
    started_ts: i64,
    connection_id: i64,
    now: i64,
) -> WorklogRow {
    let updated_ts = wl.updated.as_str().pipe(parse_jira_timestamp_public);
    let logged_ts = updated_ts.unwrap_or(started_ts);
    let comment_text = wl
        .comment
        .as_ref()
        .map(extract_adf_text)
        .filter(|s| !s.is_empty());

    WorklogRow {
        id: None,
        connection_id: Some(connection_id),
        issue_key: Some(issue_key.to_string()),
        description: comment_text,
        started_at: started_ts,
        ended_at: started_ts.saturating_add(wl.time_spent_seconds.max(0)),
        logged_at: logged_ts,
        updated_at: now,
        is_synced: true,
        synced_at: Some(now),
        remote_id: Some(wl.id.clone()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    }
}

/// Tiny `.pipe` for chaining without `let` boilerplate.
trait Pipe: Sized {
    fn pipe<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn after_midnight_local_is_kept_in_target_day_window() {
        // 2026-05-14 00:30 local — must fall inside 2026-05-14 window.
        // Note: this test uses `chrono::Local` (system timezone). It still
        // passes regardless of system timezone because both `started` and
        // the window bounds inside `is_in_local_day_window` use the same
        // `Local`.
        let day = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let started = chrono::Local
            .with_ymd_and_hms(2026, 5, 14, 0, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        assert!(
            is_in_local_day_window(started, day, day),
            "post-midnight worklog must stay in the local day it was logged in"
        );
    }

    #[test]
    fn pre_midnight_local_is_kept_in_previous_day_window() {
        let prev = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        let started = chrono::Local
            .with_ymd_and_hms(2026, 5, 13, 23, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        assert!(is_in_local_day_window(started, prev, prev));
    }
}
