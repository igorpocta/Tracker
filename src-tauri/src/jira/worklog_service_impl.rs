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
use crate::worklog_service::{ServiceFuture, WorklogService};

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
    ) -> ServiceFuture<'a, usize> {
        Box::pin(async move {
            // Readiness gate: the worklog sync needs the current user's
            // account id to filter JQL on `worklogAuthor`. The orchestrator
            // historically pre-checked `myself()` and silently skipped the
            // worklog phase when it failed — preserve that by short-circuiting
            // to Ok(0) here instead of bubbling up an error.
            if self.myself().await.is_err() {
                return Ok(0);
            }
            let n = crate::jira::worklog_sync::sync_worklogs_for_range(self, db, conn_id, from, to)
                .await?;
            Ok(n)
        })
    }
}
