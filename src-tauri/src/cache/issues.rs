//! Issues cache layer — operates against the multi-provider `issues_v2`
//! table introduced in migration 0012.
//!
//! Stores a minimal, provider-agnostic representation of tasks pulled from
//! whichever provider owns each connection. Worklogs reference issues via
//! `(connection_id, issue_key)`; the relation is purely logical (no SQL
//! foreign key) because keys can change upstream and we'd rather keep the
//! historical worklog than cascade-orphan it.

use super::db::{Db, DbError};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

/// One row in `issues_v2`.
///
/// Serialization vystaví navíc legacy alias `summary` (= `name`) a
/// `parent_summary` (= `parent_name`) pro frontend, který tato pole stále
/// používá. Až FE přejde na nová jména, aliasy můžou pryč.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct IssueRow {
    pub id: Option<i64>,
    pub connection_id: i64,
    /// Provider's native id (numeric for Freelo, alphanumeric for Jira).
    #[serde(default)]
    pub issue_id: String,
    /// Human-readable key (`"DEV-792"`, `"FREELO-12345"`).
    pub issue_key: String,
    #[serde(alias = "summary", default)]
    pub name: String,
    pub parent_key: Option<String>,
    #[serde(alias = "parent_summary", default)]
    pub parent_name: Option<String>,
    /// Last-known status string from the provider, e.g. `"In Progress"`.
    pub status: Option<String>,
    /// `true` for closed/archived tasks (suppressed in the picker).
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    /// Provider's `updated`/`date_edited_at` timestamp, used for incremental
    /// sync and for the "recently changed" sort in the task picker.
    pub remote_updated_at: Option<i64>,
    pub last_synced_at: Option<i64>,
}

impl Serialize for IssueRow {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_struct("IssueRow", 15)?;
        m.serialize_field("id", &self.id)?;
        m.serialize_field("connection_id", &self.connection_id)?;
        m.serialize_field("issue_id", &self.issue_id)?;
        m.serialize_field("issue_key", &self.issue_key)?;
        m.serialize_field("name", &self.name)?;
        m.serialize_field("parent_key", &self.parent_key)?;
        m.serialize_field("parent_name", &self.parent_name)?;
        m.serialize_field("status", &self.status)?;
        m.serialize_field("is_archived", &self.is_archived)?;
        m.serialize_field("created_at", &self.created_at)?;
        m.serialize_field("updated_at", &self.updated_at)?;
        m.serialize_field("remote_updated_at", &self.remote_updated_at)?;
        m.serialize_field("last_synced_at", &self.last_synced_at)?;
        // Legacy aliasy pro FE.
        m.serialize_field("summary", &self.name)?;
        m.serialize_field("parent_summary", &self.parent_name)?;
        m.end()
    }
}

const SELECT_COLS: &str = "id, connection_id, issue_id, issue_key, name,
                           parent_key, parent_name, status, is_archived,
                           created_at, updated_at, remote_updated_at, last_synced_at";

/// Insert or update keyed by `(connection_id, issue_key)`. On UPDATE the
/// `created_at` and `id` are preserved.
pub fn upsert(db: &Db, issue: &IssueRow) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO issues_v2 (
            connection_id, issue_id, issue_key, name,
            parent_key, parent_name, status, is_archived,
            created_at, updated_at, remote_updated_at, last_synced_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(connection_id, issue_key) DO UPDATE SET
            issue_id          = excluded.issue_id,
            name              = excluded.name,
            parent_key        = excluded.parent_key,
            parent_name       = excluded.parent_name,
            status            = excluded.status,
            is_archived       = excluded.is_archived,
            updated_at        = excluded.updated_at,
            remote_updated_at = excluded.remote_updated_at,
            last_synced_at    = excluded.last_synced_at",
        rusqlite::params![
            issue.connection_id,
            issue.issue_id,
            issue.issue_key,
            issue.name,
            issue.parent_key,
            issue.parent_name,
            issue.status,
            if issue.is_archived { 1 } else { 0 },
            issue.created_at,
            issue.updated_at,
            issue.remote_updated_at,
            issue.last_synced_at,
        ],
    )?;
    Ok(())
}

/// Look up by issue key. If the key is unique across all connections
/// (current invariant — both providers use prefixed keys), the first match
/// wins.
pub fn get_by_key(db: &Db, key: &str) -> Result<Option<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let row = conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM issues_v2 WHERE issue_key = ?1 LIMIT 1"),
        [key],
        row_to_issue,
    );
    match row {
        Ok(i) => Ok(Some(i)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Issues that were updated most recently upstream, excluding archived and
/// Freelo project pseudo-issues. Used by the empty-state task picker.
pub fn recent(db: &Db, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS}
         FROM issues_v2
         WHERE is_archived = 0
           AND issue_key NOT LIKE 'FREELO-P-%'
         ORDER BY COALESCE(remote_updated_at, updated_at) DESC
         LIMIT ?1"
    ))?;
    let rows = stmt.query_map([limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Issues that have at least one worklog, ordered by the most recent
/// worklog timestamp. Used as the empty-state "recently tracked" picker.
pub fn suggested(db: &Db, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT i.id, i.connection_id, i.issue_id, i.issue_key, i.name,
                i.parent_key, i.parent_name, i.status, i.is_archived,
                i.created_at, i.updated_at, i.remote_updated_at, i.last_synced_at
         FROM issues_v2 i
         INNER JOIN (
            SELECT issue_key, MAX(logged_at) AS last_logged
            FROM worklogs
            WHERE issue_key IS NOT NULL
              AND tombstoned_at IS NULL
            GROUP BY issue_key
         ) w ON w.issue_key = i.issue_key
         WHERE i.is_archived = 0
           AND i.issue_key NOT LIKE 'FREELO-P-%'
         ORDER BY w.last_logged DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn count(db: &Db) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM issues_v2", [], |r| r.get(0))?;
    Ok(n)
}

/// Look up the connection that owns an issue key.
pub fn get_connection_id_by_key(db: &Db, key: &str) -> Result<Option<i64>, DbError> {
    let conn = db.pool().get()?;
    let r = conn.query_row(
        "SELECT connection_id FROM issues_v2 WHERE issue_key = ?1 LIMIT 1",
        [key],
        |r| r.get::<_, i64>(0),
    );
    match r {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Substring search across key + name. Archived and Freelo project
/// pseudo-issues are excluded.
pub fn search(db: &Db, query: &str, limit: u32) -> Result<Vec<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    let q = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS}
         FROM issues_v2
         WHERE (lower(issue_key) LIKE ?1 OR lower(name) LIKE ?1)
           AND is_archived = 0
           AND issue_key NOT LIKE 'FREELO-P-%'
         ORDER BY COALESCE(remote_updated_at, updated_at) DESC
         LIMIT ?2"
    ))?;
    let rows = stmt.query_map(rusqlite::params![q, limit], row_to_issue)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn row_to_issue(r: &rusqlite::Row<'_>) -> rusqlite::Result<IssueRow> {
    Ok(IssueRow {
        id: r.get(0)?,
        connection_id: r.get(1)?,
        issue_id: r.get(2)?,
        issue_key: r.get(3)?,
        name: r.get(4)?,
        parent_key: r.get(5)?,
        parent_name: r.get(6)?,
        status: r.get(7)?,
        is_archived: r.get::<_, i64>(8)? != 0,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        remote_updated_at: r.get(11)?,
        last_synced_at: r.get(12)?,
    })
}
