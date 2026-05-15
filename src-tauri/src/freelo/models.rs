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
        self.email
            .clone()
            .unwrap_or_else(|| format!("#{}", self.id))
    }
}

/// One project that the authenticated user has access to.
///
/// Freelo's `state` field has been seen in two shapes:
///   - String (older / legacy): `"state": "active"`
///   - Object (current v1):     `"state": { "id": 1, "state": "active" }`
///
/// `FreeloProjectState` accepts either via untagged deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloProject {
    pub id: i64,
    pub name: String,
    #[serde(
        default = "default_state_object",
        deserialize_with = "deserialize_state"
    )]
    pub state: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StateField {
    /// Old API shape — bare string.
    Plain(String),
    /// Current v1 shape — `{ id, state }`. We pick `state`.
    Wrapped {
        #[allow(dead_code)]
        id: Option<i64>,
        state: String,
    },
}

fn deserialize_state<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    Option::<StateField>::deserialize(d).map(|opt| match opt {
        Some(StateField::Plain(s)) => s,
        Some(StateField::Wrapped { state, .. }) => state,
        None => default_state_object(),
    })
}

fn default_state_object() -> String {
    "active".to_string()
}

/// One task in a Freelo project. The tasks endpoint may also return tasks
/// nested under tasklists — we flatten the response in the client.
///
/// `state` is the same `{id, state}`-object-or-string union as on
/// [`FreeloProject`] — the same custom deserializer handles both shapes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeloTask {
    pub id: i64,
    pub name: String,
    /// Numeric project id. Freelo sometimes returns this as `project_id` and
    /// sometimes as a nested `project.id`; the client flattens both shapes.
    #[serde(default)]
    pub project_id: Option<i64>,
    /// Cached display name of the parent project. Filled in by the client
    /// when the `/all-tasks` response includes `project.name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Optional tasklist id (Freelo's "list" within a project).
    #[serde(default)]
    pub tasklist_id: Option<i64>,
    #[serde(
        default = "default_state_object",
        deserialize_with = "deserialize_state"
    )]
    pub state: String,
    /// ISO 8601 timestamp from Freelo's `date_edited_at` field. Used to set
    /// `IssueRow.updated_at` so search results sort by recency.
    #[serde(default)]
    pub date_edited_at: Option<String>,
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

/// Map a [`FreeloTask`] into a row in the shared `issues` cache.
///
/// Project metadata (id + name) is pulled from the task's own `project_id` /
/// `project_name` fields — these are populated by the client when the
/// `/all-tasks` response carries a nested `project: {...}` object. If both
/// are absent the parent columns are left `None`.
pub fn task_to_issue_row(t: &FreeloTask, now_unix_s: i64) -> crate::cache::issues::IssueRow {
    let key = super::task_key(t.id);
    let parent_key = t.project_id.map(super::project_key);
    // Prefer the task's own `date_edited_at` so search results can sort by
    // recency-of-edit (not recency-of-sync). Falls back to `now_unix_s` if
    // Freelo didn't include the timestamp or it failed to parse.
    let updated_at = t
        .date_edited_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc).timestamp())
        .unwrap_or(now_unix_s);
    crate::cache::issues::IssueRow {
        issue_key: key,
        issue_id: Some(t.id.to_string()),
        summary: t.name.clone(),
        status_category: Some(t.state.clone()),
        priority_order: None,
        assignee_email: None,
        assignee_account_id: None,
        parent_key: parent_key.clone(),
        parent_summary: t.project_name.clone(),
        issue_type: Some("task".into()),
        time_spent: None,
        aggregate_time_spent: None,
        time_original_estimate: None,
        time_estimate: None,
        // Freelo doesn't have epics; project doubles as the "epic" so the
        // UI's grouping logic (which keys off epic_key) works as-is.
        epic_key: parent_key,
        epic_summary: t.project_name.clone(),
        updated_at,
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
        let t = FreeloTask {
            id: 42,
            name: "Landing page".into(),
            project_id: Some(10),
            project_name: Some("Web".into()),
            tasklist_id: None,
            state: "active".into(),
            date_edited_at: None,
        };
        let r = task_to_issue_row(&t, 1_700_000_000);
        assert_eq!(r.issue_key, "FREELO-42");
        assert_eq!(r.parent_key.as_deref(), Some("FREELO-P-10"));
        assert_eq!(r.parent_summary.as_deref(), Some("Web"));
        assert_eq!(r.epic_key.as_deref(), Some("FREELO-P-10"));
    }

    #[test]
    fn freelo_task_state_handles_object_shape() {
        // Live v1 shape — state is an object, not a string.
        let j = json!({
            "id": 42, "name": "Hi", "project_id": 10,
            "state": { "id": 1, "state": "active" }
        });
        let t: FreeloTask = serde_json::from_value(j).unwrap();
        assert_eq!(t.state, "active");
    }
}
