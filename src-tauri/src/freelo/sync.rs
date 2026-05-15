//! Sync logic for Freelo: pull projects/tasks into the shared `issues` cache
//! and work-reports into `recent_worklogs`.
//!
//! Mirrors the Jira sync behaviour with mark-and-sweep tombstoning.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use thiserror::Error;

use super::client::{FreeloClient, FreeloError};
use super::models::{project_to_issue_row, task_to_issue_row, FreeloWorkReport};
use crate::cache::{self, worklogs::WorklogRow, Db, DbError};

/// Errors from [`sync_issues_for_connection`] / [`sync_worklogs_for_range`].
#[derive(Debug, Error)]
pub enum FreeloSyncError {
    #[error("freelo: {0}")]
    Freelo(#[from] FreeloError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("invalid date range: {0}")]
    InvalidRange(String),
}

/// Pull all selected projects + their tasks into the local `issues` cache.
///
/// `selected_project_ids` is the per-connection list maintained by the
/// `set_freelo_selected_projects` command. If empty, no tasks are synced and
/// the function returns 0.
///
/// Returns the number of task rows upserted.
pub async fn sync_issues_for_connection(
    client: &FreeloClient,
    db: &Db,
    selected_project_ids: &[i64],
) -> Result<usize, FreeloSyncError> {
    if selected_project_ids.is_empty() {
        tracing::warn!(
            "freelo: sync_issues called with no selected projects; skipping"
        );
        return Ok(0);
    }

    // Pull the projects list so we can match selected ids to project metadata.
    let projects = client.list_projects().await?;
    let now = Utc::now().timestamp();

    let mut upserted = 0usize;
    for project_id in selected_project_ids {
        let Some(project) = projects.iter().find(|p| p.id == *project_id) else {
            tracing::warn!(
                "freelo: selected project {project_id} not found in /all-projects; skipping"
            );
            continue;
        };
        // Upsert the project itself so it can be looked up via parent_key /
        // epic_key.
        cache::issues::upsert(db, &project_to_issue_row(project, now))?;

        let tasks = match client.list_tasks_for_project(*project_id).await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "freelo: list_tasks_for_project({project_id}) failed: {e}"
                );
                continue;
            }
        };
        for t in &tasks {
            cache::issues::upsert(db, &task_to_issue_row(t, project, now))?;
            upserted += 1;
        }
    }
    Ok(upserted)
}

/// Pull work-reports authored by `user_id` between `from` and `to` and upsert
/// them into `recent_worklogs`. Mark-and-sweep tombstones any local rows that
/// the Freelo API no longer returns.
///
/// Returns the count of upserted rows.
pub async fn sync_worklogs_for_range(
    client: &FreeloClient,
    db: &Db,
    user_id: i64,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<usize, FreeloSyncError> {
    if to < from {
        return Err(FreeloSyncError::InvalidRange(format!(
            "to {to} is before from {from}"
        )));
    }

    let entries = client.list_work_reports(from, to, user_id).await?;
    let mut upserted = 0usize;
    let mut seen_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    for e in &entries {
        let row = work_report_to_row(e);
        cache::worklogs::upsert_from_jira(db, &row)?;
        if let Some(id) = row.jira_worklog_id {
            seen_ids.insert(id);
        }
        upserted += 1;
    }

    // Range bounds in unix seconds (UTC start-of-day to end-of-day).
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

    // Mark-and-sweep: anything in our DB whose started_at falls in the
    // window AND has the freelo: prefix AND was not seen → tombstone.
    let local_ids = cache::worklogs::jira_ids_in_range(
        db,
        from_ts,
        to_ts,
        &user_id.to_string(),
    )?;
    let now_unix = Utc::now().timestamp();
    for local_id in &local_ids {
        if !local_id.starts_with(super::FREELO_WORKLOG_PREFIX) {
            continue;
        }
        if !seen_ids.contains(local_id) {
            cache::worklogs::mark_tombstoned_by_jira_id(db, local_id, now_unix)?;
        }
    }

    Ok(upserted)
}

/// Convert a Freelo work-report into a `WorklogRow` for the shared cache.
///
/// Freelo only stores date (not time-of-day). We use midnight UTC of the
/// reported date as `started_at` so the row sorts predictably; the UI knows
/// to render the date without a clock time.
pub fn work_report_to_row(w: &FreeloWorkReport) -> WorklogRow {
    let started_at = NaiveDate::parse_from_str(&w.date_reported, "%Y-%m-%d")
        .ok()
        .and_then(|d| Utc.from_local_datetime(&d.and_hms_opt(0, 0, 0)?).single())
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp());

    WorklogRow {
        id: None,
        issue_key: super::task_key(w.task_id),
        issue_id: Some(w.task_id.to_string()),
        summary: None,
        duration_s: w.minutes.saturating_mul(60),
        started_at,
        logged_at: started_at,
        comment: w.description.clone(),
        jira_worklog_id: Some(super::worklog_id_key(w.id)),
        author_account_id: Some(w.user_id.to_string()),
        // We reuse `jira` so the existing UI code (which filters by
        // `source = 'jira'` to mean "synced from remote") keeps working.
        source: "jira".to_string(),
        updated_at_jira: Some(started_at),
        pending_delete_at: None,
        tombstoned_at: None,
        pending_assignment: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_report_minutes_become_seconds() {
        let w = FreeloWorkReport {
            id: 1,
            task_id: 99,
            minutes: 15,
            date_reported: "2026-05-14".into(),
            description: Some("foo".into()),
            user_id: 7,
        };
        let row = work_report_to_row(&w);
        assert_eq!(row.duration_s, 900);
        assert_eq!(row.issue_key, "FRL-99");
        assert_eq!(row.jira_worklog_id.as_deref(), Some("freelo:1"));
        assert_eq!(row.author_account_id.as_deref(), Some("7"));
        assert_eq!(row.source, "jira");
    }
}
