//! Create / update / delete operations for Freelo work-reports, mapping the
//! shared command surface onto the Freelo API.

use chrono::{DateTime, Local, TimeZone, Utc};
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
/// Only `seconds <= 0` is rejected. Any positive duration yields at least 1
/// minute: Freelo's granularity is minutes, and rounding a sub-minute entry
/// down to 0 used to reject it — the row then saved local-only and was
/// re-POSTed (and re-rejected) on every sync forever. Rounding up to 1 keeps
/// it syncable (the lesser evil vs. permanently stuck, un-invoiced time).
pub fn seconds_to_minutes(seconds: i64) -> Result<i64, FreeloOpError> {
    if seconds <= 0 {
        return Err(FreeloOpError::DurationTooShort);
    }
    // Round to nearest minute, never below 1.
    Ok(((seconds + 30) / 60).max(1))
}

/// Convert a unix-milliseconds timestamp to a `DateTime<Local>` for the Freelo
/// API.
///
/// Freelo bere `date_reported` jako plný RFC3339 timestamp s TZ a uloží přesně,
/// co pošleme. Lokální TZ použijeme proto, ať se výsledný offset shoduje
/// s časem, který uživatel viděl v UI (`2026-05-21T00:30:00+02:00`); konverze
/// přes UTC by způsobila, že Freelo zobrazí čas posunutý do UTC.
fn ms_to_local_datetime_in_tz<TZ: TimeZone>(tz: &TZ, ms: i64) -> Option<DateTime<TZ>> {
    tz.timestamp_millis_opt(ms).single()
}

/// Convert a unix milliseconds timestamp to a `DateTime<Local>` for the Freelo
/// API. Falls back to "now" when the input is out of range (defensive — UI
/// shouldn't produce such values).
pub fn ms_to_local_datetime(ms: i64) -> DateTime<Local> {
    ms_to_local_datetime_in_tz(&Local, ms).unwrap_or_else(Local::now)
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
    let started_at = ms_to_local_datetime(started_at_ms);

    let resp = client
        .create_work_report(task_id, started_at.fixed_offset(), minutes, comment)
        .await?;

    let now = Utc::now().timestamp();
    let mut row = super::sync::work_report_to_row(&resp, connection_id, now);
    // Freelo echoes `date_reported` přesně tak, jak jsme ho poslali, takže
    // `row.started_at` z work_report_to_row už nese reálný čas. Délku
    // přepočítáme z minut, aby `ended_at` sedělo s tím, co Freelo uloží
    // a vrátí na příští sync (zaokrouhleno na minuty).
    let started_at_s = started_at_ms / 1000;
    let duration_s = minutes.saturating_mul(60);
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
    let started_at_param = new_started_at_ms
        .map(ms_to_local_datetime)
        .map(|dt| dt.fixed_offset());
    let resp = client
        .update_work_report(work_report_id, minutes, started_at_param, new_comment)
        .await?;

    // Reuse the local row's connection_id so we don't lose ownership during
    // an edit — Freelo's POST /work-reports/{id} response doesn't carry it.
    let existing = cache::worklogs::get_by_id(db, local_id)?.ok_or_else(|| {
        FreeloOpError::Db(DbError::Migration("row disappeared before update".into()))
    })?;
    let connection_id = existing.connection_id.unwrap_or(0);
    let now = Utc::now().timestamp();
    let mut row = super::sync::work_report_to_row(&resp, connection_id, now);
    // Freelo echoes plný timestamp, ale na update response někdy chybí
    // `date_reported` úplně (echo jen toho, co jsme v PATCH poslali). Když
    // přišel nový čas z UI, vezmeme ho; jinak držíme původní lokální hodnotu.
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

    #[test]
    fn seconds_to_minutes_floors_subminute_to_one() {
        // Regression: 1..30s rounded to 0 -> Err, so a sub-minute Freelo entry
        // saved local-only and was re-POSTed (and re-rejected) on every sync
        // forever. Any positive duration must yield at least 1 syncable minute.
        assert_eq!(seconds_to_minutes(1).unwrap(), 1);
        assert_eq!(seconds_to_minutes(20).unwrap(), 1);
        assert_eq!(seconds_to_minutes(29).unwrap(), 1);
    }

    #[test]
    fn ms_to_local_datetime_preserves_wall_clock_in_local_tz() {
        let tz = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let ms = tz
            .with_ymd_and_hms(2026, 5, 21, 9, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        let dt = ms_to_local_datetime_in_tz(&tz, ms).unwrap();
        // RFC3339 musí nést stejný wall-clock i offset, jaký uživatel viděl.
        assert_eq!(
            dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            "2026-05-21T09:00:00+02:00"
        );
    }
}
