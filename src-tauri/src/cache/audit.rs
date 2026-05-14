//! Append-only audit log for worklog mutations (Phase 15).
//!
//! Every successful — and every failed — mutation produces a row here. The
//! `before_json` / `after_json` columns hold full `WorklogRow` snapshots so
//! the user (or a future support engineer) can reconstruct the timeline of
//! changes applied to their Jira worklog data.
//!
//! Volume is bounded by user clicks; we don't bother with retention pruning
//! yet — see the migration comment for rationale.

use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

use super::worklogs::WorklogRow;

/// Logical operation kinds we record. Stored as TEXT for human inspection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOp {
    Create,
    Update,
    Delete,
    Move,
    SyncTombstone,
    Undo,
}

impl AuditOp {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Move => "move",
            Self::SyncTombstone => "sync_tombstone",
            Self::Undo => "undo",
        }
    }
}

/// Borrowed input for [`record`]; tests use this directly. Serializes the
/// optional `before`/`after` snapshots via `serde_json::to_string` so callers
/// don't have to think about that.
#[derive(Debug, Clone)]
pub struct AuditEvent<'a> {
    pub occurred_at: i64,
    pub op: AuditOp,
    pub issue_key: Option<&'a str>,
    pub worklog_id: Option<&'a str>,
    pub before: Option<&'a WorklogRow>,
    pub after: Option<&'a WorklogRow>,
    pub success: bool,
    pub error: Option<&'a str>,
}

/// Wire shape returned by the `get_audit_log` Tauri command. The before/after
/// snapshots are kept as raw JSON strings to keep the schema flexible (and to
/// avoid breaking changes when `WorklogRow` evolves).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub occurred_at: i64,
    pub op: String,
    pub issue_key: Option<String>,
    pub worklog_id: Option<String>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub success: bool,
    pub error: Option<String>,
}

/// Persist a single audit event. Best-effort: serialization failures of the
/// before/after snapshots are stored as `null` rather than aborting the row.
pub fn record(db: &Db, ev: AuditEvent<'_>) -> Result<i64, DbError> {
    let before_json = ev.before.and_then(|r| serde_json::to_string(r).ok());
    let after_json = ev.after.and_then(|r| serde_json::to_string(r).ok());

    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO audit_log (
            occurred_at, op, issue_key, worklog_id, before_json, after_json,
            success, error
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params![
            ev.occurred_at,
            ev.op.as_str(),
            ev.issue_key,
            ev.worklog_id,
            before_json,
            after_json,
            if ev.success { 1 } else { 0 },
            ev.error,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Return the most recent `limit` audit entries (newest first). Used by the
/// `get_audit_log` Tauri command.
pub fn recent(db: &Db, limit: u32) -> Result<Vec<AuditEntry>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, occurred_at, op, issue_key, worklog_id, before_json, after_json,
                success, error
         FROM audit_log
         ORDER BY occurred_at DESC, id DESC
         LIMIT ?1",
    )?;
    let mapped = stmt.query_map([limit], |r| {
        let success_i: i64 = r.get(7)?;
        Ok(AuditEntry {
            id: r.get(0)?,
            occurred_at: r.get(1)?,
            op: r.get(2)?,
            issue_key: r.get(3)?,
            worklog_id: r.get(4)?,
            before_json: r.get(5)?,
            after_json: r.get(6)?,
            success: success_i != 0,
            error: r.get(8)?,
        })
    })?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Total count of rows in `audit_log`. Used by tests + cache stats.
pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))?;
    Ok(n)
}
