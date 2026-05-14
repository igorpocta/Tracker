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
}

/// Insert a new locally-created worklog. The row is appended; no dedup is
/// attempted because timer stops always produce a unique entry.
pub fn record(db: &Db, w: &WorklogRow) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let source = if w.source.is_empty() { "local" } else { w.source.as_str() };
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
            w.jira_worklog_id,
            w.author_account_id,
            source,
            w.updated_at_jira,
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

    let source = if w.source.is_empty() { "jira" } else { w.source.as_str() };

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
                comment, jira_worklog_id, author_account_id, source, updated_at
         FROM recent_worklogs ORDER BY logged_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_worklog)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Worklogs with `started_at` in `[from_unix_s, to_unix_s]`, ordered most
/// recent first. If `author_account_id` is `Some(_)`, restricts to that author
/// (typically the current user). `None` returns all authors.
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
                        comment, jira_worklog_id, author_account_id, source, updated_at
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
                        comment, jira_worklog_id, author_account_id, source, updated_at
                 FROM recent_worklogs
                 WHERE started_at BETWEEN ?1 AND ?2
                 ORDER BY started_at DESC",
            )?;
            let mapped = stmt.query_map(
                rusqlite::params![from_unix_s, to_unix_s],
                row_to_worklog,
            )?;
            mapped.collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(rows)
}

/// Sum of `duration_s` for worklogs in `[from, to]` (optionally filtered by author).
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
             WHERE started_at BETWEEN ?1 AND ?2 AND author_account_id = ?3",
            rusqlite::params![from_unix_s, to_unix_s, account],
            |r| r.get(0),
        )?,
        None => conn.query_row(
            "SELECT COALESCE(SUM(duration_s), 0) FROM recent_worklogs
             WHERE started_at BETWEEN ?1 AND ?2",
            rusqlite::params![from_unix_s, to_unix_s],
            |r| r.get(0),
        )?,
    };
    Ok(total)
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
    })
}
