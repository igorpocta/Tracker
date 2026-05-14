use tempfile::TempDir;
use tracker_lib::cache::issues::{get_by_key, search, upsert, IssueRow};
use tracker_lib::cache::Db;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    (dir, db)
}

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

#[test]
fn upsert_and_get_issue_roundtrip() {
    let (_d, db) = fresh_db();
    let issue = IssueRow {
        issue_key: "ABC-1".into(),
        summary: "Fix login".into(),
        status_category: Some("indeterminate".into()),
        updated_at: 1_700_000_000,
        ..Default::default()
    };
    upsert(&db, &issue).unwrap();

    let got = get_by_key(&db, "ABC-1").unwrap().unwrap();
    assert_eq!(got.summary, "Fix login");
    assert_eq!(got.status_category.as_deref(), Some("indeterminate"));
}

#[test]
fn upsert_overwrites_existing_issue() {
    let (_d, db) = fresh_db();
    upsert(
        &db,
        &IssueRow {
            issue_key: "ABC-1".into(),
            summary: "Old summary".into(),
            updated_at: 1,
            ..Default::default()
        },
    )
    .unwrap();
    upsert(
        &db,
        &IssueRow {
            issue_key: "ABC-1".into(),
            summary: "New summary".into(),
            updated_at: 2,
            ..Default::default()
        },
    )
    .unwrap();

    let got = get_by_key(&db, "ABC-1").unwrap().unwrap();
    assert_eq!(got.summary, "New summary");
    assert_eq!(got.updated_at, 2);
}

#[test]
fn get_by_key_returns_none_for_missing() {
    let (_d, db) = fresh_db();
    assert!(get_by_key(&db, "NOPE-1").unwrap().is_none());
}

#[test]
fn search_matches_summary_substring_case_insensitive() {
    let (_d, db) = fresh_db();
    upsert(
        &db,
        &IssueRow {
            issue_key: "A-1".into(),
            summary: "Login bug".into(),
            updated_at: 1,
            ..Default::default()
        },
    )
    .unwrap();
    upsert(
        &db,
        &IssueRow {
            issue_key: "A-2".into(),
            summary: "Database migration".into(),
            updated_at: 2,
            ..Default::default()
        },
    )
    .unwrap();

    let hits = search(&db, "LOGIN", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].issue_key, "A-1");
}
