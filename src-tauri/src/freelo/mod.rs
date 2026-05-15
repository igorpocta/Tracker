//! Freelo (https://freelo.io) API v1 client + sync logic.
//!
//! Phase 18E: Tracker's second provider. Mirrors the structure of the Jira
//! module:
//!   - `client.rs`  : typed REST client with HTTP Basic auth (email + API key)
//!   - `models.rs`  : DTOs (project, task, work_report, user)
//!   - `sync.rs`    : issues + worklogs sync into the shared cache
//!   - `ops.rs`     : create/update/delete work_reports
//!
//! Freelo's API uses HTTP Basic auth where the *username* is the user's email
//! and the *password* is the user's API key. The base URL is
//! `https://api.freelo.io/v1` for the v1 API.
//!
//! Synthetic keys: Freelo tasks live in the same `issues` table as Jira
//! issues, so we tag them with a `FRL-` prefix to disambiguate the keyspace:
//!   - task        → `FRL-{task_id}`
//!   - project     → `FRL-P-{project_id}` (used as `parent_key`/`epic_key`)
//!
//! Work-reports are stored in `recent_worklogs` with a `freelo:{id}` prefix
//! in the `jira_worklog_id` column (we reuse the column name; the prefix
//! disambiguates it from numeric Jira ids).

pub mod client;
pub mod models;
pub mod ops;
pub mod reconstruct;
pub mod sync;
pub mod worklog_service_impl;

pub use client::{FreeloClient, FreeloError};
pub use models::{FreeloProject, FreeloTask, FreeloUser, FreeloWorkReport};
pub use worklog_service_impl::FreeloService;

/// Synthetic prefix for Freelo task keys in the shared `issues` table.
/// User-visible — must match what the UI pill renders (`FREELO-12345`).
pub const FREELO_TASK_PREFIX: &str = "FREELO-";
/// Synthetic prefix for Freelo project keys (parent / epic).
pub const FREELO_PROJECT_PREFIX: &str = "FREELO-P-";
/// Prefix used inside the `jira_worklog_id` column for Freelo work-report ids.
pub const FREELO_WORKLOG_PREFIX: &str = "freelo:";

/// Default Freelo API base URL (v1). Overridable via the connection config.
pub const DEFAULT_BASE_URL: &str = "https://api.freelo.io/v1";

/// Build a synthetic `issues` row issue_key for a Freelo task id.
pub fn task_key(task_id: i64) -> String {
    format!("{FREELO_TASK_PREFIX}{task_id}")
}

/// Build a synthetic `issues` row issue_key for a Freelo project id.
pub fn project_key(project_id: i64) -> String {
    format!("{FREELO_PROJECT_PREFIX}{project_id}")
}

/// Build the `jira_worklog_id` prefix value for a Freelo work-report.
pub fn worklog_id_key(work_report_id: i64) -> String {
    format!("{FREELO_WORKLOG_PREFIX}{work_report_id}")
}

/// Parse a synthetic task key back into its numeric task id. Returns `None` if
/// the key doesn't have the `FRL-` prefix or the suffix is not a number.
pub fn parse_task_key(key: &str) -> Option<i64> {
    let rest = key.strip_prefix(FREELO_TASK_PREFIX)?;
    // Reject project keys (FRL-P-…) which have the same prefix.
    if rest.starts_with("P-") {
        return None;
    }
    rest.parse::<i64>().ok()
}

/// Parse a synthetic project key back into its numeric project id.
pub fn parse_project_key(key: &str) -> Option<i64> {
    let rest = key.strip_prefix(FREELO_PROJECT_PREFIX)?;
    rest.parse::<i64>().ok()
}

/// Parse a Freelo work-report id back out of the `jira_worklog_id` column.
pub fn parse_worklog_id(s: &str) -> Option<i64> {
    s.strip_prefix(FREELO_WORKLOG_PREFIX)?.parse::<i64>().ok()
}

/// Returns true if `issue_key` is a Freelo-shaped synthetic key (task or
/// project). Useful for dispatching commands by provider when the connection
/// id isn't directly available.
pub fn is_freelo_key(issue_key: &str) -> bool {
    issue_key.starts_with(FREELO_TASK_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_key_roundtrips() {
        assert_eq!(task_key(42), "FREELO-42");
        assert_eq!(parse_task_key("FREELO-42"), Some(42));
        assert_eq!(parse_task_key("FREELO-9999"), Some(9999));
    }

    #[test]
    fn project_key_roundtrips() {
        assert_eq!(project_key(7), "FREELO-P-7");
        assert_eq!(parse_project_key("FREELO-P-7"), Some(7));
    }

    #[test]
    fn project_key_doesnt_parse_as_task() {
        assert_eq!(parse_task_key("FREELO-P-7"), None);
    }

    #[test]
    fn jira_key_is_not_freelo() {
        assert!(!is_freelo_key("ACME-1"));
        assert!(is_freelo_key("FREELO-1"));
        assert!(is_freelo_key("FREELO-P-1"));
    }

    #[test]
    fn worklog_id_roundtrips() {
        assert_eq!(worklog_id_key(123), "freelo:123");
        assert_eq!(parse_worklog_id("freelo:123"), Some(123));
        assert_eq!(parse_worklog_id("12345"), None);
    }
}
