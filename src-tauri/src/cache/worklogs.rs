//! Worklogs cache layer — operates against the multi-provider `worklogs`
//! table introduced in migration 0012.
//!
//! Each row holds one work entry. The schema is intentionally provider-
//! agnostic: the owning provider is derived from `connection_id`, never
//! stored on the worklog itself. Sync state lives in three columns:
//! `is_synced`, `synced_at`, `remote_id` (provider's worklog id, no prefix).
//!
//! `issue_key` is `NULL` for stopped-but-unassigned entries; otherwise it
//! joins to `issues_v2.issue_key` for display. Tombstoned rows are kept
//! forever (no retention sweep) as forensic / audit trail.

use super::db::{Db, DbError};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

/// One row in `worklogs`.
///
/// `summary` is a transient field populated from the `issues_v2` join when
/// the row is read — it never round-trips to the DB. Serialization to the
/// frontend includes derived fields (`duration_s`) and legacy aliases
/// (`comment`, `jira_worklog_id`, `source`, `pending_assignment`) so the
/// existing TS code keeps working without a rewrite.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct WorklogRow {
    pub id: Option<i64>,
    pub connection_id: Option<i64>,
    /// Provider-specific issue key (`"DEV-792"`, `"FREELO-12345"`).
    /// `None` for entries stopped without picking a task.
    pub issue_key: Option<String>,
    #[serde(alias = "comment")]
    pub description: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
    pub logged_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub is_synced: bool,
    pub synced_at: Option<i64>,
    /// Provider's worklog id, without any synthetic prefix.
    #[serde(alias = "jira_worklog_id")]
    pub remote_id: Option<String>,
    pub pending_delete_at: Option<i64>,
    pub tombstoned_at: Option<i64>,
    /// Transient — populated by SELECTs that JOIN `issues_v2`.
    #[serde(default)]
    pub summary: Option<String>,
}

impl WorklogRow {
    /// Derived duration in seconds.
    pub fn duration_s(&self) -> i64 {
        (self.ended_at - self.started_at).max(0)
    }
}

impl Serialize for WorklogRow {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_struct("WorklogRow", 19)?;
        m.serialize_field("id", &self.id)?;
        m.serialize_field("connection_id", &self.connection_id)?;
        m.serialize_field("issue_key", &self.issue_key)?;
        m.serialize_field("description", &self.description)?;
        m.serialize_field("started_at", &self.started_at)?;
        m.serialize_field("ended_at", &self.ended_at)?;
        m.serialize_field("logged_at", &self.logged_at)?;
        m.serialize_field("updated_at", &self.updated_at)?;
        m.serialize_field("is_synced", &self.is_synced)?;
        m.serialize_field("synced_at", &self.synced_at)?;
        m.serialize_field("remote_id", &self.remote_id)?;
        m.serialize_field("pending_delete_at", &self.pending_delete_at)?;
        m.serialize_field("tombstoned_at", &self.tombstoned_at)?;
        m.serialize_field("summary", &self.summary)?;
        // Derived (computed from started/ended).
        m.serialize_field("duration_s", &self.duration_s())?;
        // Legacy aliases for FE backwards-compat. Once the FE is on
        // {description, remote_id, is_synced}, these can go away.
        m.serialize_field("comment", &self.description)?;
        m.serialize_field("jira_worklog_id", &self.remote_id)?;
        m.serialize_field("source", if self.is_synced { "remote" } else { "local" })?;
        m.serialize_field("pending_assignment", &self.issue_key.is_none())?;
        m.end()
    }
}

// -----------------------------------------------------------------------------
// Inserts / upserts
// -----------------------------------------------------------------------------

/// Insert a fresh row. Used by every code path: timer-stop, manual entry,
/// provider sync. The caller controls `is_synced`/`synced_at`/`remote_id`.
pub fn record(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO worklogs (
            connection_id, issue_key, description,
            started_at, ended_at, logged_at, updated_at,
            is_synced, synced_at, remote_id,
            pending_delete_at, tombstoned_at
         )
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        rusqlite::params![
            w.connection_id,
            w.issue_key,
            w.description,
            w.started_at,
            w.ended_at,
            w.logged_at,
            w.updated_at,
            if w.is_synced { 1 } else { 0 },
            w.synced_at,
            w.remote_id,
            w.pending_delete_at,
            w.tombstoned_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Upsert a worklog pulled from a provider, keyed by `(connection_id,
/// remote_id)`. Both must be `Some(_)`; the function errors otherwise.
///
/// On UPDATE we preserve `logged_at` (the moment the row first appeared
/// locally) and refresh `updated_at` and `synced_at`.
pub fn upsert_from_remote(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let connection_id = w
        .connection_id
        .ok_or_else(|| DbError::Migration("upsert_from_remote: connection_id required".into()))?;
    let remote_id = w
        .remote_id
        .as_deref()
        .ok_or_else(|| DbError::Migration("upsert_from_remote: remote_id required".into()))?;

    let conn = db.pool().get()?;
    let existing: Option<i64> = match conn.query_row(
        "SELECT id FROM worklogs WHERE connection_id = ?1 AND remote_id = ?2",
        rusqlite::params![connection_id, remote_id],
        |r| r.get(0),
    ) {
        Ok(id) => Some(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(e.into()),
    };

    if let Some(id) = existing {
        conn.execute(
            "UPDATE worklogs SET
                issue_key = ?2,
                description = ?3,
                started_at = ?4,
                ended_at = ?5,
                updated_at = ?6,
                is_synced = 1,
                synced_at = ?7
             WHERE id = ?1",
            rusqlite::params![
                id,
                w.issue_key,
                w.description,
                w.started_at,
                w.ended_at,
                w.updated_at,
                w.synced_at,
            ],
        )?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO worklogs (
                connection_id, issue_key, description,
                started_at, ended_at, logged_at, updated_at,
                is_synced, synced_at, remote_id
             )
             VALUES (?1,?2,?3,?4,?5,?6,?7,1,?8,?9)",
            rusqlite::params![
                connection_id,
                w.issue_key,
                w.description,
                w.started_at,
                w.ended_at,
                w.logged_at,
                w.updated_at,
                w.synced_at,
                remote_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

// -----------------------------------------------------------------------------
// Reads
// -----------------------------------------------------------------------------

/// All columns from `worklogs` plus the joined `issues_v2.name` as
/// `summary`. Use this with `FROM worklogs LEFT JOIN issues_v2 ON ...` so
/// reads expose the task title without an extra round trip.
const SELECT_COLS: &str = "w.id, w.connection_id, w.issue_key, w.description,
                           w.started_at, w.ended_at, w.logged_at, w.updated_at,
                           w.is_synced, w.synced_at, w.remote_id,
                           w.pending_delete_at, w.tombstoned_at,
                           i.name";

const FROM_JOIN: &str = "FROM worklogs w
                          LEFT JOIN issues_v2 i
                            ON i.issue_key = w.issue_key
                           AND (i.connection_id = w.connection_id
                                OR w.connection_id IS NULL)";

pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM worklogs", [], |r| r.get(0))?;
    Ok(n)
}

pub fn recent(db: &Db, limit: u32) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN}
         WHERE w.tombstoned_at IS NULL
         ORDER BY w.logged_at DESC LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], row_to_worklog)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Worklogs with `started_at` in `[from_unix_s, to_unix_s]`, ordered most
/// recent first. Tombstoned rows are excluded.
pub fn for_date_range(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN}
         WHERE w.started_at BETWEEN ?1 AND ?2
           AND w.tombstoned_at IS NULL
         ORDER BY w.started_at DESC"
    ))?;
    let rows = stmt.query_map(rusqlite::params![from_unix_s, to_unix_s], row_to_worklog)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Diagnostic variant that includes tombstoned rows. Used by the
/// mark-and-sweep logic and audit views.
pub fn for_date_range_including_tombstoned(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN}
         WHERE w.started_at BETWEEN ?1 AND ?2
         ORDER BY w.started_at DESC"
    ))?;
    let rows = stmt.query_map(rusqlite::params![from_unix_s, to_unix_s], row_to_worklog)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Sum of derived duration for worklogs in `[from, to]`, excluding tombstones.
pub fn total_seconds_for_range(db: &Db, from_unix_s: i64, to_unix_s: i64) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN ended_at > started_at
                                 THEN ended_at - started_at ELSE 0 END), 0)
         FROM worklogs
         WHERE started_at BETWEEN ?1 AND ?2
           AND tombstoned_at IS NULL",
        rusqlite::params![from_unix_s, to_unix_s],
        |r| r.get(0),
    )?;
    Ok(total)
}

pub fn get_by_id(db: &Db, id: i64) -> Result<Option<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!("SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.id = ?1"))?;
    match stmt.query_row([id], row_to_worklog) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Look up a row by provider remote id within a specific connection.
pub fn get_by_remote_id(
    db: &Db,
    connection_id: i64,
    remote_id: &str,
) -> Result<Option<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN}
         WHERE w.connection_id = ?1 AND w.remote_id = ?2"
    ))?;
    match stmt.query_row(rusqlite::params![connection_id, remote_id], row_to_worklog) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Look up a row by `remote_id` across all connections. Used by command
/// dispatchers that only know the upstream id — the unique index guarantees
/// at most one match.
pub fn get_by_remote_id_any(db: &Db, remote_id: &str) -> Result<Option<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN} WHERE w.remote_id = ?1 LIMIT 1"
    ))?;
    match stmt.query_row([remote_id], row_to_worklog) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// -----------------------------------------------------------------------------
// Mutations used by the two-way sync commands
// -----------------------------------------------------------------------------

/// Update the editable fields on a worklog (issue_key, description,
/// start/end, optional sync stamp). When `synced_at` is `Some(_)` we also
/// set `is_synced = 1`.
pub fn update_fields(
    db: &Db,
    id: i64,
    issue_key: Option<&str>,
    description: Option<&str>,
    started_at: i64,
    ended_at: i64,
    synced_at: Option<i64>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE worklogs SET
            issue_key   = ?2,
            description = ?3,
            started_at  = ?4,
            ended_at    = ?5,
            updated_at  = ?6,
            is_synced   = CASE WHEN ?7 IS NOT NULL THEN 1 ELSE is_synced END,
            synced_at   = COALESCE(?7, synced_at)
         WHERE id = ?1",
        rusqlite::params![
            id,
            issue_key,
            description,
            started_at,
            ended_at,
            now,
            synced_at,
        ],
    )?;
    Ok(())
}

/// Soft-delete window. The 5s undo banner watches this column; if it stays
/// non-null at expiry the background task commits the actual remote DELETE
/// and then calls [`mark_tombstoned`].
pub fn mark_pending_delete(db: &Db, id: i64, now_unix_s: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE worklogs SET pending_delete_at = ?2, updated_at = ?2 WHERE id = ?1",
        rusqlite::params![id, now_unix_s],
    )?;
    Ok(())
}

pub fn clear_pending_delete(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE worklogs SET pending_delete_at = NULL, updated_at = ?2 WHERE id = ?1",
        rusqlite::params![id, now],
    )?;
    Ok(())
}

/// Mark a row tombstoned. Rows stay forever (no retention sweep) so the
/// audit trail is complete even after the provider drops the original.
pub fn mark_tombstoned(db: &Db, id: i64, now_unix_s: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE worklogs SET
            tombstoned_at = ?2,
            pending_delete_at = NULL,
            updated_at = ?2
         WHERE id = ?1",
        rusqlite::params![id, now_unix_s],
    )?;
    Ok(())
}

/// Mark tombstoned by `(connection_id, remote_id)`. Used by mark-and-sweep
/// after a sync pass to flag entries the provider no longer returns.
pub fn mark_tombstoned_by_remote_id(
    db: &Db,
    connection_id: i64,
    remote_id: &str,
    now_unix_s: i64,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE worklogs SET
            tombstoned_at = ?3,
            pending_delete_at = NULL,
            updated_at = ?3
         WHERE connection_id = ?1
           AND remote_id = ?2
           AND tombstoned_at IS NULL",
        rusqlite::params![connection_id, remote_id, now_unix_s],
    )?;
    Ok(())
}

/// Hard-delete a single row. Used after a successful `move_worklog`
/// composite operation that needs to discard the old entry entirely.
pub fn delete_row(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM worklogs WHERE id = ?1", [id])?;
    Ok(())
}

/// Rows whose `pending_delete_at` is older than `older_than_unix_s` and that
/// haven't been tombstoned yet. Used by startup recovery to fire deletes
/// that didn't get committed before the app crashed.
pub fn pending_deletes_older_than(
    db: &Db,
    older_than_unix_s: i64,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} {FROM_JOIN}
         WHERE w.pending_delete_at IS NOT NULL
           AND w.pending_delete_at < ?1
           AND w.tombstoned_at IS NULL
         ORDER BY w.pending_delete_at ASC"
    ))?;
    let mapped = stmt.query_map([older_than_unix_s], row_to_worklog)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// All `remote_id` values for a given connection whose `started_at` falls
/// inside the range. Used by mark-and-sweep: anything in this set that the
/// provider didn't return on the latest sync is presumed deleted upstream.
pub fn remote_ids_in_range(
    db: &Db,
    connection_id: i64,
    from_unix_s: i64,
    to_unix_s: i64,
) -> Result<Vec<String>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT remote_id FROM worklogs
         WHERE connection_id = ?1
           AND remote_id IS NOT NULL
           AND tombstoned_at IS NULL
           AND started_at BETWEEN ?2 AND ?3",
    )?;
    let mapped = stmt.query_map(
        rusqlite::params![connection_id, from_unix_s, to_unix_s],
        |r| r.get::<_, Option<String>>(0),
    )?;
    let mut out = Vec::new();
    for v in mapped {
        if let Some(s) = v? {
            out.push(s);
        }
    }
    Ok(out)
}

/// Attach an issue to a previously-unassigned worklog. If `remote_id` is
/// `Some(_)` we also stamp `is_synced = 1` and `synced_at = now`.
pub fn assign_issue(
    db: &Db,
    id: i64,
    connection_id: Option<i64>,
    issue_key: &str,
    remote_id: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE worklogs SET
            connection_id = COALESCE(?2, connection_id),
            issue_key     = ?3,
            remote_id     = COALESCE(?4, remote_id),
            is_synced     = CASE WHEN ?4 IS NOT NULL THEN 1 ELSE is_synced END,
            synced_at     = CASE WHEN ?4 IS NOT NULL THEN ?5 ELSE synced_at END,
            updated_at    = ?5
         WHERE id = ?1",
        rusqlite::params![id, connection_id, issue_key, remote_id, now],
    )?;
    Ok(())
}

/// Hard-delete a row that has never been synced (no `remote_id`). Used by
/// the timer-discard / local-only delete flow.
pub fn delete_local_only(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM worklogs WHERE id = ?1 AND remote_id IS NULL",
        [id],
    )?;
    Ok(())
}

fn row_to_worklog(r: &rusqlite::Row<'_>) -> rusqlite::Result<WorklogRow> {
    Ok(WorklogRow {
        id: r.get(0)?,
        connection_id: r.get(1)?,
        issue_key: r.get(2)?,
        description: r.get(3)?,
        started_at: r.get(4)?,
        ended_at: r.get(5)?,
        logged_at: r.get(6)?,
        updated_at: r.get(7)?,
        is_synced: r.get::<_, i64>(8)? != 0,
        synced_at: r.get(9)?,
        remote_id: r.get(10)?,
        pending_delete_at: r.get(11)?,
        tombstoned_at: r.get(12)?,
        // From the LEFT JOIN with `issues_v2`. `None` if the task hasn't
        // been synced into the issues cache yet.
        summary: r.get(13)?,
    })
}
