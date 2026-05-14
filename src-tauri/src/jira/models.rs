//! Strongly-typed structs for the subset of Jira Cloud v3 responses we consume.

use serde::{Deserialize, Serialize};

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
