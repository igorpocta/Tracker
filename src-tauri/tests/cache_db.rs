use tempfile::TempDir;
use tracker_lib::cache::issues::{get_by_key, search, upsert, IssueRow};
use tracker_lib::cache::timer::{get as timer_get, start as timer_start, stop as timer_stop};
use tracker_lib::cache::settings::{get as setting_get, set as setting_set};
use tracker_lib::cache::worklogs::{recent as worklog_recent, record as worklog_record, WorklogRow};
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

#[test]
fn timer_singleton_overwrites_on_start() {
    let (_d, db) = fresh_db();
    timer_start(&db, "A-1", 1_000).unwrap();
    timer_start(&db, "B-2", 2_000).unwrap();
    let t = timer_get(&db).unwrap().unwrap();
    assert_eq!(t.issue_key, "B-2");
    assert_eq!(t.started_at, 2_000);
}

#[test]
fn timer_stop_clears_state() {
    let (_d, db) = fresh_db();
    timer_start(&db, "A-1", 1).unwrap();
    timer_stop(&db).unwrap();
    assert!(timer_get(&db).unwrap().is_none());
}

#[test]
fn timer_get_returns_none_when_no_timer() {
    let (_d, db) = fresh_db();
    assert!(timer_get(&db).unwrap().is_none());
}

#[test]
fn record_and_list_worklogs_ordered_by_logged_at_desc() {
    let (_d, db) = fresh_db();
    let id1 = worklog_record(
        &db,
        &WorklogRow {
            issue_key: "A-1".into(),
            duration_s: 600,
            started_at: 1,
            logged_at: 100,
            ..Default::default()
        },
    )
    .unwrap();
    let id2 = worklog_record(
        &db,
        &WorklogRow {
            issue_key: "A-2".into(),
            duration_s: 300,
            started_at: 1,
            logged_at: 200,
            ..Default::default()
        },
    )
    .unwrap();

    assert_ne!(id1, id2, "ids must be auto-assigned and unique");

    let rows = worklog_recent(&db, 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].issue_key, "A-2");
    assert_eq!(rows[1].issue_key, "A-1");
    assert_eq!(rows[0].id, Some(id2));
    assert_eq!(rows[1].id, Some(id1));
}

#[test]
fn recent_respects_limit() {
    let (_d, db) = fresh_db();
    for i in 0..5 {
        worklog_record(
            &db,
            &WorklogRow {
                issue_key: format!("X-{i}"),
                duration_s: 60,
                started_at: 0,
                logged_at: i,
                ..Default::default()
            },
        )
        .unwrap();
    }
    let rows = worklog_recent(&db, 3).unwrap();
    assert_eq!(rows.len(), 3);
}

#[test]
fn settings_get_returns_none_for_missing_key() {
    let (_d, db) = fresh_db();
    assert!(setting_get(&db, "no.such.key").unwrap().is_none());
}

#[test]
fn settings_set_and_get_roundtrip() {
    let (_d, db) = fresh_db();
    setting_set(&db, "daily_goal_minutes", "480").unwrap();
    assert_eq!(
        setting_get(&db, "daily_goal_minutes").unwrap().as_deref(),
        Some("480")
    );
}

#[test]
fn settings_set_overwrites_existing_value() {
    let (_d, db) = fresh_db();
    setting_set(&db, "k", "v1").unwrap();
    setting_set(&db, "k", "v2").unwrap();
    assert_eq!(setting_get(&db, "k").unwrap().as_deref(), Some("v2"));
}
