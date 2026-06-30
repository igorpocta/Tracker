//! P1-4: pre-POST reconciliation.
//!
//! Worklog creation is remote-first: we POST to the provider, then persist the
//! returned `remote_id` locally. If the local write fails AFTER a successful
//! HTTP 201, the `remote_id` is lost — and a naive retry would POST again,
//! creating a duplicate upstream worklog.
//!
//! These helpers close that hole: before (re)POSTing, we ask the provider
//! whether a worklog matching this exact local intent already exists (same
//! issue/task, same author, same start instant, same duration). If so, the
//! caller adopts its id instead of creating a second one.
//!
//! Matching is deliberately strict — it keys on the start instant (to the
//! second) plus the duration and the authoring user, all of which Tracker
//! controls when it POSTs. A false positive would require a second, distinct
//! worklog by the same user on the same issue at the identical second and
//! duration, which the UI cannot produce.

use crate::freelo::client::FreeloClient;
use crate::jira::JiraClient;

/// Find a Jira worklog already created for this local intent. Returns the
/// remote worklog id to adopt, or `None` to proceed with a fresh POST.
///
/// Any provider error (network/auth) yields `None` so the caller falls back to
/// the normal POST path — reconciliation is a best-effort safety net, never a
/// hard dependency.
pub async fn find_existing_jira_worklog_id(
    client: &JiraClient,
    issue_key: &str,
    started_at_s: i64,
    duration_s: i64,
) -> Option<String> {
    let me = client.myself().await.ok()?;
    let page = client.issue_worklogs(issue_key, 0, 1000).await.ok()?;
    page.worklogs
        .into_iter()
        .find(|w| {
            w.author.account_id == me.account_id
                && w.time_spent_seconds == duration_s
                && crate::jira::models::parse_jira_timestamp_public(&w.started)
                    == Some(started_at_s)
        })
        .map(|w| w.id)
}

/// Freelo counterpart. `task_id` is the numeric Freelo task id, `minutes` the
/// rounded duration we would POST. Returns the existing report (so the caller
/// can build the local row from it) to adopt instead of POSTing again.
pub async fn find_existing_freelo_report(
    client: &FreeloClient,
    task_id: i64,
    user_id: i64,
    started_at_s: i64,
    minutes: i64,
) -> Option<crate::freelo::models::FreeloWorkReport> {
    use chrono::{Local, TimeZone};
    let date = Local.timestamp_opt(started_at_s, 0).single()?.date_naive();
    let reports = client
        .list_work_reports(date, date, user_id, &[])
        .await
        .ok()?;
    reports
        .into_iter()
        .find(|r| freelo_report_matches(r, task_id, user_id, minutes, date))
}

/// Parse Freelo's `date_reported` into a calendar date. The API returns it
/// either as a full RFC3339 timestamp (`2026-05-26T09:00:00+02:00`) or a bare
/// `YYYY-MM-DD`; both must be accepted.
fn freelo_report_date(date_reported: &str) -> Option<chrono::NaiveDate> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date_reported) {
        return Some(dt.date_naive());
    }
    chrono::NaiveDate::parse_from_str(date_reported, "%Y-%m-%d").ok()
}

/// Does an existing Freelo report match the local intent we're about to POST?
/// Keyed on task + author + rounded minutes + the calendar day. The day is
/// compared as a parsed date, NOT a string — Freelo returns a full timestamp,
/// so the old `date_reported == "YYYY-MM-DD"` string compare never matched and
/// the dedup silently re-POSTed (double-billing).
fn freelo_report_matches(
    r: &crate::freelo::models::FreeloWorkReport,
    task_id: i64,
    user_id: i64,
    minutes: i64,
    target_date: chrono::NaiveDate,
) -> bool {
    r.task_id == task_id
        && r.user_id == user_id
        && r.minutes == minutes
        && freelo_report_date(&r.date_reported) == Some(target_date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freelo::models::FreeloWorkReport;
    use chrono::NaiveDate;

    fn report(date_reported: &str, minutes: i64) -> FreeloWorkReport {
        FreeloWorkReport {
            id: 1,
            task_id: 100,
            task_name: None,
            minutes,
            date_reported: date_reported.to_string(),
            description: None,
            user_id: 7,
        }
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn freelo_report_date_parses_both_forms() {
        assert_eq!(freelo_report_date("2026-05-26"), Some(day(2026, 5, 26)));
        assert_eq!(
            freelo_report_date("2026-05-26T09:00:00+02:00"),
            Some(day(2026, 5, 26))
        );
        assert_eq!(freelo_report_date("garbage"), None);
    }

    #[test]
    fn matches_rfc3339_timestamp_date_reported() {
        // Regression: the API returns a full timestamp; the old string compare
        // against "YYYY-MM-DD" never matched, so dedup re-POSTed every retry.
        let r = report("2026-05-26T09:00:00+02:00", 30);
        assert!(freelo_report_matches(&r, 100, 7, 30, day(2026, 5, 26)));
    }

    #[test]
    fn matches_bare_date_reported() {
        let r = report("2026-05-26", 30);
        assert!(freelo_report_matches(&r, 100, 7, 30, day(2026, 5, 26)));
    }

    #[test]
    fn rejects_on_any_key_mismatch() {
        let r = report("2026-05-26T09:00:00+02:00", 30);
        let d = day(2026, 5, 26);
        assert!(!freelo_report_matches(&r, 999, 7, 30, d), "task");
        assert!(!freelo_report_matches(&r, 100, 8, 30, d), "user");
        assert!(!freelo_report_matches(&r, 100, 7, 45, d), "minutes");
        assert!(!freelo_report_matches(&r, 100, 7, 30, day(2026, 5, 27)), "date");
    }
}
