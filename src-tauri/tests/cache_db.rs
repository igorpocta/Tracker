use tempfile::TempDir;
use tracker_lib::cache::Db;

#[test]
fn open_creates_database_file_and_wal_mode() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tracker.db");

    let db = Db::open(&path).expect("open");
    let conn = db.pool().get().unwrap();

    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");

    assert!(path.exists());
}

#[test]
fn migrations_create_all_core_tables() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let conn = db.pool().get().unwrap();

    for table in [
        "issues",
        "active_timer",
        "recent_worklogs",
        "app_settings",
        "schema_migrations",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "table {table} should exist");
    }
}
