use super::db::{Db, DbError};
use rusqlite::Connection;

const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
    (
        2,
        include_str!("../../migrations/0002_worklog_authority.sql"),
    ),
    (
        3,
        include_str!("../../migrations/0003_worklog_tombstone.sql"),
    ),
    (4, include_str!("../../migrations/0004_audit_log.sql")),
    (
        5,
        include_str!("../../migrations/0005_audit_log_linkage.sql"),
    ),
    (6, include_str!("../../migrations/0006_connections.sql")),
    (7, include_str!("../../migrations/0007_calendar.sql")),
    (8, include_str!("../../migrations/0008_activity.sql")),
    (
        9,
        include_str!("../../migrations/0009_pending_assignment.sql"),
    ),
    (
        10,
        include_str!("../../migrations/0010_active_timer_comment.sql"),
    ),
    (11, include_str!("../../migrations/0011_favorites.sql")),
    (12, include_str!("../../migrations/0012_worklogs_v2.sql")),
    (13, include_str!("../../migrations/0013_sync_runs.sql")),
    (14, include_str!("../../migrations/0014_project_colors.sql")),
];

pub fn run(db: &Db) -> Result<(), DbError> {
    let mut conn = db.pool().get()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for (version, sql) in MIGRATIONS {
        if !is_applied(&conn, *version)? {
            apply_migration(&mut conn, *version, sql)?;
        }
    }
    Ok(())
}

/// Apply a single migration atomically: the schema statements AND the
/// `schema_migrations` version row are written in ONE transaction, so a failure
/// mid-migration rolls back entirely. Without this, a partial multi-statement
/// migration (e.g. a RENAME that lands before a CREATE that fails) left the
/// version unrecorded and the next launch re-ran non-idempotent DDL, bricking
/// the DB permanently.
fn apply_migration(conn: &mut Connection, version: i32, sql: &str) -> Result<(), DbError> {
    let tx = conn.transaction()?;
    tx.execute_batch(sql)
        .map_err(|e| DbError::Migration(format!("v{version}: {e}")))?;
    tx.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, strftime('%s','now'))",
        [version],
    )?;
    tx.commit()?;
    Ok(())
}

fn is_applied(conn: &Connection, v: i32) -> Result<bool, DbError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
        [v],
        |r| r.get(0),
    )?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Db;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mig.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn migration_failure_rolls_back_atomically() {
        let db = open_db();
        let mut conn = db.pool().get().unwrap();
        // A multi-statement migration whose second statement is invalid. The
        // first CREATE must NOT survive, and the version must NOT be recorded.
        let bad = "CREATE TABLE mig_probe (x INTEGER); THIS IS NOT SQL;";
        let res = apply_migration(&mut conn, 9001, bad);
        assert!(res.is_err(), "broken migration should error");
        let probe_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mig_probe')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!probe_exists, "partial DDL must be rolled back");
        assert!(!is_applied(&conn, 9001).unwrap(), "version must not be recorded");
    }

    #[test]
    fn successful_migration_commits_and_records_version() {
        let db = open_db();
        let mut conn = db.pool().get().unwrap();
        apply_migration(&mut conn, 9002, "CREATE TABLE mig_ok (x INTEGER);").unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='mig_ok')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists);
        assert!(is_applied(&conn, 9002).unwrap());
    }
}
