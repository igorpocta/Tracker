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
    let date_str = date.format("%Y-%m-%d").to_string();
    let reports = client
        .list_work_reports(date, date, user_id, &[])
        .await
        .ok()?;
    reports.into_iter().find(|r| {
        r.task_id == task_id
            && r.user_id == user_id
            && r.minutes == minutes
            && r.date_reported == date_str
    })
}
