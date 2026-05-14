//! Tests for the Tauri-free *inner* helpers behind each command.
//!
//! These never start a Tauri runtime; they exercise pure functions over an
//! in-memory SQLite database. Anything that requires a `tauri::AppHandle`
//! (event emission, window navigation) lives outside the inner helpers and is
//! verified by the full app at runtime.

use tempfile::TempDir;

use tracker_lib::cache::issues::{
    recent as issues_recent, suggested as issues_suggested, upsert as issue_upsert, IssueRow,
};
use tracker_lib::cache::timer::{self, ActiveTimer};
use tracker_lib::cache::worklogs::{recent as worklog_recent, WorklogRow};
use tracker_lib::cache::Db;
use tracker_lib::commands::prefs::{
    get_daily_goal_inner, set_app_icon_inner, set_daily_goal_inner, set_widget_format_inner,
    DEFAULT_DAILY_GOAL_SECONDS,
};
use tracker_lib::commands::timer::{
    get_timer_state_inner, record_local_stop, start_timer_inner, update_timer_start_inner,
};

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    (dir, db)
}

fn issue(key: &str, summary: &str, updated_at: i64) -> IssueRow {
    IssueRow {
        issue_key: key.into(),
        summary: summary.into(),
        updated_at,
        ..Default::default()
    }
}

// -----------------------------------------------------------------------------
// Timer commands.
// -----------------------------------------------------------------------------

#[test]
fn timer_state_is_none_when_no_active_timer() {
    let (_dir, db) = fresh_db();
    assert!(get_timer_state_inner(&db, 1_000).unwrap().is_none());
}

#[test]
fn start_then_get_returns_running_timer_with_elapsed() {
    let (_dir, db) = fresh_db();

    // Start at t=100_000 ms (100 s).
    let state = start_timer_inner(&db, "ACME-1", 100_000).unwrap();
    assert_eq!(state.issue_key, "ACME-1");
    assert_eq!(state.started_at, 100_000);
    assert_eq!(state.elapsed_seconds, 0);

    // At t=200_000 ms we have 100 seconds elapsed.
    let snap = get_timer_state_inner(&db, 200_000).unwrap().unwrap();
    assert_eq!(snap.issue_key, "ACME-1");
    assert_eq!(snap.started_at, 100_000);
    assert_eq!(snap.elapsed_seconds, 100);
}

#[test]
fn start_timer_replaces_previous_row() {
    let (_dir, db) = fresh_db();
    start_timer_inner(&db, "ACME-1", 1_000).unwrap();
    start_timer_inner(&db, "ACME-2", 5_000).unwrap();
    let t = timer::get(&db).unwrap().unwrap();
    assert_eq!(t.issue_key, "ACME-2");
    assert_eq!(t.started_at, 5);
}

#[test]
fn update_timer_start_changes_the_started_at_in_place() {
    let (_dir, db) = fresh_db();
    start_timer_inner(&db, "ACME-1", 1_000).unwrap();
    let updated = update_timer_start_inner(&db, 500, 1_500).unwrap();
    assert_eq!(updated.issue_key, "ACME-1");
    assert_eq!(updated.started_at, 0);
    assert_eq!(updated.elapsed_seconds, 1);
}

#[test]
fn update_timer_start_errors_when_no_timer_running() {
    let (_dir, db) = fresh_db();
    let err = update_timer_start_inner(&db, 0, 0).unwrap_err();
    assert!(err.contains("no active timer"));
}

#[test]
fn record_local_stop_writes_worklog_and_clears_timer() {
    let (_dir, db) = fresh_db();
    issue_upsert(&db, &issue("ACME-1", "fix the bug", 0)).unwrap();
    start_timer_inner(&db, "ACME-1", 0).unwrap();
    let timer_state = ActiveTimer {
        issue_key: "ACME-1".into(),
        started_at: 0,
    };

    let row = record_local_stop(&db, &timer_state, 60_000, Some("done"), Some("J-1")).unwrap();
    assert!(row.id.is_some());
    assert_eq!(row.duration_s, 60);
    assert_eq!(row.issue_key, "ACME-1");
    assert_eq!(row.summary.as_deref(), Some("fix the bug"));
    assert_eq!(row.jira_worklog_id.as_deref(), Some("J-1"));
    assert_eq!(row.comment.as_deref(), Some("done"));

    // Timer should be cleared.
    assert!(timer::get(&db).unwrap().is_none());

    // The row should be visible in the recent_worklogs query.
    let recent: Vec<WorklogRow> = worklog_recent(&db, 10).unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].issue_key, "ACME-1");
}

#[test]
fn record_local_stop_clamps_negative_duration_to_zero() {
    let (_dir, db) = fresh_db();
    let timer_state = ActiveTimer {
        issue_key: "ACME-1".into(),
        started_at: 100,
    };
    // now < started_at → duration would be negative; we expect 0.
    let row = record_local_stop(&db, &timer_state, 0, None, None).unwrap();
    assert_eq!(row.duration_s, 0);
}

// -----------------------------------------------------------------------------
// Prefs commands.
// -----------------------------------------------------------------------------

#[test]
fn daily_goal_defaults_to_eight_hours_when_unset() {
    let (_dir, db) = fresh_db();
    let v = get_daily_goal_inner(&db).unwrap();
    assert_eq!(v, DEFAULT_DAILY_GOAL_SECONDS);
    assert_eq!(v, 8 * 60 * 60);
}

#[test]
fn set_and_get_daily_goal_roundtrip() {
    let (_dir, db) = fresh_db();
    set_daily_goal_inner(&db, 6 * 60 * 60).unwrap();
    assert_eq!(get_daily_goal_inner(&db).unwrap(), 6 * 60 * 60);
}

#[test]
fn set_daily_goal_rejects_negative_values() {
    let (_dir, db) = fresh_db();
    let err = set_daily_goal_inner(&db, -1).unwrap_err();
    assert!(err.contains("non-negative"));
}

#[test]
fn set_widget_format_and_app_icon_persist() {
    let (_dir, db) = fresh_db();
    set_widget_format_inner(&db, "hh:mm:ss").unwrap();
    set_app_icon_inner(&db, "dark").unwrap();
    // No public getter for these yet; verify via raw settings table.
    let v = tracker_lib::cache::settings::get(&db, "widget_format")
        .unwrap()
        .unwrap();
    assert_eq!(v, "hh:mm:ss");
    let v = tracker_lib::cache::settings::get(&db, "app_icon")
        .unwrap()
        .unwrap();
    assert_eq!(v, "dark");
}

// -----------------------------------------------------------------------------
// Issues recent / suggested helpers.
// -----------------------------------------------------------------------------

#[test]
fn recent_issues_orders_by_updated_at_desc() {
    let (_dir, db) = fresh_db();
    issue_upsert(&db, &issue("A-1", "old", 100)).unwrap();
    issue_upsert(&db, &issue("A-2", "new", 500)).unwrap();
    issue_upsert(&db, &issue("A-3", "mid", 300)).unwrap();

    let rows = issues_recent(&db, 10).unwrap();
    let keys: Vec<&str> = rows.iter().map(|r| r.issue_key.as_str()).collect();
    assert_eq!(keys, vec!["A-2", "A-3", "A-1"]);
}

#[test]
fn suggested_issues_returns_only_issues_with_worklogs() {
    let (_dir, db) = fresh_db();
    issue_upsert(&db, &issue("A-1", "with worklog", 100)).unwrap();
    issue_upsert(&db, &issue("A-2", "no worklog", 200)).unwrap();

    let timer_state = ActiveTimer {
        issue_key: "A-1".into(),
        started_at: 0,
    };
    record_local_stop(&db, &timer_state, 60_000, None, None).unwrap();

    let rows = issues_suggested(&db, 10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].issue_key, "A-1");
}
