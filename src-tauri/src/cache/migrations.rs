use super::db::{Db, DbError};
use rusqlite::Connection;

const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_worklog_authority.sql")),
    (3, include_str!("../../migrations/0003_worklog_tombstone.sql")),
    (4, include_str!("../../migrations/0004_audit_log.sql")),
    (5, include_str!("../../migrations/0005_audit_log_linkage.sql")),
];

pub fn run(db: &Db) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for (version, sql) in MIGRATIONS {
        if !is_applied(&conn, *version)? {
            conn.execute_batch(sql)
                .map_err(|e| DbError::Migration(format!("v{version}: {e}")))?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, strftime('%s','now'))",
                [version],
            )?;
        }
    }
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
