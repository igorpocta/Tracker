//! DTOs returned by the Freelo REST API.
//!
//! Field names use `#[serde(rename_all = "snake_case")]` (or per-field
//! `#[serde(rename)]`) to match the Freelo API. Where the API's exact field
//! names are unverified at implementation time we use lenient deserialisation
//! (Option-wrapping, defaulting) so a misnamed field shows up as `None` rather
//! than a hard failure.
//!
//! See `https://freelo.docs.apiary.io/` for the canonical schema.

use serde::{Deserialize, Serialize};

/// The authenticated Freelo user (returned by `GET /users/manage-workers`
/// or `GET /me` — implementation tries both).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloUser {
    pub id: i64,
    #[serde(default)]
    pub email: Option<String>,
    /// Display name. Freelo returns it as `first_name` + `last_name`; we
    /// concatenate them at parse time into `display_name`.
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default, rename = "first_name")]
    pub first_name: Option<String>,
    #[serde(default, rename = "last_name")]
    pub last_name: Option<String>,
}

impl FreeloUser {
    /// Render the best human-friendly name we can: prefer `display_name` if
    /// set, otherwise concatenate first/last, otherwise fall back to email.
    pub fn best_name(&self) -> String {
        if let Some(n) = &self.display_name {
            if !n.trim().is_empty() {
                return n.clone();
            }
        }
        let combined = format!(
            "{} {}",
            self.first_name.as_deref().unwrap_or(""),
            self.last_name.as_deref().unwrap_or("")
        )
        .trim()
        .to_string();
        if !combined.is_empty() {
            return combined;
        }
        self.email.clone().unwrap_or_else(|| format!("#{}", self.id))
    }
}

/// One project that the authenticated user has access to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloProject {
    pub id: i64,
    pub name: String,
    /// `active`, `finished`, `archived`, etc. Defaults to `active` when the
    /// API doesn't include the field.
    #[serde(default = "default_state")]
    pub state: String,
}

/// One task in a Freelo project. The tasks endpoint may also return tasks
/// nested under tasklists — we flatten the response in [`crate::freelo::client::FreeloClient::list_tasks_for_project`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloTask {
    pub id: i64,
    pub name: String,
    /// Numeric project id. Freelo sometimes returns this as `project_id` and
    /// sometimes as a nested `project.id`; the client flattens both shapes.
    #[serde(default)]
    pub project_id: Option<i64>,
    /// Optional tasklist id (Freelo's "list" within a project).
    #[serde(default)]
    pub tasklist_id: Option<i64>,
    /// `active`, `finished`, etc. Defaults to `active`.
    #[serde(default = "default_state")]
    pub state: String,
}

/// One work-report (time entry) in Freelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloWorkReport {
    pub id: i64,
    /// Numeric task id this entry belongs to. Freelo nests this under
    /// `task.id` in some response shapes; the client flattens.
    pub task_id: i64,
    /// Time worked, in minutes. Freelo stores minutes natively (Tracker
    /// converts to seconds for the shared `recent_worklogs.duration_s`).
    pub minutes: i64,
    /// Date the work was reported, in `YYYY-MM-DD` form.
    pub date_reported: String,
    /// Optional free-text note.
    #[serde(default)]
    pub description: Option<String>,
    /// Numeric Freelo user id of the author.
    pub user_id: i64,
}

fn default_state() -> String {
    "active".into()
}

/// Map a [`FreeloProject`] into the synthetic `issues` row used as the
/// "parent" / epic for all tasks in that project. We treat the project itself
/// as a queryable "issue" so it shows up in the cache and can be looked up
/// via [`crate::cache::issues::get_by_key`].
pub fn project_to_issue_row(p: &FreeloProject, now_unix_s: i64) -> crate::cache::issues::IssueRow {
    let key = super::project_key(p.id);
    crate::cache::issues::IssueRow {
        issue_key: key,
        issue_id: Some(p.id.to_string()),
        summary: p.name.clone(),
        status_category: Some(p.state.clone()),
        priority_order: None,
        assignee_email: None,
        assignee_account_id: None,
        parent_key: None,
        parent_summary: None,
        issue_type: Some("project".into()),
        time_spent: None,
        aggregate_time_spent: None,
        time_original_estimate: None,
        time_estimate: None,
        epic_key: None,
        epic_summary: None,
        updated_at: now_unix_s,
    }
}

/// Map a [`FreeloTask`] into a row in the shared `issues` cache, using the
/// supplied project for the `parent_*` and `epic_*` columns.
pub fn task_to_issue_row(
    t: &FreeloTask,
    project: &FreeloProject,
    now_unix_s: i64,
) -> crate::cache::issues::IssueRow {
    let key = super::task_key(t.id);
    let parent_key = super::project_key(project.id);
    crate::cache::issues::IssueRow {
        issue_key: key,
        issue_id: Some(t.id.to_string()),
        summary: t.name.clone(),
        status_category: Some(t.state.clone()),
        priority_order: None,
        assignee_email: None,
        assignee_account_id: None,
        parent_key: Some(parent_key.clone()),
        parent_summary: Some(project.name.clone()),
        issue_type: Some("task".into()),
        time_spent: None,
        aggregate_time_spent: None,
        time_original_estimate: None,
        time_estimate: None,
        // Freelo doesn't have epics, but we surface the project as the "epic"
        // so the UI's grouping logic (which keys off epic_key) works as-is.
        epic_key: Some(parent_key),
        epic_summary: Some(project.name.clone()),
        updated_at: now_unix_s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn freelo_user_deserialises_with_first_last() {
        let j = json!({
            "id": 7,
            "email": "x@y.cz",
            "first_name": "Igor",
            "last_name": "Pocta"
        });
        let u: FreeloUser = serde_json::from_value(j).unwrap();
        assert_eq!(u.id, 7);
        assert_eq!(u.best_name(), "Igor Pocta");
    }

    #[test]
    fn freelo_project_default_state_is_active() {
        let j = json!({ "id": 5, "name": "Marketing" });
        let p: FreeloProject = serde_json::from_value(j).unwrap();
        assert_eq!(p.state, "active");
    }

    #[test]
    fn task_to_row_uses_synthetic_keys() {
        let p = FreeloProject {
            id: 10,
            name: "Web".into(),
            state: "active".into(),
        };
        let t = FreeloTask {
            id: 42,
            name: "Landing page".into(),
            project_id: Some(10),
            tasklist_id: None,
            state: "active".into(),
        };
        let r = task_to_issue_row(&t, &p, 1_700_000_000);
        assert_eq!(r.issue_key, "FRL-42");
        assert_eq!(r.parent_key.as_deref(), Some("FRL-P-10"));
        assert_eq!(r.parent_summary.as_deref(), Some("Web"));
        assert_eq!(r.epic_key.as_deref(), Some("FRL-P-10"));
    }
}
