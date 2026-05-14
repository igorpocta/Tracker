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
    get_daily_goal_inner, get_density_inner, get_font_size_inner, get_hourly_rate_inner,
    get_theme_inner, set_app_icon_inner, set_daily_goal_inner, set_density_inner,
    set_font_size_inner, set_hourly_rate_inner, set_theme_inner, set_widget_format_inner,
    DEFAULT_DAILY_GOAL_SECONDS, DEFAULT_DENSITY, DEFAULT_FONT_SIZE, DEFAULT_HOURLY_RATE,
    DEFAULT_THEME,
};
use tracker_lib::commands::config::{
    sign_out_inner, test_jira_connection_inner, update_config_inner,
};
use tracker_lib::config::JiraConfig;
use tracker_lib::state::AppState;
use tracker_lib::commands::timer::{
    get_timer_state_inner, record_local_stop, start_timer_inner, update_timer_start_inner,
};
use tracker_lib::jira::JiraError;

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

    let row = record_local_stop(&db, &timer_state, 60_000, Some("done"), Some("J-1"), None).unwrap();
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
    let row = record_local_stop(&db, &timer_state, 0, None, None, None).unwrap();
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
fn hourly_rate_defaults_to_zero_when_unset() {
    let (_dir, db) = fresh_db();
    let v = get_hourly_rate_inner(&db).unwrap();
    assert_eq!(v, DEFAULT_HOURLY_RATE);
    assert_eq!(v, 0.0);
}

#[test]
fn set_and_get_hourly_rate_roundtrip() {
    let (_dir, db) = fresh_db();
    set_hourly_rate_inner(&db, 1500.0).unwrap();
    assert_eq!(get_hourly_rate_inner(&db).unwrap(), 1500.0);
}

#[test]
fn set_hourly_rate_rejects_negative_values() {
    let (_dir, db) = fresh_db();
    let err = set_hourly_rate_inner(&db, -1.0).unwrap_err();
    assert!(err.contains("non-negative"));
}

#[test]
fn set_hourly_rate_rejects_non_finite_values() {
    let (_dir, db) = fresh_db();
    let err = set_hourly_rate_inner(&db, f64::NAN).unwrap_err();
    assert!(err.contains("finite"));
}

#[test]
fn theme_defaults_to_auto_when_unset() {
    let (_dir, db) = fresh_db();
    assert_eq!(get_theme_inner(&db).unwrap(), DEFAULT_THEME);
    assert_eq!(get_theme_inner(&db).unwrap(), "auto");
}

#[test]
fn set_and_get_theme_roundtrip() {
    let (_dir, db) = fresh_db();
    set_theme_inner(&db, "dark").unwrap();
    assert_eq!(get_theme_inner(&db).unwrap(), "dark");
    set_theme_inner(&db, "light").unwrap();
    assert_eq!(get_theme_inner(&db).unwrap(), "light");
}

#[test]
fn set_theme_rejects_invalid_values() {
    let (_dir, db) = fresh_db();
    let err = set_theme_inner(&db, "rainbow").unwrap_err();
    assert!(err.contains("invalid theme"), "got: {err}");
}

#[test]
fn font_size_defaults_to_md() {
    let (_dir, db) = fresh_db();
    assert_eq!(get_font_size_inner(&db).unwrap(), DEFAULT_FONT_SIZE);
    assert_eq!(get_font_size_inner(&db).unwrap(), "md");
}

#[test]
fn set_and_get_font_size_roundtrip() {
    let (_dir, db) = fresh_db();
    set_font_size_inner(&db, "lg").unwrap();
    assert_eq!(get_font_size_inner(&db).unwrap(), "lg");
}

#[test]
fn set_font_size_rejects_invalid_values() {
    let (_dir, db) = fresh_db();
    let err = set_font_size_inner(&db, "xxl").unwrap_err();
    assert!(err.contains("invalid font_size"), "got: {err}");
}

#[test]
fn density_defaults_to_comfortable() {
    let (_dir, db) = fresh_db();
    assert_eq!(get_density_inner(&db).unwrap(), DEFAULT_DENSITY);
    assert_eq!(get_density_inner(&db).unwrap(), "comfortable");
}

#[test]
fn set_and_get_density_roundtrip() {
    let (_dir, db) = fresh_db();
    set_density_inner(&db, "compact").unwrap();
    assert_eq!(get_density_inner(&db).unwrap(), "compact");
}

#[test]
fn set_density_rejects_invalid_values() {
    let (_dir, db) = fresh_db();
    let err = set_density_inner(&db, "spacious").unwrap_err();
    assert!(err.contains("invalid density"), "got: {err}");
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
    record_local_stop(&db, &timer_state, 60_000, None, None, None).unwrap();

    let rows = issues_suggested(&db, 10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].issue_key, "A-1");
}

// -----------------------------------------------------------------------------
// Config — test_jira_connection inner helper (uses wiremock).
// -----------------------------------------------------------------------------

#[tokio::test]
async fn test_jira_connection_inner_returns_user_on_success() {
    use serde_json::json;
    use wiremock::matchers::{basic_auth, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .and(basic_auth("alice@example.com", "secret-token"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": "abc123",
            "displayName": "Alice Example",
            "emailAddress": "alice@example.com",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let user = test_jira_connection_inner(&server.uri(), "alice@example.com", "secret-token")
        .await
        .expect("ok");

    assert_eq!(user.account_id, "abc123");
    assert_eq!(user.display_name, "Alice Example");
    assert_eq!(user.email_address.as_deref(), Some("alice@example.com"));
}

#[tokio::test]
async fn test_jira_connection_inner_returns_error_on_401() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;

    let err = test_jira_connection_inner(&server.uri(), "alice@example.com", "bad-token")
        .await
        .unwrap_err();
    assert!(matches!(err, JiraError::Unauthorized), "got {err:?}");
}

#[tokio::test]
async fn test_jira_connection_inner_rejects_bogus_url() {
    let err = test_jira_connection_inner("not a url", "a@b.c", "t")
        .await
        .unwrap_err();
    assert!(matches!(err, JiraError::InvalidUrl(_)), "got {err:?}");
}

// -----------------------------------------------------------------------------
// Phase 11A — update_config / sign_out inner helpers.
//
// These tests stub out the keychain via the closure parameter, so they don't
// require the OS keychain and run anywhere.
// -----------------------------------------------------------------------------

fn fresh_state() -> (TempDir, AppState) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let app_data_dir = dir.path().to_path_buf();
    (dir, AppState::new(db, app_data_dir))
}

#[test]
fn update_config_inner_writes_toml_and_updates_state() {
    let (dir, state) = fresh_state();
    let cfg_path = dir.path().join("config.toml");

    let cfg = JiraConfig {
        base_url: "https://acme.atlassian.net".into(),
        email: "alice@acme.example".into(),
    };

    // Token closure: just record that we got it; no keychain access.
    let saved = std::cell::Cell::new(None);
    update_config_inner(&state, &cfg_path, cfg.clone(), Some("new-token".into()), |t| {
        saved.set(Some(t.to_string()));
        Ok(())
    })
    .unwrap();

    assert!(cfg_path.exists(), "config file should be created");
    assert_eq!(saved.into_inner().as_deref(), Some("new-token"));

    let in_memory = state.jira_config_cloned().expect("config in memory");
    assert_eq!(in_memory.base_url, cfg.base_url);
    assert_eq!(in_memory.email, cfg.email);

    // The TOML on disk should round-trip.
    let loaded = tracker_lib::config::load_from_path(&cfg_path).unwrap();
    assert_eq!(loaded.base_url, cfg.base_url);
}

#[test]
fn update_config_inner_does_not_save_token_when_none() {
    let (dir, state) = fresh_state();
    let cfg_path = dir.path().join("config.toml");

    let cfg = JiraConfig {
        base_url: "https://acme.atlassian.net".into(),
        email: "alice@acme.example".into(),
    };
    let called = std::cell::Cell::new(false);
    update_config_inner(&state, &cfg_path, cfg, None, |_t| {
        called.set(true);
        Ok(())
    })
    .unwrap();
    assert!(!called.into_inner(), "save_token closure must not run when new_token = None");
}

#[test]
fn sign_out_inner_removes_config_and_clears_state() {
    let (dir, state) = fresh_state();
    let cfg_path = dir.path().join("config.toml");

    // Seed: write a config + populate the in-memory state so we can verify
    // both halves get cleared.
    let cfg = JiraConfig {
        base_url: "https://acme.atlassian.net".into(),
        email: "alice@acme.example".into(),
    };
    tracker_lib::config::save_to_path(&cfg_path, &cfg).unwrap();
    *state.jira_config.write().unwrap() = Some(cfg);

    let cleared = std::cell::Cell::new(false);
    sign_out_inner(&state, &cfg_path, || {
        cleared.set(true);
        Ok(())
    })
    .unwrap();

    assert!(!cfg_path.exists(), "config file should be deleted");
    assert!(cleared.into_inner(), "clear_token closure must run");
    assert!(state.jira_config_cloned().is_none());
    assert!(state.jira_client_cloned().is_none());
}

#[test]
fn sign_out_inner_is_idempotent_when_already_signed_out() {
    let (dir, state) = fresh_state();
    let cfg_path = dir.path().join("missing.toml");
    // File never existed.
    sign_out_inner(&state, &cfg_path, || Ok(())).expect("idempotent");
    assert!(state.jira_config_cloned().is_none());
}
