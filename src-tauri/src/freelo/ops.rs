//! Create / update / delete operations for Freelo work-reports, mapping the
//! shared command surface onto the Freelo API.

use chrono::{NaiveDate, TimeZone, Utc};
use thiserror::Error;

use super::client::{FreeloClient, FreeloError};
// `work_report_to_row` is re-used by callers via `super::sync::work_report_to_row`.
use crate::cache::{self, worklogs::WorklogRow, Db, DbError};

/// Errors produced by the Freelo work-report ops.
#[derive(Debug, Error)]
pub enum FreeloOpError {
    #[error("freelo: {0}")]
    Freelo(#[from] FreeloError),
    #[error("db: {0}")]
    Db(#[from] DbError),
    #[error("invalid issue key: {0}")]
    InvalidIssueKey(String),
    #[error("minimum duration is 1 minute")]
    DurationTooShort,
}

/// Convert local seconds → Freelo minutes (rounding nearest, but at least 1).
///
/// Returns `Err(DurationTooShort)` if `seconds == 0` so the caller can
/// surface the same "Doba musí být alespoň minuta" message the user expects.
pub fn seconds_to_minutes(seconds: i64) -> Result<i64, FreeloOpError> {
    if seconds <= 0 {
        return Err(FreeloOpError::DurationTooShort);
    }
    // Round to nearest minute.
    let m = (seconds + 30) / 60;
    if m == 0 {
        return Err(FreeloOpError::DurationTooShort);
    }
    Ok(m)
}

/// Convert a unix milliseconds timestamp to a local-date for the Freelo API.
pub fn ms_to_date(ms: i64) -> NaiveDate {
    let dt = Utc
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
        .naive_utc();
    dt.date()
}

/// Add a new work-report to Freelo for the given task and record it locally.
///
/// `issue_key` must be a synthetic Freelo key (`FRL-{task_id}`); other
/// shapes are rejected with `InvalidIssueKey`.
#[allow(clippy::too_many_arguments)]
pub async fn add_work_report(
    client: &FreeloClient,
    db: &Db,
    issue_key: &str,
    started_at_ms: i64,
    duration_seconds: i64,
    comment: Option<&str>,
    connection_id: i64,
    _sync_user_id: i64,
) -> Result<WorklogRow, FreeloOpError> {
    let task_id = super::parse_task_key(issue_key)
        .ok_or_else(|| FreeloOpError::InvalidIssueKey(issue_key.to_string()))?;
    let minutes = seconds_to_minutes(duration_seconds)?;
    let date = ms_to_date(started_at_ms);

    let resp = client
        .create_work_report(task_id, date, minutes, comment)
        .await?;

    let now = Utc::now().timestamp();
    let mut row = super::sync::work_report_to_row(&resp, connection_id, now);
    // Freelo API ukládá jen datum (`date_reported`), ne čas. work_report_to_row
    // proto vrací `started_at = 00:00 UTC` daného dne — což by se ve výpisu
    // projevilo jako "0:00–0:39" místo skutečného intervalu časomíry.
    // Zachováme proto skutečný čas, který si pamatujeme lokálně.
    let started_at_s = started_at_ms / 1000;
    let duration_s = i64::from(minutes).saturating_mul(60);
    row.started_at = started_at_s;
    row.ended_at = started_at_s.saturating_add(duration_s);
    row.logged_at = started_at_s;
    let id = cache::worklogs::upsert_from_remote(db, &row)?;
    row.id = Some(id);

    Ok(row)
}

/// Update an existing Freelo work-report and reconcile the local row.
#[allow(clippy::too_many_arguments)]
pub async fn update_work_report(
    client: &FreeloClient,
    db: &Db,
    local_id: i64,
    work_report_id: i64,
    new_started_at_ms: Option<i64>,
    new_duration_seconds: Option<i64>,
    new_comment: Option<&str>,
) -> Result<WorklogRow, FreeloOpError> {
    let minutes = match new_duration_seconds {
        Some(s) => Some(seconds_to_minutes(s)?),
        None => None,
    };
    let date = new_started_at_ms.map(ms_to_date);
    let resp = client
        .update_work_report(work_report_id, minutes, date, new_comment)
        .await?;

    // Reuse the local row's connection_id so we don't lose ownership during
    // an edit — Freelo's POST /work-reports/{id} response doesn't carry it.
    let existing = cache::worklogs::get_by_id(db, local_id)?.ok_or_else(|| {
        FreeloOpError::Db(DbError::Migration("row disappeared before update".into()))
    })?;
    let connection_id = existing.connection_id.unwrap_or(0);
    let now = Utc::now().timestamp();
    let mut row = super::sync::work_report_to_row(&resp, connection_id, now);
    // Stejný důvod jako v add_work_report: Freelo response nese jen datum,
    // proto by row.started_at byl 00:00 UTC. Pokud nám přišel nový čas
    // z UI, použijeme ho; jinak zachováme původní hodnotu z lokálního řádku
    // (zachová čas, který uživatel viděl před editací).
    let started_at_s = new_started_at_ms
        .map(|ms| ms / 1000)
        .unwrap_or(existing.started_at);
    let effective_minutes = minutes.unwrap_or((existing.ended_at - existing.started_at) / 60);
    let duration_s = effective_minutes.saturating_mul(60);
    row.started_at = started_at_s;
    row.ended_at = started_at_s.saturating_add(duration_s);
    row.logged_at = started_at_s;
    cache::worklogs::update_fields(
        db,
        local_id,
        row.issue_key.as_deref(),
        row.description.as_deref(),
        row.started_at,
        row.ended_at,
        Some(now),
    )?;
    let after = cache::worklogs::get_by_id(db, local_id)?.ok_or_else(|| {
        FreeloOpError::Db(DbError::Migration("row disappeared after update".into()))
    })?;
    Ok(after)
}

/// Delete a Freelo work-report. Returns Ok if Freelo confirms the delete or
/// if the work-report is already gone (404 treated as success).
pub async fn delete_work_report(
    client: &FreeloClient,
    work_report_id: i64,
) -> Result<(), FreeloOpError> {
    match client.delete_work_report(work_report_id).await {
        Ok(()) => Ok(()),
        Err(FreeloError::WorkReportNotFound) => Ok(()),
        Err(e) => Err(FreeloOpError::Freelo(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_to_minutes_rejects_zero() {
        assert!(seconds_to_minutes(0).is_err());
    }

    #[test]
    fn seconds_to_minutes_rounds_nearest() {
        assert_eq!(seconds_to_minutes(30).unwrap(), 1);
        assert_eq!(seconds_to_minutes(60).unwrap(), 1);
        assert_eq!(seconds_to_minutes(90).unwrap(), 2);
        assert_eq!(seconds_to_minutes(89).unwrap(), 1);
    }
}
