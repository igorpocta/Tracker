use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub issue_key: String,
    pub started_at: i64,
    /// Phase 18B — Item 6: optional in-flight comment. `None` (and `""`) both
    /// mean "no comment". Only persisted to Jira when the timer stops, unless
    /// the StopDialog provides its own override.
    pub comment: Option<String>,
}

pub fn start(db: &Db, issue_key: &str, started_at: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO active_timer (id, issue_key, started_at) VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET issue_key=excluded.issue_key, started_at=excluded.started_at",
        rusqlite::params![issue_key, started_at],
    )?;
    Ok(())
}

/// Phase 18B — Item 6: variant of `start` that also stores an initial comment.
pub fn start_with_comment(
    db: &Db,
    issue_key: &str,
    started_at: i64,
    comment: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO active_timer (id, issue_key, started_at, comment)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
            issue_key=excluded.issue_key,
            started_at=excluded.started_at,
            comment=excluded.comment",
        rusqlite::params![issue_key, started_at, comment],
    )?;
    Ok(())
}

/// Update only the comment on the running timer (does NOT bump `started_at`).
pub fn set_comment(db: &Db, comment: Option<&str>) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE active_timer SET comment = ?1 WHERE id = 1",
        rusqlite::params![comment],
    )?;
    Ok(())
}

pub fn get(db: &Db) -> Result<Option<ActiveTimer>, DbError> {
    let conn = db.pool().get()?;
    match conn.query_row(
        "SELECT issue_key, started_at, comment FROM active_timer WHERE id = 1",
        [],
        |r| {
            Ok(ActiveTimer {
                issue_key: r.get(0)?,
                started_at: r.get(1)?,
                comment: r.get(2)?,
            })
        },
    ) {
        Ok(t) => Ok(Some(t)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn stop(db: &Db) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM active_timer WHERE id = 1", [])?;
    Ok(())
}
