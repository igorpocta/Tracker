//! Jira Cloud REST API v3 client.
//!
//! Provides a typed wrapper around the endpoints used by Tracker:
//! - `GET  /rest/api/3/myself`
//! - `POST /rest/api/3/search/jql`
//! - `POST /rest/api/3/issue/{key}/worklog`
//!
//! The client uses `reqwest` with `rustls-tls` and Basic auth (email + API token).

pub mod adf;
pub mod client;
pub mod jql;
pub mod models;
pub mod reconstruct;
pub mod worklog_ops;
pub mod worklog_service_impl;
pub mod worklog_sync;

pub use adf::{extract_adf_text, make_adf_comment};
pub use client::{JiraClient, JiraError};
pub use jql::{DEFAULT_JQL, SYNC_MAX_RESULTS_TOTAL, SYNC_PAGE_MAX_RESULTS};
pub use models::{
    map_issue_to_row, IssueWorklogsPage, JiraIssue, JiraIssueFields, JiraUser, JiraWorklog,
    JiraWorklogAuthor, SearchPage, WorklogRequest, WorklogResponse, WorklogUpdatedEntry,
    WorklogUpdatedPage,
};

use crate::cache::{self, Db};
use thiserror::Error;

/// Errors produced by the full-sync convenience entry point.
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("jira: {0}")]
    Jira(#[from] JiraError),
    #[error("db: {0}")]
    Db(#[from] cache::DbError),
}

/// Production-relevant Jira fields the sync requests.
pub const SYNC_FIELDS: &[&str] = &[
    "summary",
    "status",
    "priority",
    "assignee",
    "parent",
    "issuetype",
    "timetracking",
    "customfield_10014",
    "updated",
];

/// Fetch all issues matching [`DEFAULT_JQL`] and upsert them into the local SQLite cache.
///
/// Returns the number of issues processed across all pages. Capped at
/// [`SYNC_MAX_RESULTS_TOTAL`] so that an accidental "give me everything"
/// query on a huge instance can't run away with the disk.
pub async fn sync_issues_from_jira(
    client: &JiraClient,
    db: &Db,
    connection_id: i64,
) -> Result<usize, SyncError> {
    let mut total = 0usize;
    let mut page_token: Option<String> = None;
    let now = chrono::Utc::now().timestamp();
    loop {
        let page = client
            .search_jql(
                DEFAULT_JQL,
                page_token.as_deref(),
                SYNC_FIELDS,
                SYNC_PAGE_MAX_RESULTS,
            )
            .await?;
        for issue in &page.issues {
            cache::issues::upsert(db, &map_issue_to_row(issue, connection_id, now))?;
        }
        total += page.issues.len();
        if total >= SYNC_MAX_RESULTS_TOTAL {
            break;
        }
        if page.is_last || page.next_page_token.is_none() {
            break;
        }
        page_token = page.next_page_token;
    }
    Ok(total)
}
