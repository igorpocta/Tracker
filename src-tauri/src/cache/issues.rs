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

/// Insert or update. Tabulka má dva UNIQUE indexy — `(connection_id,
/// issue_key)` a `(connection_id, issue_id)`. `issue_id` je stabilní
/// (interní ID z provideru), `issue_key` se může v čase měnit (Jira
/// project rename, přesun issue mezi projekty). Proto: nejdřív UPDATE
/// podle stabilního páru `(connection_id, issue_id)` — to ošetří změnu
/// klíče a zachová lokální `id` i `created_at`. Pokud taková řádka
/// neexistuje, INSERT s `ON CONFLICT(connection_id, issue_key)` jako
/// fallback (běžný nový záznam, případně reuse keyu novým id).
pub fn upsert(db: &Db, issue: &IssueRow) -> Result<(), DbError> {
    let mut conn = db.pool().get()?;
    let tx = conn.transaction()?;

    let updated = tx.execute(
        "UPDATE issues_v2 SET
            issue_key         = ?1,
            name              = ?2,
            parent_key        = ?3,
            parent_name       = ?4,
            status            = ?5,
            is_archived       = ?6,
            updated_at        = ?7,
            remote_updated_at = ?8,
            last_synced_at    = ?9
         WHERE connection_id = ?10 AND issue_id = ?11",
        rusqlite::params![
            issue.issue_key,
            issue.name,
            issue.parent_key,
            issue.parent_name,
            issue.status,
            if issue.is_archived { 1 } else { 0 },
            issue.updated_at,
            issue.remote_updated_at,
            issue.last_synced_at,
            issue.connection_id,
            issue.issue_id,
        ],
    )?;

    if updated == 0 {
        tx.execute(
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
    }

    tx.commit()?;
    Ok(())
}

/// Look up by issue key. If the key is unique across all connections
/// (current invariant — both providers use prefixed keys), the first match
/// wins.
pub fn get_by_key(db: &Db, key: &str) -> Result<Option<IssueRow>, DbError> {
    let conn = db.pool().get()?;
    // Deterministic pick when a key exists in several connections.
    let row = conn.query_row(
        &format!(
            "SELECT {SELECT_COLS} FROM issues_v2 WHERE issue_key = ?1 \
             ORDER BY connection_id ASC LIMIT 1"
        ),
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

/// Look up the connection that owns an issue key. When the same key exists in
/// more than one connection (two tenants sharing a project key), prefer an
/// ENABLED connection and otherwise pick deterministically by id — never an
/// arbitrary row, which could route a worklog to the wrong / disabled tenant.
pub fn get_connection_id_by_key(db: &Db, key: &str) -> Result<Option<i64>, DbError> {
    let conn = db.pool().get()?;
    let r = conn.query_row(
        "SELECT i.connection_id
         FROM issues_v2 i
         LEFT JOIN connections c ON c.id = i.connection_id
         WHERE i.issue_key = ?1
         ORDER BY c.enabled DESC, i.connection_id ASC
         LIMIT 1",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Db;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    fn new_connection(db: &Db) -> i64 {
        crate::cache::connections::insert(
            db,
            crate::cache::connections::NewConnection {
                provider: "jira",
                name: "test",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap()
    }

    // Regrese: Jira může u stejného `issue.id` změnit `issue.key`
    // (přesun mezi projekty, project rename). Stará verze upsertu
    // padala na UNIQUE constraint (connection_id, issue_id), protože
    // ON CONFLICT mířil jen na (connection_id, issue_key).
    #[test]
    fn upsert_handles_issue_key_change_for_same_issue_id() {
        let db = open_db();
        let connection_id = new_connection(&db);

        let original = IssueRow {
            connection_id,
            issue_id: "10001".into(),
            issue_key: "OLD-1".into(),
            name: "before".into(),
            created_at: 100,
            updated_at: 100,
            ..Default::default()
        };
        upsert(&db, &original).unwrap();

        let original_id = get_by_key(&db, "OLD-1").unwrap().unwrap().id;

        let renamed = IssueRow {
            connection_id,
            issue_id: "10001".into(),
            issue_key: "NEW-1".into(),
            name: "after".into(),
            created_at: 200,
            updated_at: 200,
            ..Default::default()
        };
        upsert(&db, &renamed).unwrap();

        assert_eq!(count(&db).unwrap(), 1, "musí přepsat, ne vytvořit duplikát");
        assert!(get_by_key(&db, "OLD-1").unwrap().is_none());
        let found = get_by_key(&db, "NEW-1").unwrap().unwrap();
        assert_eq!(found.id, original_id, "id se musí zachovat");
        assert_eq!(found.created_at, 100, "created_at se musí zachovat");
        assert_eq!(found.name, "after");
    }

    #[test]
    fn get_connection_id_by_key_prefers_enabled_connection() {
        use crate::cache::connections::{insert as insert_conn, NewConnection};
        let db = open_db();
        // Disabled connection inserted first (lower id) — without an
        // enabled-preference + ORDER BY, the arbitrary LIMIT 1 would return it.
        let disabled = insert_conn(
            &db,
            NewConnection {
                provider: "jira",
                name: "old",
                enabled: false,
                config_json: "{}",
            },
        )
        .unwrap();
        let enabled = insert_conn(
            &db,
            NewConnection {
                provider: "jira",
                name: "new",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        for cid in [disabled, enabled] {
            upsert(
                &db,
                &IssueRow {
                    connection_id: cid,
                    issue_id: format!("id-{cid}"),
                    issue_key: "SHARED-1".into(),
                    name: "x".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            get_connection_id_by_key(&db, "SHARED-1").unwrap(),
            Some(enabled),
            "must route to the enabled connection, not an arbitrary one"
        );
    }
}
