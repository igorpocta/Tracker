use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

/// One row in `recent_worklogs`.
///
/// This table is the "all worklogs" cache: it holds entries created locally by
/// the timer-stop flow ([`source = "local"`]) as well as entries fetched from
/// Jira ([`source = "jira"`]), e.g. worklogs the user added directly via the
/// Jira web UI. Locally-created entries that also get pushed to Jira carry
/// `jira_worklog_id` so the next sync can dedupe via the unique index.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WorklogRow {
    pub id: Option<i64>,
    pub issue_key: String,
    pub issue_id: Option<String>,
    pub summary: Option<String>,
    pub duration_s: i64,
    /// Unix seconds — when the work was started (Jira's `started`).
    pub started_at: i64,
    /// Unix seconds — when the entry was logged (locally or in Jira).
    pub logged_at: i64,
    pub comment: Option<String>,
    pub jira_worklog_id: Option<String>,
    /// Jira `author.accountId` when source = "jira"; copied from the current
    /// user for source = "local".
    pub author_account_id: Option<String>,
    /// `"local"` or `"jira"`. Defaults to `"local"`.
    pub source: String,
    /// Jira's `updated` timestamp (Unix seconds) for entries pulled from Jira.
    pub updated_at_jira: Option<i64>,
    /// Phase 15: Unix seconds set when the user clicks trash on a worklog. A
    /// background task waits 5s before actually firing the Jira DELETE; the
    /// frontend optimistically hides the row in the meantime. Cleared by the
    /// undo flow.
    pub pending_delete_at: Option<i64>,
    /// Phase 15: Unix seconds set after a worklog has been deleted in Jira
    /// (either by us or detected via mark-and-sweep). Rows with this set are
    /// hidden from the default `for_date_range` query but kept for audit.
    pub tombstoned_at: Option<i64>,
    /// Phase 18A: true (1) for unassigned-timer worklogs (stopped without an
    /// issue selected). They have `issue_key = ""`, `source = "local"`, and no
    /// `jira_worklog_id`. The user assigns an issue later via
    /// `assign_worklog_issue` — at which point this flag is cleared and the
    /// row is POSTed to Jira.
    #[serde(default)]
    pub pending_assignment: bool,
}

/// Insert a new locally-created worklog. The row is appended; no dedup is
/// attempted because timer stops always produce a unique entry.
pub fn record(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let source = if w.source.is_empty() {
        "local"
    } else {
        w.source.as_str()
    };
    conn.execute(
        "INSERT INTO recent_worklogs (
            issue_key, issue_id, summary, duration_s, started_at, logged_at,
            comment, jira_worklog_id, author_account_id, source, updated_at,
            pending_assignment
         )
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        rusqlite::params![
            w.issue_key,
            w.issue_id,
            w.summary,
            w.duration_s,
            w.started_at,
            w.logged_at,
            w.comment,
            w.jira_worklog_id,
            w.author_account_id,
            source,
            w.updated_at_jira,
            if w.pending_assignment { 1 } else { 0 },
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Upsert a worklog fetched from Jira, keyed by `jira_worklog_id`.
///
/// Replaces any existing row with the same `jira_worklog_id`. Caller must
/// populate `jira_worklog_id`; this function returns the rowid of the
/// inserted/replaced row.
pub fn upsert_from_jira(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let jira_id = w
        .jira_worklog_id
        .as_deref()
        .ok_or_else(|| DbError::Migration("upsert_from_jira: jira_worklog_id required".into()))?;

    let conn = db.pool().get()?;
    // Find existing row with this jira id, if any.
    let existing: Option<i64> = match conn.query_row(
        "SELECT id FROM recent_worklogs WHERE jira_worklog_id = ?1",
        [jira_id],
        |r| r.get(0),
    ) {
        Ok(id) => Some(id),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(e.into()),
    };

    let source = if w.source.is_empty() {
        "jira"
    } else {
        w.source.as_str()
    };

    if let Some(id) = existing {
        conn.execute(
            "UPDATE recent_worklogs SET
                issue_key = ?2,
                issue_id = ?3,
                summary = ?4,
                duration_s = ?5,
                started_at = ?6,
                logged_at = ?7,
                comment = ?8,
                author_account_id = ?9,
                source = ?10,
                updated_at = ?11
             WHERE id = ?1",
            rusqlite::params![
                id,
                w.issue_key,
                w.issue_id,
                w.summary,
                w.duration_s,
                w.started_at,
                w.logged_at,
                w.comment,
                w.author_account_id,
                source,
                w.updated_at_jira,
            ],
        )?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO recent_worklogs (
                issue_key, issue_id, summary, duration_s, started_at, logged_at,
                comment, jira_worklog_id, author_account_id, source, updated_at
             )
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            rusqlite::params![
                w.issue_key,
                w.issue_id,
                w.summary,
                w.duration_s,
                w.started_at,
                w.logged_at,
                w.comment,
                jira_id,
                w.author_account_id,
                source,
                w.updated_at_jira,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }
}

/// Total number of worklog rows currently in the local cache.
pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM recent_worklogs", [], |r| r.get(0))?;
    Ok(n)
}

pub fn recent(db: &Db, limit: u32) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                comment, jira_worklog_id, author_account_id, source, updated_at,
                pending_delete_at, tombstoned_at, pending_assignment
         FROM recent_worklogs
         WHERE tombstoned_at IS NULL
         ORDER BY logged_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_worklog)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Worklogs with `started_at` in `[from_unix_s, to_unix_s]`, ordered most
/// recent first. If `author_account_id` is `Some(_)`, restricts to that author
/// (typically the current user). `None` returns all authors.
///
/// **Excludes tombstoned rows by default** (Phase 15). Use
/// [`for_date_range_including_tombstoned`] for forensic / debug queries.
pub fn for_date_range(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
    author_account_id: Option<&str>,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let rows: Vec<WorklogRow> = match author_account_id {
        Some(account) => {
            let mut stmt = conn.prepare(
                "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                        comment, jira_worklog_id, author_account_id, source, updated_at,
                        pending_delete_at, tombstoned_at, pending_assignment
                 FROM recent_worklogs
                 WHERE started_at BETWEEN ?1 AND ?2
                   AND author_account_id = ?3
                   AND tombstoned_at IS NULL
                 ORDER BY started_at DESC",
            )?;
            let mapped = stmt.query_map(
                rusqlite::params![from_unix_s, to_unix_s, account],
                row_to_worklog,
            )?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                        comment, jira_worklog_id, author_account_id, source, updated_at,
                        pending_delete_at, tombstoned_at, pending_assignment
                 FROM recent_worklogs
                 WHERE started_at BETWEEN ?1 AND ?2
                   AND tombstoned_at IS NULL
                 ORDER BY started_at DESC",
            )?;
            let mapped =
                stmt.query_map(rusqlite::params![from_unix_s, to_unix_s], row_to_worklog)?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(rows)
}

/// Diagnostic / audit variant of [`for_date_range`] that includes tombstoned
/// rows. Used by the mark-and-sweep logic and any future audit UI.
pub fn for_date_range_including_tombstoned(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
    author_account_id: Option<&str>,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let rows: Vec<WorklogRow> = match author_account_id {
        Some(account) => {
            let mut stmt = conn.prepare(
                "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                        comment, jira_worklog_id, author_account_id, source, updated_at,
                        pending_delete_at, tombstoned_at, pending_assignment
                 FROM recent_worklogs
                 WHERE started_at BETWEEN ?1 AND ?2
                   AND author_account_id = ?3
                 ORDER BY started_at DESC",
            )?;
            let mapped = stmt.query_map(
                rusqlite::params![from_unix_s, to_unix_s, account],
                row_to_worklog,
            )?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
        None => {
            let mut stmt = conn.prepare(
                "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                        comment, jira_worklog_id, author_account_id, source, updated_at,
                        pending_delete_at, tombstoned_at, pending_assignment
                 FROM recent_worklogs
                 WHERE started_at BETWEEN ?1 AND ?2
                 ORDER BY started_at DESC",
            )?;
            let mapped =
                stmt.query_map(rusqlite::params![from_unix_s, to_unix_s], row_to_worklog)?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(rows)
}

/// Sum of `duration_s` for worklogs in `[from, to]` (optionally filtered by
/// author). Tombstoned rows are excluded.
pub fn total_seconds_for_range(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
    author_account_id: Option<&str>,
) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let total: i64 = match author_account_id {
        Some(account) => conn.query_row(
            "SELECT COALESCE(SUM(duration_s), 0) FROM recent_worklogs
             WHERE started_at BETWEEN ?1 AND ?2
               AND author_account_id = ?3
               AND tombstoned_at IS NULL",
            rusqlite::params![from_unix_s, to_unix_s, account],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COALESCE(SUM(duration_s), 0) FROM recent_worklogs
             WHERE started_at BETWEEN ?1 AND ?2
               AND tombstoned_at IS NULL",
            rusqlite::params![from_unix_s, to_unix_s],
            |r| r.get(0),
        )?,
    };
    Ok(total)
}

// -----------------------------------------------------------------------------
// Phase 15: lookups + mutations used by the two-way sync commands.
// -----------------------------------------------------------------------------

/// Look up a row by its local rowid. Returns `None` if the row was hard-deleted
/// (tombstoned rows are still returned — callers need them for audit / undo).
pub fn get_by_id(db: &Db, id: i64) -> Result<Option<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                comment, jira_worklog_id, author_account_id, source, updated_at,
                pending_delete_at, tombstoned_at, pending_assignment
         FROM recent_worklogs WHERE id = ?1",
    )?;
    match stmt.query_row([id], row_to_worklog) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Look up a row by its Jira worklog id. Returns `None` if there is no such
/// row locally (e.g. the row was hard-deleted, or never synced).
pub fn get_by_jira_id(db: &Db, jira_id: &str) -> Result<Option<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                comment, jira_worklog_id, author_account_id, source, updated_at,
                pending_delete_at, tombstoned_at, pending_assignment
         FROM recent_worklogs WHERE jira_worklog_id = ?1",
    )?;
    match stmt.query_row([jira_id], row_to_worklog) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Update the mutable fields (issue_key, duration_s, started_at, comment) on a
/// local row. Used by the `update_worklog` and `move_worklog` Tauri commands.
#[allow(clippy::too_many_arguments)]
pub fn update_fields(
    db: &Db,
    id: i64,
    issue_key: &str,
    issue_id: Option<&str>,
    summary: Option<&str>,
    duration_s: i64,
    started_at: i64,
    comment: Option<&str>,
    updated_at_jira: Option<i64>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE recent_worklogs SET
            issue_key = ?2,
            issue_id = ?3,
            summary = ?4,
            duration_s = ?5,
            started_at = ?6,
            comment = ?7,
            updated_at = ?8
         WHERE id = ?1",
        rusqlite::params![
            id,
            issue_key,
            issue_id,
            summary,
            duration_s,
            started_at,
            comment,
            updated_at_jira,
        ],
    )?;
    Ok(())
}

/// Mark a row as pending-delete. The frontend optimistically hides the row,
/// then 5 seconds later a background task either commits the delete (calling
/// `Jira DELETE` and setting `tombstoned_at`) or — if the user pressed undo,
/// clearing `pending_delete_at` — does nothing.
pub fn mark_pending_delete(db: &Db, id: i64, now_unix_s: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE recent_worklogs SET pending_delete_at = ?2 WHERE id = ?1",
        rusqlite::params![id, now_unix_s],
    )?;
    Ok(())
}

/// Clear the `pending_delete_at` column (user pressed undo, or background
/// task already fired the delete).
pub fn clear_pending_delete(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE recent_worklogs SET pending_delete_at = NULL WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

/// Mark a row as tombstoned (deleted in Jira). The row is retained for ~30
/// days as a forensic audit trail; `purge_old_tombstoned` will hard-delete it
/// on the next sync after the retention window.
pub fn mark_tombstoned(db: &Db, id: i64, now_unix_s: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE recent_worklogs SET
            tombstoned_at = ?2,
            pending_delete_at = NULL
         WHERE id = ?1",
        rusqlite::params![id, now_unix_s],
    )?;
    Ok(())
}

/// Mark a row tombstoned by Jira worklog id. Convenience wrapper for the
/// mark-and-sweep code path in `worklog_sync`.
pub fn mark_tombstoned_by_jira_id(db: &Db, jira_id: &str, now_unix_s: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE recent_worklogs SET
            tombstoned_at = ?2,
            pending_delete_at = NULL
         WHERE jira_worklog_id = ?1
           AND tombstoned_at IS NULL",
        rusqlite::params![jira_id, now_unix_s],
    )?;
    Ok(())
}

/// Hard-delete rows whose `tombstoned_at` is older than `older_than_unix_s`.
/// Returns the number of rows actually removed.
pub fn purge_old_tombstoned(db: &Db, older_than_unix_s: i64) -> Result<usize, DbError> {
    let conn = db.pool().get()?;
    let n = conn.execute(
        "DELETE FROM recent_worklogs
         WHERE tombstoned_at IS NOT NULL
           AND tombstoned_at < ?1",
        [older_than_unix_s],
    )?;
    Ok(n)
}

/// Hard-delete a single row. Used by `move_worklog` after a successful
/// composite operation to discard the old row entirely.
pub fn delete_row(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM recent_worklogs WHERE id = ?1", [id])?;
    Ok(())
}

/// Return all rows currently in pending-delete state (used by startup recovery
/// to fire deletes that didn't get committed before the app crashed).
pub fn pending_deletes_older_than(
    db: &Db,
    older_than_unix_s: i64,
) -> Result<Vec<WorklogRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, issue_key, issue_id, summary, duration_s, started_at, logged_at,
                comment, jira_worklog_id, author_account_id, source, updated_at,
                pending_delete_at, tombstoned_at, pending_assignment
         FROM recent_worklogs
         WHERE pending_delete_at IS NOT NULL
           AND pending_delete_at < ?1
           AND tombstoned_at IS NULL
         ORDER BY pending_delete_at ASC",
    )?;
    let mapped = stmt.query_map([older_than_unix_s], row_to_worklog)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Return all `jira_worklog_id` values for `source='jira'` rows whose
/// `started_at` falls inside the given range, for the given author. Used by
/// the mark-and-sweep pass: anything in this set that the next Jira fetch
/// did NOT return is presumed deleted upstream.
pub fn jira_ids_in_range(
    db: &Db,
    from_unix_s: i64,
    to_unix_s: i64,
    author_account_id: &str,
) -> Result<Vec<String>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT jira_worklog_id FROM recent_worklogs
         WHERE source = 'jira'
           AND tombstoned_at IS NULL
           AND jira_worklog_id IS NOT NULL
           AND started_at BETWEEN ?1 AND ?2
           AND author_account_id = ?3",
    )?;
    let mapped = stmt.query_map(
        rusqlite::params![from_unix_s, to_unix_s, author_account_id],
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

fn row_to_worklog(r: &rusqlite::Row<'_>) -> rusqlite::Result<WorklogRow> {
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
        author_account_id: r.get(9)?,
        source: r.get(10)?,
        updated_at_jira: r.get(11)?,
        pending_delete_at: r.get(12)?,
        tombstoned_at: r.get(13)?,
        pending_assignment: r.get::<_, i64>(14).unwrap_or(0) != 0,
    })
}

/// Assign an issue to a previously-unassigned worklog. Clears the
/// `pending_assignment` flag and stamps a fresh `updated_at`.
pub fn assign_issue(
    db: &Db,
    id: i64,
    issue_key: &str,
    issue_id: Option<&str>,
    summary: Option<&str>,
    jira_worklog_id: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE recent_worklogs SET
            issue_key = ?2,
            issue_id = ?3,
            summary = ?4,
            jira_worklog_id = ?5,
            source = CASE WHEN ?5 IS NOT NULL THEN 'jira' ELSE source END,
            pending_assignment = 0,
            updated_at = ?6
         WHERE id = ?1",
        rusqlite::params![id, issue_key, issue_id, summary, jira_worklog_id, now,],
    )?;
    Ok(())
}

/// Hard-delete a local-only row (no Jira-side delete). The caller is
/// responsible for ensuring `jira_worklog_id IS NULL` — this function does
/// not check.
pub fn delete_local_only(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM recent_worklogs WHERE id = ?1 AND jira_worklog_id IS NULL",
        [id],
    )?;
    Ok(())
}
