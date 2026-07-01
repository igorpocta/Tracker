//! Locally-hidden Jira dashboard issues (feedback: hide/show from přehled).
//!
//! Purely local UI state — hiding never touches Jira. Keyed by
//! `(connection_id, issue_key)` so the same key in two tenants hides
//! independently.

use super::db::{Db, DbError};
use chrono::Utc;

pub fn hide(db: &Db, connection_id: i64, issue_key: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO dashboard_hidden_issues (connection_id, issue_key, hidden_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(connection_id, issue_key) DO NOTHING",
        rusqlite::params![connection_id, issue_key, Utc::now().timestamp()],
    )?;
    Ok(())
}

pub fn unhide(db: &Db, connection_id: i64, issue_key: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM dashboard_hidden_issues WHERE connection_id = ?1 AND issue_key = ?2",
        rusqlite::params![connection_id, issue_key],
    )?;
    Ok(())
}

/// All hidden `(connection_id, issue_key)` pairs — used to annotate dashboard
/// rows in one pass.
pub fn list(db: &Db) -> Result<Vec<(i64, String)>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare("SELECT connection_id, issue_key FROM dashboard_hidden_issues")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
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
        let path = dir.path().join("dh.db");
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
    fn hide_unhide_is_scoped_to_connection() {
        let db = open_db();
        let c1 = seed_conn(&db, "A");
        let c2 = seed_conn(&db, "B");

        hide(&db, c1, "PROJ-1").unwrap();
        hide(&db, c2, "PROJ-1").unwrap();
        // Idempotent — second hide of the same pair is a no-op.
        hide(&db, c1, "PROJ-1").unwrap();
        assert_eq!(list(&db).unwrap().len(), 2);

        unhide(&db, c1, "PROJ-1").unwrap();
        let hidden = list(&db).unwrap();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0], (c2, "PROJ-1".to_string()));
    }
}
