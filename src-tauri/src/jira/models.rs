//! Strongly-typed structs for the subset of Jira Cloud v3 responses we consume.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub parent: Option<JiraParent>,
    #[serde(default)]
    pub issuetype: Option<JiraIssueType>,
    #[serde(default)]
    pub timetracking: Option<JiraTimeTracking>,
    #[serde(default)]
    pub updated: Option<String>,
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
