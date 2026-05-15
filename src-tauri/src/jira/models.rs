//! Strongly-typed structs for the subset of Jira Cloud v3 responses we consume.

use chrono::{DateTime, NaiveDateTime};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cache::issues::IssueRow;

/// `GET /rest/api/3/myself` response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraUser {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "emailAddress", default)]
    pub email_address: Option<String>,
}

/// A single issue returned by Jira (`/rest/api/3/search/jql` items, etc.).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraIssue {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub fields: JiraIssueFields,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraIssueFields {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub status: Option<JiraStatus>,
    #[serde(default)]
    pub priority: Option<JiraPriority>,
    #[serde(default)]
    pub assignee: Option<JiraAssignee>,
    #[serde(default)]
    pub reporter: Option<JiraAssignee>,
    #[serde(default)]
    pub parent: Option<JiraParent>,
    #[serde(default)]
    pub issuetype: Option<JiraIssueType>,
    #[serde(default)]
    pub timetracking: Option<JiraTimeTracking>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    /// ISO date YYYY-MM-DD nebo null.
    #[serde(default)]
    pub duedate: Option<String>,
    /// Epic Link — Jira's classic custom field.
    #[serde(default, rename = "customfield_10014")]
    pub customfield_10014: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraStatus {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "statusCategory")]
    pub status_category: Option<JiraStatusCategory>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraStatusCategory {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraPriority {
    #[serde(default)]
    pub name: Option<String>,
    /// Jira returns priority ordering as a stringified integer.
    #[serde(default, rename = "id")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraAssignee {
    #[serde(default, rename = "accountId")]
    pub account_id: Option<String>,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default, rename = "emailAddress")]
    pub email_address: Option<String>,
    /// Mapa avatar URL z Jiry (`"16x16"`, `"24x24"`, `"32x32"`, `"48x48"`).
    /// Pro UI rendering bere FE nejvyšší dostupnou velikost.
    #[serde(default, rename = "avatarUrls")]
    pub avatar_urls: Option<JiraAvatarUrls>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraAvatarUrls {
    #[serde(default, rename = "16x16")]
    pub size_16: Option<String>,
    #[serde(default, rename = "24x24")]
    pub size_24: Option<String>,
    #[serde(default, rename = "32x32")]
    pub size_32: Option<String>,
    #[serde(default, rename = "48x48")]
    pub size_48: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraParent {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub fields: Option<JiraParentFields>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraParentFields {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub issuetype: Option<JiraIssueType>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraIssueType {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraTimeTracking {
    #[serde(default, rename = "timeSpentSeconds")]
    pub time_spent_seconds: Option<i64>,
    #[serde(default, rename = "originalEstimateSeconds")]
    pub original_estimate_seconds: Option<i64>,
    #[serde(default, rename = "remainingEstimateSeconds")]
    pub remaining_estimate_seconds: Option<i64>,
}

/// `POST /rest/api/3/search/jql` response (the new pagination model).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchPage {
    #[serde(default)]
    pub issues: Vec<JiraIssue>,
    #[serde(default, rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(default, rename = "isLast")]
    pub is_last: bool,
}

/// Outgoing body for `POST /rest/api/3/issue/{key}/worklog`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorklogRequest {
    pub started: String,
    #[serde(rename = "timeSpentSeconds")]
    pub time_spent_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<Value>,
}

/// Response from Jira after creating a worklog.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorklogResponse {
    pub id: String,
    #[serde(default, rename = "issueId")]
    pub issue_id: Option<String>,
    #[serde(default, rename = "timeSpentSeconds")]
    pub time_spent_seconds: Option<i64>,
    #[serde(default)]
    pub started: Option<String>,
}

/// Author block on a `JiraWorklog`. We tolerate missing `displayName` /
/// `emailAddress` because Jira can elide them for restricted accounts.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraWorklogAuthor {
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default, rename = "emailAddress")]
    pub email_address: Option<String>,
}

/// A worklog entry as returned by `/rest/api/3/worklog/list` or
/// `/rest/api/3/issue/{key}/worklog`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JiraWorklog {
    pub id: String,
    /// Numeric issue id; populated by `/worklog/list`, **not** by
    /// `/issue/{key}/worklog`.
    #[serde(default, rename = "issueId")]
    pub issue_id: Option<String>,
    pub author: JiraWorklogAuthor,
    /// ISO 8601 with milliseconds and offset, e.g. `2026-05-14T09:30:00.000+0000`.
    pub started: String,
    #[serde(rename = "timeSpentSeconds")]
    pub time_spent_seconds: i64,
    /// ADF document; use [`crate::jira::adf::extract_adf_text`] to flatten.
    #[serde(default)]
    pub comment: Option<Value>,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub created: String,
}

/// One entry in the `values` array of `GET /rest/api/3/worklog/updated`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorklogUpdatedEntry {
    #[serde(rename = "worklogId")]
    pub worklog_id: i64,
    #[serde(default, rename = "updatedTime")]
    pub updated_time: Option<i64>,
}

/// `GET /rest/api/3/worklog/updated` response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorklogUpdatedPage {
    #[serde(default)]
    pub values: Vec<WorklogUpdatedEntry>,
    #[serde(default, rename = "lastPage")]
    pub last_page: bool,
    #[serde(default, rename = "nextPage")]
    pub next_page: Option<String>,
    #[serde(default)]
    pub since: i64,
    #[serde(default)]
    pub until: i64,
}

/// `GET /rest/api/3/issue/{key}/worklog` response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IssueWorklogsPage {
    #[serde(default)]
    pub worklogs: Vec<JiraWorklog>,
    #[serde(default)]
    pub total: u32,
    #[serde(default, rename = "startAt")]
    pub start_at: u32,
    #[serde(default, rename = "maxResults")]
    pub max_results: u32,
}

/// Parse a Jira `started` / `updated` / `created` timestamp into a Unix
/// second. Re-exported here as a public helper because `parse_jira_timestamp`
/// is otherwise module-private.
pub fn parse_jira_timestamp_public(s: &str) -> Option<i64> {
    parse_jira_timestamp(s)
}

/// Parse Jira's timestamp formats into a Unix second.
///
/// Jira emits times like `"2026-05-14T09:30:00.000+0000"` (no colon in the
/// offset), which is not strictly RFC3339. We try the Jira-native format first
/// and fall back to RFC3339 / naive variants.
fn parse_jira_timestamp(s: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3f%z") {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z") {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3f") {
        return Some(ndt.and_utc().timestamp());
    }
    None
}

/// Convert a Jira issue payload into the row shape expected by the local cache.
///
/// Missing optional fields map to `None`; an unparseable `updated` falls back
/// to `0` so the row is still insertable.
/// Map a Jira issue into the multi-provider `issues_v2` row.
///
/// `connection_id` ties the row to the integration that fetched it; `now` is
/// the wall-clock timestamp the row should record as `updated_at` (the
/// caller passes one value for the whole batch to keep diagnostics tidy).
pub fn map_issue_to_row(issue: &JiraIssue, connection_id: i64, now: i64) -> IssueRow {
    let fields = &issue.fields;

    let name = fields.summary.clone().unwrap_or_default();

    // Status: prefer the human-readable name, fall back to category key.
    let status = fields.status.as_ref().and_then(|s| {
        s.name
            .clone()
            .or_else(|| s.status_category.as_ref().and_then(|c| c.key.clone()))
    });

    // Treat Jira's "done" status category as archived; the picker hides it
    // by default. Other categories (`new`, `indeterminate`) stay visible.
    let is_archived = fields
        .status
        .as_ref()
        .and_then(|s| s.status_category.as_ref())
        .and_then(|c| c.key.as_deref())
        .map(|k| k.eq_ignore_ascii_case("done"))
        .unwrap_or(false);

    let parent_key = fields.parent.as_ref().and_then(|p| p.key.clone());
    let parent_name = fields
        .parent
        .as_ref()
        .and_then(|p| p.fields.as_ref())
        .and_then(|f| f.summary.clone());

    let remote_updated_at = fields.updated.as_deref().and_then(parse_jira_timestamp);

    IssueRow {
        id: None,
        connection_id,
        issue_id: issue.id.clone(),
        issue_key: issue.key.clone(),
        name,
        parent_key,
        parent_name,
        status,
        is_archived,
        created_at: now,
        updated_at: now,
        remote_updated_at,
        last_synced_at: Some(now),
    }
}
