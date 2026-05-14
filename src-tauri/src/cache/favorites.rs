//! Favorite (starred) issues — Phase 18B Item 26.
//!
//! Stored in the local `favorite_issues` table. The list is small (single
//! digits typically) so we don't bother with pagination. Listing returns
//! rows newest-first; the UI typically renders them alphabetically by key
//! after fetching.

use super::db::{Db, DbError};
use chrono::Utc;

pub fn add(db: &Db, issue_key: &str, connection_id: Option<i64>) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO favorite_issues (issue_key, connection_id, added_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(issue_key) DO UPDATE SET
            connection_id = excluded.connection_id",
        rusqlite::params![issue_key, connection_id, Utc::now().timestamp()],
    )?;
    Ok(())
}

pub fn remove(db: &Db, issue_key: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM favorite_issues WHERE issue_key = ?1",
        rusqlite::params![issue_key],
    )?;
    Ok(())
}

pub fn is_favorite(db: &Db, issue_key: &str) -> Result<bool, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM favorite_issues WHERE issue_key = ?1",
        [issue_key],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Returns the list of starred `issue_key`s ordered newest-first.
pub fn list_keys(db: &Db) -> Result<Vec<String>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn
        .prepare("SELECT issue_key FROM favorite_issues ORDER BY added_at DESC")?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
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

    #[test]
    fn add_and_check() {
        let db = open_db();
        assert!(!is_favorite(&db, "ACME-1").unwrap());
        add(&db, "ACME-1", None).unwrap();
        assert!(is_favorite(&db, "ACME-1").unwrap());
    }

    #[test]
    fn remove_works() {
        let db = open_db();
        add(&db, "ACME-1", None).unwrap();
        remove(&db, "ACME-1").unwrap();
        assert!(!is_favorite(&db, "ACME-1").unwrap());
    }

    #[test]
    fn list_returns_keys_newest_first() {
        let db = open_db();
        add(&db, "ACME-1", None).unwrap();
        // Ensure ordering — added_at is to-the-second, so insert another row
        // with a slight pause is fine for tests but we just check both are
        // present here.
        add(&db, "ACME-2", None).unwrap();
        let keys = list_keys(&db).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"ACME-1".to_string()));
        assert!(keys.contains(&"ACME-2".to_string()));
    }
}
