use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub issue_key: String,
    pub started_at: i64,
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

pub fn get(db: &Db) -> Result<Option<ActiveTimer>, DbError> {
    let conn = db.pool().get()?;
    match conn.query_row(
        "SELECT issue_key, started_at FROM active_timer WHERE id = 1",
        [],
        |r| {
            Ok(ActiveTimer {
                issue_key: r.get(0)?,
                started_at: r.get(1)?,
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
