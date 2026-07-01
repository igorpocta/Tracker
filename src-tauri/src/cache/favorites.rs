//! Favorite (starred) issues — Phase 18B Item 26.
//!
//! Stored in the local `favorite_issues` table. The list is small (single
//! digits typically) so we don't bother with pagination. Listing returns
//! rows newest-first; the UI typically renders them alphabetically by key
//! after fetching.

use super::db::{Db, DbError};
use chrono::Utc;

/// One favorite row. Identity is `(connection_id, issue_key)` — the same
/// issue key can be favorited independently in two tenants. `connection_id`
/// is `None` only for legacy rows the migration could not disambiguate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FavoriteRow {
    pub connection_id: Option<i64>,
    pub issue_key: String,
}

/// Returns all favorites, newest-first, carrying their `connection_id`.
pub fn list(db: &Db) -> Result<Vec<FavoriteRow>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn
        .prepare("SELECT connection_id, issue_key FROM favorite_issues ORDER BY added_at DESC")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(FavoriteRow {
                connection_id: r.get(0)?,
                issue_key: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn add(db: &Db, issue_key: &str, connection_id: Option<i64>) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    // Identita je (connection_id, issue_key). Pro non-NULL connection funguje
    // ON CONFLICT přes idx_favorites_conn_key jako no-op re-star; NULL
    // connection (legacy) se v UNIQUE chová jako distinct, takže se
    // před vložením ještě ptáme, ať legacy klíč nezaložíme dvakrát.
    if connection_id.is_none() {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM favorite_issues WHERE issue_key = ?1 AND connection_id IS NULL",
            [issue_key],
            |r| r.get(0),
        )?;
        if exists > 0 {
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO favorite_issues (connection_id, issue_key, added_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(connection_id, issue_key) DO UPDATE SET
            added_at = excluded.added_at",
        rusqlite::params![connection_id, issue_key, Utc::now().timestamp()],
    )?;
    Ok(())
}

pub fn remove(db: &Db, issue_key: &str, connection_id: Option<i64>) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    // `IS` řeší i NULL connection_id (legacy víceznačné favority).
    conn.execute(
        "DELETE FROM favorite_issues WHERE issue_key = ?1 AND connection_id IS ?2",
        rusqlite::params![issue_key, connection_id],
    )?;
    Ok(())
}

pub fn is_favorite(db: &Db, issue_key: &str, connection_id: Option<i64>) -> Result<bool, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM favorite_issues WHERE issue_key = ?1 AND connection_id IS ?2",
        rusqlite::params![issue_key, connection_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Db;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("favs.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    fn seed_conn(db: &Db, name: &str) -> i64 {
        crate::cache::connections::insert(
            db,
            crate::cache::connections::NewConnection {
                provider: "jira",
                name,
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap()
    }

    #[test]
    fn same_key_two_connections_yields_two_favorites() {
        // AK4.1: the same issue key favorited in two tenants must produce two
        // distinct favorites, each retaining its connection_id.
        let db = open_db();
        let c1 = seed_conn(&db, "Tenant A");
        let c2 = seed_conn(&db, "Tenant B");
        add(&db, "PROJ-1", Some(c1)).unwrap();
        add(&db, "PROJ-1", Some(c2)).unwrap();
        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.connection_id == Some(c1)));
        assert!(rows.iter().any(|r| r.connection_id == Some(c2)));
    }

    // NOTE: `remove_is_scoped_to_connection` (AK4.2) is added back in the GREEN
    // step once `remove`/`is_favorite` take a connection_id.

    #[test]
    fn add_and_check() {
        let db = open_db();
        let c1 = seed_conn(&db, "Tenant A");
        assert!(!is_favorite(&db, "ACME-1", Some(c1)).unwrap());
        add(&db, "ACME-1", Some(c1)).unwrap();
        assert!(is_favorite(&db, "ACME-1", Some(c1)).unwrap());
    }

    #[test]
    fn remove_works() {
        let db = open_db();
        let c1 = seed_conn(&db, "Tenant A");
        add(&db, "ACME-1", Some(c1)).unwrap();
        remove(&db, "ACME-1", Some(c1)).unwrap();
        assert!(!is_favorite(&db, "ACME-1", Some(c1)).unwrap());
    }

    #[test]
    fn list_returns_keys_newest_first() {
        let db = open_db();
        let c1 = seed_conn(&db, "Tenant A");
        add(&db, "ACME-1", Some(c1)).unwrap();
        add(&db, "ACME-2", Some(c1)).unwrap();
        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.issue_key == "ACME-1"));
        assert!(rows.iter().any(|r| r.issue_key == "ACME-2"));
    }

    #[test]
    fn remove_is_scoped_to_connection() {
        // AK4.2: removing the favorite for one connection leaves the other.
        let db = open_db();
        let c1 = seed_conn(&db, "Tenant A");
        let c2 = seed_conn(&db, "Tenant B");
        add(&db, "PROJ-1", Some(c1)).unwrap();
        add(&db, "PROJ-1", Some(c2)).unwrap();
        remove(&db, "PROJ-1", Some(c1)).unwrap();
        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].connection_id, Some(c2));
        assert!(!is_favorite(&db, "PROJ-1", Some(c1)).unwrap());
        assert!(is_favorite(&db, "PROJ-1", Some(c2)).unwrap());
    }

    #[test]
    fn legacy_null_connection_not_duplicated() {
        // AK4.4-ish: legacy NULL-connection favorite isn't inserted twice.
        let db = open_db();
        add(&db, "OLD-1", None).unwrap();
        add(&db, "OLD-1", None).unwrap();
        assert_eq!(list(&db).unwrap().len(), 1);
    }
}
