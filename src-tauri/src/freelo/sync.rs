//! Freelo sync — pulls tasks into `issues_v2` and work-reports into
//! `worklogs`. Mirrors the Jira sync's mark-and-sweep semantics.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use thiserror::Error;

use super::client::{FreeloClient, FreeloError};
use super::models::{task_to_issue_row, FreeloWorkReport};
use crate::cache::{self, worklogs::WorklogRow, Db, DbError};

#[derive(Debug, Error)]
pub enum FreeloSyncError {
    #[error("freelo: {0}")]
    Freelo(#[from] FreeloError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("invalid date range: {0}")]
    InvalidRange(String),
}

/// Pull all tasks from the selected projects into `issues_v2`. Projects
/// themselves are NOT cached as issues — they aren't trackable units in
/// Freelo; they only appear as parent context on each task's row.
pub async fn sync_issues_for_connection(
    client: &FreeloClient,
    db: &Db,
    connection_id: i64,
    selected_project_ids: &[i64],
) -> Result<usize, FreeloSyncError> {
    if selected_project_ids.is_empty() {
        tracing::warn!("freelo: sync_issues called with no selected projects; skipping");
        return Ok(0);
    }

    let tasks = client.list_tasks_for_projects(selected_project_ids).await?;
    let now = Utc::now().timestamp();

    let mut upserted = 0usize;
    for t in &tasks {
        cache::issues::upsert(db, &task_to_issue_row(t, connection_id, now))?;
        upserted += 1;
    }
    Ok(upserted)
}

/// Pull work-reports authored by `user_id` between `from` and `to` and
/// upsert them into `worklogs`. Mark-and-sweep tombstones any local rows
/// that the Freelo API no longer returns.
pub async fn sync_worklogs_for_range(
    client: &FreeloClient,
    db: &Db,
    connection_id: i64,
    user_id: i64,
    from: NaiveDate,
    to: NaiveDate,
    project_ids: &[i64],
) -> Result<usize, FreeloSyncError> {
    if to < from {
        return Err(FreeloSyncError::InvalidRange(format!(
            "to {to} is before from {from}"
        )));
    }

    let entries = client
        .list_work_reports(from, to, user_id, project_ids)
        .await?;

    let now = Utc::now().timestamp();
    let mut upserted = 0usize;
    let mut seen_remote_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for e in &entries {
        let row = work_report_to_row(e, connection_id, now);
        cache::worklogs::upsert_from_remote(db, &row)?;
        if let Some(id) = &row.remote_id {
            seen_remote_ids.insert(id.clone());
        }
        upserted += 1;
    }

    let from_ts = Utc
        .with_ymd_and_hms(from.year(), from.month(), from.day(), 0, 0, 0)
        .single()
        .ok_or_else(|| FreeloSyncError::InvalidRange("from is ambiguous".into()))?
        .timestamp();
    let to_ts = Utc
        .with_ymd_and_hms(to.year(), to.month(), to.day(), 23, 59, 59)
        .single()
        .ok_or_else(|| FreeloSyncError::InvalidRange("to is ambiguous".into()))?
        .timestamp();

    let local_ids = cache::worklogs::remote_ids_in_range(db, connection_id, from_ts, to_ts)?;
    for remote_id in &local_ids {
        if !seen_remote_ids.contains(remote_id) {
            cache::worklogs::mark_tombstoned_by_remote_id(db, connection_id, remote_id, now)?;
        }
    }

    Ok(upserted)
}

/// Convert a Freelo work-report into the multi-provider `WorklogRow`.
pub fn work_report_to_row(w: &FreeloWorkReport, connection_id: i64, now: i64) -> WorklogRow {
    // Freelo `date_reported` is either an ISO 8601 timestamp with TZ
    // ("2026-04-15T12:24:27+02:00") or a bare date ("2026-05-14") in
    // older shapes. Try the richer parse first.
    let started_at = chrono::DateTime::parse_from_rfc3339(&w.date_reported)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp())
        .or_else(|| {
            NaiveDate::parse_from_str(&w.date_reported, "%Y-%m-%d")
                .ok()
                .and_then(|d| Utc.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single())
                .map(|dt| dt.timestamp())
        })
        .unwrap_or(now);

    let duration_s = w.minutes.saturating_mul(60).max(0);

    WorklogRow {
        id: None,
        connection_id: Some(connection_id),
        issue_key: Some(super::task_key(w.task_id)),
        description: w
            .description
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        started_at,
        ended_at: started_at.saturating_add(duration_s),
        logged_at: started_at,
        updated_at: now,
        is_synced: true,
        synced_at: Some(now),
        remote_id: Some(w.id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_report_minutes_become_duration() {
        let w = FreeloWorkReport {
            id: 1,
            task_id: 99,
            task_name: Some("Vyřešit deploy pipeline".into()),
            minutes: 15,
            date_reported: "2026-05-14".into(),
            description: Some("foo".into()),
            user_id: 7,
        };
        let row = work_report_to_row(&w, 42, 1_700_000_000);
        assert_eq!(row.duration_s(), 900);
        assert_eq!(row.issue_key.as_deref(), Some("FREELO-99"));
        assert_eq!(row.remote_id.as_deref(), Some("1"));
        assert_eq!(row.connection_id, Some(42));
        assert!(row.is_synced);
    }

    #[test]
    fn blank_description_drops_to_none() {
        let w = FreeloWorkReport {
            id: 3,
            task_id: 101,
            task_name: None,
            minutes: 30,
            date_reported: "2026-05-14".into(),
            description: Some("   ".into()),
            user_id: 7,
        };
        assert!(work_report_to_row(&w, 42, 0).description.is_none());
    }
}
