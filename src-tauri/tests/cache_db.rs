use tempfile::TempDir;
use tracker_lib::cache::issues::{get_by_key, search, upsert, IssueRow};
use tracker_lib::cache::timer::{get as timer_get, start as timer_start, stop as timer_stop};
use tracker_lib::cache::settings::{get as setting_get, set as setting_set};
use tracker_lib::cache::worklogs::{
    for_date_range as worklog_for_date_range, recent as worklog_recent, record as worklog_record,
    total_seconds_for_range, upsert_from_jira, WorklogRow,
};
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

// -----------------------------------------------------------------------------
// Phase 11A: new worklog methods (Jira-sourced rows, date-range queries).
// -----------------------------------------------------------------------------

#[test]
fn migration_0002_adds_authority_columns() {
    let (_d, db) = fresh_db();
    let conn = db.pool().get().unwrap();

    // Read column names from PRAGMA table_info.
    let mut stmt = conn.prepare("PRAGMA table_info(recent_worklogs)").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    for col in ["author_account_id", "source", "updated_at"] {
        assert!(
            names.iter().any(|n| n == col),
            "column {col} should exist (have: {names:?})"
        );
    }
}

#[test]
fn upsert_from_jira_replaces_by_jira_id() {
    let (_d, db) = fresh_db();

    // Insert an initial Jira-sourced row.
    let id1 = upsert_from_jira(
        &db,
        &WorklogRow {
            issue_key: "ACME-1".into(),
            duration_s: 600,
            started_at: 100,
            logged_at: 100,
            comment: Some("initial".into()),
            jira_worklog_id: Some("J-42".into()),
            author_account_id: Some("user-a".into()),
            source: "jira".into(),
            updated_at_jira: Some(1_000),
            ..Default::default()
        },
    )
    .unwrap();

    // Replace via the same jira id with updated data.
    let id2 = upsert_from_jira(
        &db,
        &WorklogRow {
            issue_key: "ACME-1".into(),
            duration_s: 900,
            started_at: 100,
            logged_at: 200,
            comment: Some("updated".into()),
            jira_worklog_id: Some("J-42".into()),
            author_account_id: Some("user-a".into()),
            source: "jira".into(),
            updated_at_jira: Some(2_000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        id1, id2,
        "second upsert must reuse the original rowid (no duplicate row)"
    );

    let all = worklog_recent(&db, 50).unwrap();
    assert_eq!(all.len(), 1, "should still be exactly one row");
    assert_eq!(all[0].duration_s, 900);
    assert_eq!(all[0].comment.as_deref(), Some("updated"));
    assert_eq!(all[0].updated_at_jira, Some(2_000));
    assert_eq!(all[0].source, "jira");
}

#[test]
fn upsert_from_jira_rejects_missing_jira_id() {
    let (_d, db) = fresh_db();
    let result = upsert_from_jira(
        &db,
        &WorklogRow {
            issue_key: "A-1".into(),
            duration_s: 60,
            ..Default::default()
        },
    );
    assert!(result.is_err(), "missing jira_worklog_id must be rejected");
}

#[test]
fn for_date_range_filters_and_orders() {
    let (_d, db) = fresh_db();

    // started_at = 1000 — inside the range.
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "IN-1".into(),
            duration_s: 100,
            started_at: 1_000,
            logged_at: 1_000,
            author_account_id: Some("me".into()),
            source: "local".into(),
            ..Default::default()
        },
    )
    .unwrap();

    // started_at = 2000 — inside the range.
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "IN-2".into(),
            duration_s: 200,
            started_at: 2_000,
            logged_at: 2_000,
            author_account_id: Some("me".into()),
            source: "local".into(),
            ..Default::default()
        },
    )
    .unwrap();

    // started_at = 5000 — outside the range.
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "OUT-1".into(),
            duration_s: 50,
            started_at: 5_000,
            logged_at: 5_000,
            author_account_id: Some("me".into()),
            source: "local".into(),
            ..Default::default()
        },
    )
    .unwrap();

    // started_at = 1500, but different author.
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "OTHER-1".into(),
            duration_s: 999,
            started_at: 1_500,
            logged_at: 1_500,
            author_account_id: Some("other".into()),
            source: "jira".into(),
            ..Default::default()
        },
    )
    .unwrap();

    // No author filter: should see IN-1, IN-2, and OTHER-1, ordered by
    // started_at DESC.
    let all = worklog_for_date_range(&db, 0, 4_000, None).unwrap();
    let keys: Vec<&str> = all.iter().map(|w| w.issue_key.as_str()).collect();
    assert_eq!(keys, vec!["IN-2", "OTHER-1", "IN-1"]);

    // Filter by author "me": should only see IN-2 and IN-1.
    let mine = worklog_for_date_range(&db, 0, 4_000, Some("me")).unwrap();
    let keys: Vec<&str> = mine.iter().map(|w| w.issue_key.as_str()).collect();
    assert_eq!(keys, vec!["IN-2", "IN-1"]);
}

#[test]
fn total_seconds_for_range_sums_correctly() {
    let (_d, db) = fresh_db();
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "A".into(),
            duration_s: 100,
            started_at: 1_000,
            logged_at: 1_000,
            author_account_id: Some("me".into()),
            ..Default::default()
        },
    )
    .unwrap();
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "B".into(),
            duration_s: 250,
            started_at: 2_000,
            logged_at: 2_000,
            author_account_id: Some("me".into()),
            ..Default::default()
        },
    )
    .unwrap();
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "OTHER".into(),
            duration_s: 7_000,
            started_at: 2_000,
            logged_at: 2_000,
            author_account_id: Some("other".into()),
            ..Default::default()
        },
    )
    .unwrap();
    // Out of range.
    worklog_record(
        &db,
        &WorklogRow {
            issue_key: "C".into(),
            duration_s: 50,
            started_at: 9_999,
            logged_at: 9_999,
            author_account_id: Some("me".into()),
            ..Default::default()
        },
    )
    .unwrap();

    let total_me = total_seconds_for_range(&db, 0, 5_000, Some("me")).unwrap();
    assert_eq!(total_me, 350);

    let total_all = total_seconds_for_range(&db, 0, 5_000, None).unwrap();
    assert_eq!(total_all, 7_350);

    // Empty range.
    let none = total_seconds_for_range(&db, 100_000, 200_000, None).unwrap();
    assert_eq!(none, 0);
}
