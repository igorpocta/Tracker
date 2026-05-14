use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WorklogRow {
    pub id: Option<i64>,
    pub issue_key: String,
    pub issue_id: Option<String>,
    pub summary: Option<String>,
    pub duration_s: i64,
    pub started_at: i64,
    pub logged_at: i64,
    pub comment: Option<String>,
    pub jira_worklog_id: Option<String>,
}

pub fn record(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO recent_worklogs (issue_key, issue_id, summary, duration_s, started_at, logged_at, comment, jira_worklog_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            w.issue_key,
            w.issue_id,
            w.summary,
            w.duration_s,
            w.started_at,
            w.logged_at,
            w.comment,
            w.jira_worklog_id
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn recent(db: &Db, limit: u32) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at, comment, jira_worklog_id
         FROM recent_worklogs ORDER BY logged_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(WorklogRow {
            id: r.get(0)?,
            issue_key: r.get(1)?,
            issue_id: r.get(2)?,
            summary: r.get(3)?,
            duration_s: r.get(4)?,
            started_at: r.get(5)?,
            logged_at: r.get(6)?,
            comment: r.get(7)?,
            jira_worklog_id: r.get(8)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
