//! Append-only audit log for worklog mutations (Phase 15 + 16).
//!
//! Every successful — and every failed — mutation produces a row here. The
//! `before_json` / `after_json` columns hold full `WorklogRow` snapshots so
//! the user (or a future support engineer) can reconstruct the timeline of
//! changes applied to their Jira worklog data.
//!
//! Phase 16 adds:
//! - Three "reconstruction" op kinds (`Restore`, `Revert`, `Retry`) wired up
//!   by the matching Tauri commands. They store the rowid of the audit entry
//!   they were spawned from in `source_audit_id` so the UI can detect "this
//!   delete has already been restored" and hide the action button.
//! - Paginated + filtered query support for the Historie změn UI.
//!
//! Volume is bounded by user clicks but Phase 16 adds an explicit `purge`
//! helper for users who want to trim the history themselves.

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
    /// Phase 16 — a deleted (or sync-tombstoned) worklog was recreated in Jira
    /// from its before-snapshot. The new audit row's `source_audit_id` points
    /// at the original delete entry.
    Restore,
    /// Phase 16 — an `update` was rolled back by pushing `before_json` back to
    /// Jira as a fresh update.
    Revert,
    /// Phase 16 — a previously-failed action was retried successfully (or
    /// failed again — `success` carries the outcome).
    Retry,
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
            Self::Restore => "restore",
            Self::Revert => "revert",
            Self::Retry => "retry",
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
    /// Phase 16 — rowid of the audit row that triggered this entry, when
    /// applicable (restore / revert / retry).
    pub source_audit_id: Option<i64>,
}

impl<'a> Default for AuditEvent<'a> {
    fn default() -> Self {
        Self {
            occurred_at: 0,
            op: AuditOp::Create,
            issue_key: None,
            worklog_id: None,
            before: None,
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        }
    }
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
    /// Phase 16 — set on entries spawned by a restore/revert/retry action.
    pub source_audit_id: Option<i64>,
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
            success, error, source_audit_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            ev.occurred_at,
            ev.op.as_str(),
            ev.issue_key,
            ev.worklog_id,
            before_json,
            after_json,
            if ev.success { 1 } else { 0 },
            ev.error,
            ev.source_audit_id,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Return the most recent `limit` audit entries (newest first). Used by the
/// `get_audit_log` Tauri command for the first page.
pub fn recent(db: &Db, limit: u32) -> Result<Vec<AuditEntry>, DbError> {
    list(db, limit, None, None, false)
}

/// Phase 16 — paginated + filtered audit log query.
///
/// - `limit`: max rows to return.
/// - `before_id`: if `Some(id)`, return only rows with `id < before_id`.
///   Use the last `id` of the previous page to paginate.
/// - `ops`: if `Some(&[...])`, restrict to those op kinds (matched as strings).
///   Empty slice ≡ "no filter".
/// - `only_failed`: if `true`, restrict to rows with `success = 0`.
///
/// Results are ordered by `occurred_at DESC, id DESC`. Using `id DESC` as a
/// secondary key keeps pagination stable when multiple events land in the same
/// second (which happens frequently in test fixtures).
pub fn list(
    db: &Db,
    limit: u32,
    before_id: Option<i64>,
    ops: Option<&[String]>,
    only_failed: bool,
) -> Result<Vec<AuditEntry>, DbError> {
    let conn = db.pool().get()?;

    // Build SQL dynamically to keep the filter combinations explicit.
    let mut sql = String::from(
        "SELECT id, occurred_at, op, issue_key, worklog_id, before_json, after_json,
                success, error, source_audit_id
         FROM audit_log
         WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(b) = before_id {
        sql.push_str(" AND id < ?");
        params.push(Box::new(b));
    }
    if only_failed {
        sql.push_str(" AND success = 0");
    }
    if let Some(op_list) = ops {
        if !op_list.is_empty() {
            // Build "AND op IN (?,?,?)" — placeholder count = ops.len().
            sql.push_str(" AND op IN (");
            for (i, op) in op_list.iter().enumerate() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('?');
                params.push(Box::new(op.clone()));
            }
            sql.push(')');
        }
    }

    sql.push_str(" ORDER BY occurred_at DESC, id DESC LIMIT ?");
    params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let mapped = stmt.query_map(rusqlite::params_from_iter(param_refs), |r| {
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
            source_audit_id: r.get(9)?,
        })
    })?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Phase 16 — fetch a single audit entry by id. Returns `None` if no such
/// row exists (e.g. it was purged).
pub fn get_by_id(db: &Db, id: i64) -> Result<Option<AuditEntry>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, occurred_at, op, issue_key, worklog_id, before_json, after_json,
                success, error, source_audit_id
         FROM audit_log WHERE id = ?1",
    )?;
    match stmt.query_row([id], |r| {
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
            source_audit_id: r.get(9)?,
        })
    }) {
        Ok(e) => Ok(Some(e)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Phase 16 — hard-delete audit rows whose `occurred_at` is strictly older
/// than the supplied cutoff. Returns the number of rows actually deleted.
///
/// Used by the Settings "Vyprázdnit historii" affordance.
pub fn purge_older_than(db: &Db, cutoff_unix_s: i64) -> Result<usize, DbError> {
    let conn = db.pool().get()?;
    let n = conn.execute(
        "DELETE FROM audit_log WHERE occurred_at < ?1",
        [cutoff_unix_s],
    )?;
    Ok(n)
}

/// Total count of rows in `audit_log`. Used by tests + cache stats.
pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))?;
    Ok(n)
}
