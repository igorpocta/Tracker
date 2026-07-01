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
    /// Tenant the user picked when starting (from a favorite / search row).
    /// `None` → resolve from `issue_key` at stop time (legacy / unassigned).
    /// Lets stop route to the correct connection even when two enabled tenants
    /// share the same issue key.
    #[serde(default)]
    pub connection_id: Option<i64>,
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
/// `connection_id` is the tenant the caller resolved (see [`ActiveTimer`]).
pub fn start_with_comment(
    db: &Db,
    issue_key: &str,
    started_at: i64,
    comment: Option<&str>,
    connection_id: Option<i64>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO active_timer (id, issue_key, started_at, comment, connection_id)
         VALUES (1, ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
            issue_key=excluded.issue_key,
            started_at=excluded.started_at,
            comment=excluded.comment,
            connection_id=excluded.connection_id",
        rusqlite::params![issue_key, started_at, comment, connection_id],
    )?;
    Ok(())
}

/// P1-1: start a timer **only if none is running**. Returns `Ok(true)` when a
/// new timer was started, `Ok(false)` when one was already active. Atomic —
/// relies on the primary key on `active_timer.id` (a plain INSERT without
/// `ON CONFLICT` fails with a constraint violation when a row already exists),
/// so concurrent start attempts from the popover, tray and HTTP API can never
/// silently overwrite a running timer.
pub fn try_start_with_comment(
    db: &Db,
    issue_key: &str,
    started_at: i64,
    comment: Option<&str>,
    connection_id: Option<i64>,
) -> Result<bool, DbError> {
    let conn = db.pool().get()?;
    let res = conn.execute(
        "INSERT INTO active_timer (id, issue_key, started_at, comment, connection_id)
         VALUES (1, ?1, ?2, ?3, ?4)",
        rusqlite::params![issue_key, started_at, comment, connection_id],
    );
    match res {
        Ok(_) => Ok(true),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
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
        "SELECT issue_key, started_at, comment, connection_id FROM active_timer WHERE id = 1",
        [],
        |r| {
            Ok(ActiveTimer {
                issue_key: r.get(0)?,
                started_at: r.get(1)?,
                comment: r.get(2)?,
                connection_id: r.get(3)?,
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
