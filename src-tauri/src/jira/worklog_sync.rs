//! Worklog sync — pulls worklogs from Jira for a given date range and
//! upserts them into the local cache.
//!
//! Strategy:
//! 1. Run a JQL search for issues where the current user has logged worklogs
//!    in the requested range. JQL's `worklogAuthor` + `worklogDate` filters
//!    are what we need; we use the existing paged search and the issue
//!    "summary" field to keep the per-issue upsert cheap.
//! 2. For each issue, page over `/issue/{key}/worklog`.
//! 3. Filter entries: keep only the ones authored by `me_account_id` whose
//!    `started` falls inside `[from_date, to_date]` (inclusive).
//! 4. Upsert each surviving entry via `worklogs::upsert_from_jira`.
//!
//! Returns the count of upserted worklog rows.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
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

/// Page size used when paginating per-issue worklog lists. Jira's API max is
/// effectively 1000 for this endpoint and time tracking volumes never get
/// near that — we still paginate in case someone has a hyper-active issue.
const ISSUE_WORKLOG_PAGE_SIZE: u32 = 1000;

/// How long we keep tombstoned rows around before hard-deleting them. The
/// rows are excluded from the default worklog queries via the
/// `tombstoned_at IS NULL` filter, but we hold onto them as a forensic
/// audit trail.
const TOMBSTONE_RETENTION_SECONDS: i64 = 30 * 24 * 60 * 60;

/// Fetch worklogs authored by `me_account_id` between `from_date` and
/// `to_date` (inclusive), and upsert them into the local cache.
///
/// Also runs **mark-and-sweep**: any local row with `source='jira'` whose
/// `jira_worklog_id` was *not* returned by this sync (and which falls inside
/// the requested range) is presumed deleted upstream and gets tombstoned
/// locally. This catches the case where the user deletes a worklog directly
/// in the Jira web UI between syncs.
///
/// Tombstoned rows older than [`TOMBSTONE_RETENTION_SECONDS`] are then
/// hard-deleted.
///
/// Returns the number of worklog rows upserted (deduplicated by
/// `jira_worklog_id` via [`cache::worklogs::upsert_from_jira`]).
pub async fn sync_worklogs_for_range(
    client: &JiraClient,
    db: &Db,
    me_account_id: &str,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> Result<usize, SyncError> {
    if to_date < from_date {
        return Err(SyncError::InvalidRange(format!(
            "to_date {to_date} is before from_date {from_date}"
        )));
    }

    // Convert the date range to inclusive unix seconds. We use UTC bounds:
    // anything started on `from_date` 00:00 UTC up to `to_date` 23:59:59 UTC.
    let from_ts = Utc
        .with_ymd_and_hms(from_date.year(), from_date.month(), from_date.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| SyncError::InvalidRange("from_date is ambiguous".into()))?
        .timestamp();
    let to_ts = Utc
        .with_ymd_and_hms(to_date.year(), to_date.month(), to_date.day(), 23, 59, 59)
        .single()
        .ok_or_else(|| SyncError::InvalidRange("to_date is ambiguous".into()))?
        .timestamp();

    // Build JQL. `worklogAuthor` and `worklogDate` are both indexed by Jira.
    // We don't bother escaping `me_account_id` because account ids are
    // [a-z0-9:-] and never contain quotes.
    let jql = format!(
        r#"worklogAuthor = "{me}" AND worklogDate >= "{from}" AND worklogDate <= "{to}""#,
        me = me_account_id,
        from = from_date,
        to = to_date,
    );

    let mut upserted = 0usize;
    let mut page_token: Option<String> = None;
    // Collect the set of Jira worklog ids we saw in this sync — used by the
    // mark-and-sweep step below.
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        let page = client
            .search_jql(
                &jql,
                page_token.as_deref(),
                &["summary", "updated"],
                crate::jira::SYNC_PAGE_MAX_RESULTS,
            )
            .await?;

        for issue in &page.issues {
            // Make sure the issue exists in our cache (otherwise the worklog
            // row would have no summary/issue_id fallback).
            cache::issues::upsert(db, &map_issue_to_row(issue))?;

            // Page through this issue's worklogs.
            let mut start_at: u32 = 0;
            loop {
                let wl_page = client
                    .issue_worklogs(&issue.key, start_at, ISSUE_WORKLOG_PAGE_SIZE)
                    .await?;
                let returned = wl_page.worklogs.len() as u32;

                for wl in &wl_page.worklogs {
                    // Skip entries not by the current user.
                    if wl.author.account_id != me_account_id {
                        continue;
                    }
                    // Skip entries outside the requested range.
                    let started_ts = match parse_jira_timestamp_public(&wl.started) {
                        Some(ts) => ts,
                        None => continue,
                    };
                    if started_ts < from_ts || started_ts > to_ts {
                        continue;
                    }

                    let row = jira_worklog_to_row(wl, &issue.key, started_ts);
                    cache::worklogs::upsert_from_jira(db, &row)?;
                    seen_ids.insert(wl.id.clone());
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

    // ----- Mark-and-sweep -----
    //
    // Any `source='jira'` row whose started_at falls inside our query window
    // and whose `jira_worklog_id` was NOT returned this pass is presumed
    // deleted upstream. Tombstone it locally.
    //
    // Note: we filter on author too because the sync itself is author-scoped
    // — without that filter we'd tombstone other users' worklogs if the
    // current user can see them (which doesn't happen in our schema today
    // because we only sync our own, but defensive is cheap).
    let local_ids =
        cache::worklogs::jira_ids_in_range(db, from_ts, to_ts, me_account_id)?;
    let now_unix = Utc::now().timestamp();
    for local_id in &local_ids {
        if !seen_ids.contains(local_id) {
            cache::worklogs::mark_tombstoned_by_jira_id(db, local_id, now_unix)?;
            // Audit the synthetic tombstone for traceability.
            let _ = crate::cache::audit::record(
                db,
                crate::cache::audit::AuditEvent {
                    occurred_at: now_unix,
                    op: crate::cache::audit::AuditOp::SyncTombstone,
                    issue_key: None,
                    worklog_id: Some(local_id.as_str()),
                    before: None,
                    after: None,
                    success: true,
                    error: None,
                },
            );
        }
    }

    // Hard-delete tombstoned rows older than the retention window.
    let retention_cutoff = now_unix - TOMBSTONE_RETENTION_SECONDS;
    cache::worklogs::purge_old_tombstoned(db, retention_cutoff)?;

    Ok(upserted)
}

/// Convert a `JiraWorklog` to the local row shape. Caller supplies the
/// already-parsed `started_ts` to avoid parsing the timestamp twice.
fn jira_worklog_to_row(wl: &JiraWorklog, issue_key: &str, started_ts: i64) -> WorklogRow {
    let logged_ts = wl
        .updated
        .as_str()
        .pipe(parse_jira_timestamp_public)
        .unwrap_or(started_ts);
    let updated_ts = wl.updated.as_str().pipe(parse_jira_timestamp_public);

    let comment_text = wl
        .comment
        .as_ref()
        .map(extract_adf_text)
        .filter(|s| !s.is_empty());

    WorklogRow {
        id: None,
        issue_key: issue_key.to_string(),
        issue_id: wl.issue_id.clone(),
        summary: None,
        duration_s: wl.time_spent_seconds,
        started_at: started_ts,
        logged_at: logged_ts,
        comment: comment_text,
        jira_worklog_id: Some(wl.id.clone()),
        author_account_id: Some(wl.author.account_id.clone()),
        source: "jira".to_string(),
        updated_at_jira: updated_ts,
        pending_delete_at: None,
        tombstoned_at: None,
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
