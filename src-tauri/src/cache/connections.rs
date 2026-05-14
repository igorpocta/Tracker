//! Connection rows — multi-provider/multi-account support (Phase 18A).
//!
//! A "connection" is a single configured provider account (today: Jira). The
//! provider-specific configuration lives in `config_json` as an opaque blob; the
//! caller deserialises it according to `provider`. The API token (or other
//! secret) for connection `id` is stored in the secret file under the key
//! `connection:<id>:token` — never in this table.

use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

/// One row in the `connections` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRow {
    pub id: i64,
    pub provider: String,
    pub name: String,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Provider-specific JSON config. For Jira this is `{"base_url": "...",
    /// "email": "...", "sync_jql": "?", "my_issues_jql": "?"}`.
    pub config_json: String,
}

/// Args for [`insert`]; `id`/`created_at`/`updated_at` are filled in here.
pub struct NewConnection<'a> {
    pub provider: &'a str,
    pub name: &'a str,
    pub enabled: bool,
    pub config_json: &'a str,
}

pub fn insert(db: &Db, new: NewConnection<'_>) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO connections (provider, name, enabled, created_at, updated_at, config_json)
         VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
        rusqlite::params![
            new.provider,
            new.name,
            if new.enabled { 1 } else { 0 },
            now,
            new.config_json,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list(db: &Db) -> Result<Vec<ConnectionRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, provider, name, enabled, created_at, updated_at, config_json
         FROM connections
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], row_to_conn)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn get_by_id(db: &Db, id: i64) -> Result<Option<ConnectionRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT id, provider, name, enabled, created_at, updated_at, config_json
         FROM connections WHERE id = ?1",
    )?;
    match stmt.query_row([id], row_to_conn) {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Update mutable fields. `None` arguments leave that column untouched.
pub fn update_fields(
    db: &Db,
    id: i64,
    name: Option<&str>,
    enabled: Option<bool>,
    config_json: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    if let Some(n) = name {
        conn.execute(
            "UPDATE connections SET name = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, n, now],
        )?;
    }
    if let Some(e) = enabled {
        conn.execute(
            "UPDATE connections SET enabled = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, if e { 1 } else { 0 }, now],
        )?;
    }
    if let Some(cfg) = config_json {
        conn.execute(
            "UPDATE connections SET config_json = ?2, updated_at = ?3 WHERE id = ?1",
            rusqlite::params![id, cfg, now],
        )?;
    }
    Ok(())
}

pub fn delete(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM connections WHERE id = ?1", [id])?;
    Ok(())
}

fn row_to_conn(r: &rusqlite::Row<'_>) -> rusqlite::Result<ConnectionRow> {
    Ok(ConnectionRow {
        id: r.get(0)?,
        provider: r.get(1)?,
        name: r.get(2)?,
        enabled: r.get::<_, i64>(3)? != 0,
        created_at: r.get(4)?,
        updated_at: r.get(5)?,
        config_json: r.get(6)?,
    })
}

// -----------------------------------------------------------------------------
// Secret file key helpers
// -----------------------------------------------------------------------------

/// Key used to look up a connection's API token in the secret file.
pub fn token_key(id: i64) -> String {
    format!("connection:{id}:token")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn insert_then_list_roundtrips() {
        let db = open_db();
        let id = insert(
            &db,
            NewConnection {
                provider: "jira",
                name: "Work",
                enabled: true,
                config_json: r#"{"base_url":"https://acme.atlassian.net","email":"a@b"}"#,
            },
        )
        .unwrap();
        assert!(id > 0);
        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Work");
        assert!(rows[0].enabled);
    }

    #[test]
    fn update_fields_changes_only_provided_columns() {
        let db = open_db();
        let id = insert(
            &db,
            NewConnection {
                provider: "jira",
                name: "Work",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        update_fields(&db, id, Some("Side"), None, None).unwrap();
        let row = get_by_id(&db, id).unwrap().unwrap();
        assert_eq!(row.name, "Side");
        assert!(row.enabled);

        update_fields(&db, id, None, Some(false), None).unwrap();
        let row = get_by_id(&db, id).unwrap().unwrap();
        assert!(!row.enabled);
        assert_eq!(row.name, "Side");
    }

    #[test]
    fn unique_name_constraint() {
        let db = open_db();
        insert(
            &db,
            NewConnection {
                provider: "jira",
                name: "Work",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        let err = insert(
            &db,
            NewConnection {
                provider: "jira",
                name: "Work",
                enabled: true,
                config_json: "{}",
            },
        );
        assert!(err.is_err());
    }

    #[test]
    fn delete_removes_row() {
        let db = open_db();
        let id = insert(
            &db,
            NewConnection {
                provider: "jira",
                name: "Work",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        delete(&db, id).unwrap();
        assert!(get_by_id(&db, id).unwrap().is_none());
    }

    #[test]
    fn token_key_format() {
        assert_eq!(token_key(42), "connection:42:token");
    }
}
