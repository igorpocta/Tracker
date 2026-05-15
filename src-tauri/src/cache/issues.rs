use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IssueRow {
    pub issue_key: String,
    pub issue_id: Option<String>,
    pub summary: String,
    pub status_category: Option<String>,
    pub priority_order: Option<i64>,
    pub assignee_email: Option<String>,
    pub assignee_account_id: Option<String>,
    pub parent_key: Option<String>,
    pub parent_summary: Option<String>,
    pub issue_type: Option<String>,
    pub time_spent: Option<i64>,
    pub aggregate_time_spent: Option<i64>,
    pub time_original_estimate: Option<i64>,
    pub time_estimate: Option<i64>,
    pub epic_key: Option<String>,
    pub epic_summary: Option<String>,
    pub updated_at: i64,
}

pub fn upsert(db: &Db, issue: &IssueRow) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO issues (
            issue_key, issue_id, summary, status_category, priority_order,
            assignee_email, assignee_account_id, parent_key, parent_summary,
            issue_type, time_spent, aggregate_time_spent, time_original_estimate,
            time_estimate, epic_key, epic_summary, updated_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
        ON CONFLICT(issue_key) DO UPDATE SET
            issue_id=excluded.issue_id, summary=excluded.summary,
            status_category=excluded.status_category, priority_order=excluded.priority_order,
            assignee_email=excluded.assignee_email, assignee_account_id=excluded.assignee_account_id,
            parent_key=excluded.parent_key, parent_summary=excluded.parent_summary,
            issue_type=excluded.issue_type, time_spent=excluded.time_spent,
            aggregate_time_spent=excluded.aggregate_time_spent,
            time_original_estimate=excluded.time_original_estimate,
            time_estimate=excluded.time_estimate, epic_key=excluded.epic_key,
            epic_summary=excluded.epic_summary, updated_at=excluded.updated_at",
        rusqlite::params![
            issue.issue_key,
            issue.issue_id,
            issue.summary,
            issue.status_category,
            issue.priority_order,
            issue.assignee_email,
            issue.assignee_account_id,
            issue.parent_key,
            issue.parent_summary,
            issue.issue_type,
            issue.time_spent,
            issue.aggregate_time_spent,
            issue.time_original_estimate,
            issue.time_estimate,
            issue.epic_key,
            issue.epic_summary,
            issue.updated_at,
        ],
    )?;
    Ok(())
}

pub fn get_by_key(db: &Db, key: &str) -> Result<Option<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let row = conn.query_row(
        "SELECT issue_key, issue_id, summary, status_category, priority_order,
                assignee_email, assignee_account_id, parent_key, parent_summary,
                issue_type, time_spent, aggregate_time_spent, time_original_estimate,
                time_estimate, epic_key, epic_summary, updated_at
         FROM issues WHERE issue_key = ?1",
        [key],
        row_to_issue,
    );
    match row {
        Ok(i) => Ok(Some(i)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn recent(db: &Db, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT issue_key, issue_id, summary, status_category, priority_order,
                assignee_email, assignee_account_id, parent_key, parent_summary,
                issue_type, time_spent, aggregate_time_spent, time_original_estimate,
                time_estimate, epic_key, epic_summary, updated_at
         FROM issues
         ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Return issues that have at least one entry in `recent_worklogs`, ordered by
/// the most recent worklog timestamp. Useful as a "suggested" / "frequently
/// tracked" picker on the main window.
pub fn suggested(db: &Db, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT i.issue_key, i.issue_id, i.summary, i.status_category, i.priority_order,
                i.assignee_email, i.assignee_account_id, i.parent_key, i.parent_summary,
                i.issue_type, i.time_spent, i.aggregate_time_spent, i.time_original_estimate,
                i.time_estimate, i.epic_key, i.epic_summary, i.updated_at
         FROM issues i
         INNER JOIN (
            SELECT issue_key, MAX(logged_at) AS last_logged
            FROM recent_worklogs
            GROUP BY issue_key
         ) w ON w.issue_key = i.issue_key
         ORDER BY w.last_logged DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Total number of issues currently cached.
pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))?;
    Ok(n)
}

/// Look up the `connection_id` column for a given issue key. Returns `None`
/// if the issue is unknown or has no associated connection (legacy rows).
pub fn get_connection_id_by_key(db: &Db, key: &str) -> Result<Option<i64>, DbError> {
    let conn = db.pool().get()?;
    let r = conn.query_row(
        "SELECT connection_id FROM issues WHERE issue_key = ?1",
        [key],
        |r| r.get::<_, Option<i64>>(0),
    );
    match r {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn search(db: &Db, query: &str, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let q = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT issue_key, issue_id, summary, status_category, priority_order,
                assignee_email, assignee_account_id, parent_key, parent_summary,
                issue_type, time_spent, aggregate_time_spent, time_original_estimate,
                time_estimate, epic_key, epic_summary, updated_at
         FROM issues
         WHERE lower(issue_key) LIKE ?1 OR lower(summary) LIKE ?1
         ORDER BY updated_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![q, limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn row_to_issue(r: &rusqlite::Row<'_>) -> rusqlite::Result<IssueRow> {
    Ok(IssueRow {
        issue_key: r.get(0)?,
        issue_id: r.get(1)?,
        summary: r.get(2)?,
        status_category: r.get(3)?,
        priority_order: r.get(4)?,
        assignee_email: r.get(5)?,
        assignee_account_id: r.get(6)?,
        parent_key: r.get(7)?,
        parent_summary: r.get(8)?,
        issue_type: r.get(9)?,
        time_spent: r.get(10)?,
        aggregate_time_spent: r.get(11)?,
        time_original_estimate: r.get(12)?,
        time_estimate: r.get(13)?,
        epic_key: r.get(14)?,
        epic_summary: r.get(15)?,
        updated_at: r.get(16)?,
    })
}
