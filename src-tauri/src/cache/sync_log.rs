//! `sync_runs` cache layer — drží historii dokončených syncov pro UI
//! "Historie synchronizací". Read-only ze strany UI, jediný writer je
//! `sync_one_connection` v `commands::worklog`.

use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncRunRow {
    pub id: Option<i64>,
    pub connection_id: Option<i64>,
    pub connection_name: Option<String>,
    pub provider: Option<String>,
    pub mode: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub issues_count: i64,
    pub worklogs_count: i64,
    pub error_phase: Option<String>,
    pub error_message: Option<String>,
}

pub fn record(db: &Db, row: &SyncRunRow) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO sync_runs (
            connection_id, connection_name, provider, mode,
            started_at, finished_at, issues_count, worklogs_count,
            error_phase, error_message
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params![
            row.connection_id,
            row.connection_name,
            row.provider,
            row.mode,
            row.started_at,
            row.finished_at,
            row.issues_count,
            row.worklogs_count,
            row.error_phase,
            row.error_message,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_recent(db: &Db, limit: u32) -> Result<Vec<SyncRunRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, connection_id, connection_name, provider, mode,
                started_at, finished_at, issues_count, worklogs_count,
                error_phase, error_message
         FROM sync_runs
         ORDER BY finished_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |r| {
        Ok(SyncRunRow {
            id: r.get(0)?,
            connection_id: r.get(1)?,
            connection_name: r.get(2)?,
            provider: r.get(3)?,
            mode: r.get(4)?,
            started_at: r.get(5)?,
            finished_at: r.get(6)?,
            issues_count: r.get(7)?,
            worklogs_count: r.get(8)?,
            error_phase: r.get(9)?,
            error_message: r.get(10)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Smazat staré záznamy — výchozí policy 90 dnů, ať historie nepřebobtná.
/// Volá ho startup recovery v `lib.rs` (jednou za boot).
pub fn purge_older_than(db: &Db, older_than_unix_s: i64) -> Result<usize, DbError> {
    let conn = db.pool().get()?;
    let n = conn.execute(
        "DELETE FROM sync_runs WHERE finished_at < ?1",
        [older_than_unix_s],
    )?;
    Ok(n)
}
