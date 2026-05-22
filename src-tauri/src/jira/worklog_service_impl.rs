//! `WorklogService` implementation for [`JiraClient`].
//!
//! Phase B4 — wires the Jira-specific sync entry points
//! (`sync_issues_from_jira` + `worklog_sync::sync_worklogs_for_range`)
//! into the provider-agnostic [`crate::worklog_service::WorklogService`]
//! trait so the orchestrator in `commands::worklog::sync` can dispatch
//! through `&dyn WorklogService` without matching on `ProviderClient`.

use chrono::NaiveDate;

use super::JiraClient;
use crate::cache::Db;
use crate::worklog_service::{ServiceFuture, SyncOutcome, WorklogService};

impl WorklogService for JiraClient {
    fn provider_name(&self) -> &'static str {
        "jira"
    }

    fn sync_issues<'a>(&'a self, db: &'a Db, conn_id: i64) -> ServiceFuture<'a, usize> {
        Box::pin(async move {
            let n = crate::jira::sync_issues_from_jira(self, db, conn_id).await?;
            Ok(n)
        })
    }

    fn sync_worklogs<'a>(
        &'a self,
        db: &'a Db,
        conn_id: i64,
        from: NaiveDate,
        to: NaiveDate,
    ) -> ServiceFuture<'a, SyncOutcome> {
        Box::pin(async move {
            // Readiness gate: the worklog sync needs the current user's
            // account id to filter JQL on `worklogAuthor`. If `myself()`
            // fails we must surface that to the orchestrator as Skipped
            // (the provider was never called) — short-circuiting to Ok(0)
            // would let the orchestrator clear the persisted error and
            // report a healthy run that secretly did nothing.
            if let Err(e) = self.myself().await {
                return Ok(SyncOutcome::skipped(format!(
                    "jira: nepodařilo se získat účet (myself: {e})"
                )));
            }
            let n = crate::jira::worklog_sync::sync_worklogs_for_range(self, db, conn_id, from, to)
                .await?;
            Ok(SyncOutcome::ok(n))
        })
    }
}
