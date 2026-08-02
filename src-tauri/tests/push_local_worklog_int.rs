//! Skipped on Windows: these three targets are the only ones that build a
//! Tauri app (`tauri::test::mock_app`), which drags the webview2 imports into
//! the test binary. The Windows loader then refuses to start it with
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) — the process dies before a
//! single test runs, so nothing here is actually failing.
//!
//! What they cover is platform-neutral (HTTP handlers, push, dedup) and the
//! macOS leg runs all of it. The Windows leg exists to compile the Win32 code
//! in `focus/apps.rs` and friends, and it still does that.
#![cfg(not(windows))]

//! Regression test for concurrent retry paths pushing the same local worklog.
//!
//! Startup flush, periodic auto-sync, the manual "Synchronizovat" action and
//! the local HTTP bridge can all target the same unsynced row. The push path
//! is serialised through `AppState.worklog_push_lock` so a single row cannot
//! be created twice upstream.

use std::time::Duration;

use serde_json::json;
use tauri::Manager;
use tempfile::TempDir;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{upsert as issue_upsert, IssueRow};
use tracker_lib::cache::worklogs::{get_by_id, record, WorklogRow};
use tracker_lib::cache::Db;
use tracker_lib::commands::worklog::crud::push_local_worklog_inner;
use tracker_lib::jira::JiraClient;
use tracker_lib::state::{ActiveConnection, AppState, ProviderClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret-token";

#[tokio::test]
async fn concurrent_pushes_of_same_row_post_only_once() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-1/worklog"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_delay(Duration::from_millis(50))
                .set_body_json(json!({
                    "id": "7001",
                    "issueId": "10001",
                    "timeSpentSeconds": 900
                })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let app = tauri::test::mock_app();
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("push.db")).unwrap();
    let cfg = format!(r#"{{"base_url":"{}","email":"{}"}}"#, server.uri(), EMAIL);
    let conn_id = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "tenant-a",
            enabled: true,
            config_json: &cfg,
        },
    )
    .unwrap();
    issue_upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: "DEV-1".into(),
            issue_key: "DEV-1".into(),
            name: "Sample".into(),
            ..Default::default()
        },
    )
    .unwrap();

    let state = AppState::new(db, dir.path().to_path_buf());
    {
        let mut conns = state.connections.write().unwrap();
        conns.push(ActiveConnection {
            id: conn_id,
            kind: "jira".into(),
            name: "tenant-a".into(),
            enabled: true,
            client: ProviderClient::Jira(
                JiraClient::new(server.uri(), EMAIL.into(), TOKEN.into()).unwrap(),
            ),
        });
    }
    app.handle().manage(state);

    let app_state = app.state::<AppState>();
    let local_id = record(
        &app_state.db,
        &WorklogRow {
            id: None,
            connection_id: Some(conn_id),
            issue_key: Some("DEV-1".into()),
            description: Some("retry me".into()),
            started_at: 1_700_000_000,
            ended_at: 1_700_000_000 + 900,
            logged_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            is_synced: false,
            synced_at: None,
            remote_id: None,
            pending_delete_at: None,
            tombstoned_at: None,
            summary: None,
        },
    )
    .unwrap();

    let fut_a = push_local_worklog_inner(app.handle(), &app_state, local_id);
    let fut_b = push_local_worklog_inner(app.handle(), &app_state, local_id);
    let (res_a, res_b) = tokio::join!(fut_a, fut_b);

    let outcomes = [res_a, res_b];
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(outcomes.iter().filter(|r| r.is_err()).count(), 1);
    let err = outcomes
        .iter()
        .find_map(|r| r.as_ref().err())
        .expect("one call should be rejected after the first sync");
    assert!(
        err.contains("již synchronizovaný"),
        "unexpected error: {err}"
    );

    let after = get_by_id(&app_state.db, local_id).unwrap().unwrap();
    assert!(after.is_synced);
    assert_eq!(after.remote_id.as_deref(), Some("7001"));
}
