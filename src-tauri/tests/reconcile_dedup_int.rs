//! P1-4: pre-POST reconciliation must adopt an already-created upstream
//! worklog instead of POSTing a duplicate.
//!
//! Scenario from the review: a prior attempt POSTed successfully (HTTP 201)
//! but the subsequent local DB write failed, so the row is still unsynced
//! (`remote_id IS NULL`). On retry the push path queries the provider, finds
//! the matching worklog and adopts its id — it must NOT create a second one.

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
const ACCOUNT_ID: &str = "acc-123";

// 1_700_000_000 == 2023-11-14T22:13:20Z.
const STARTED_AT_S: i64 = 1_700_000_000;
const DURATION_S: i64 = 900;

#[tokio::test]
async fn push_adopts_existing_remote_worklog_instead_of_posting_again() {
    let server = MockServer::start().await;

    // Who am I — needed so reconciliation matches on the authoring user.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "accountId": ACCOUNT_ID,
            "displayName": "Alice",
        })))
        .mount(&server)
        .await;

    // The issue already has a worklog from the prior (partially failed) attempt:
    // same author, same start instant, same duration.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/DEV-1/worklog"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "worklogs": [{
                "id": "7001",
                "author": { "accountId": ACCOUNT_ID },
                "started": "2023-11-14T22:13:20.000+0000",
                "timeSpentSeconds": DURATION_S,
            }],
            "total": 1,
            "startAt": 0,
            "maxResults": 1000,
        })))
        .mount(&server)
        .await;

    // The POST must NEVER fire — adopting the existing worklog is the whole point.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-1/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "9999",
            "issueId": "10001",
            "timeSpentSeconds": DURATION_S,
        })))
        .expect(0)
        .mount(&server)
        .await;

    let app = tauri::test::mock_app();
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("reconcile.db")).unwrap();
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
            started_at: STARTED_AT_S,
            ended_at: STARTED_AT_S + DURATION_S,
            logged_at: STARTED_AT_S,
            updated_at: STARTED_AT_S,
            is_synced: false,
            synced_at: None,
            remote_id: None,
            pending_delete_at: None,
            tombstoned_at: None,
            summary: None,
        },
    )
    .unwrap();

    let saved = push_local_worklog_inner(app.handle(), &app_state, local_id)
        .await
        .expect("push should adopt the existing remote worklog");

    // Adopted the existing remote id, not the (never-sent) POST id "9999".
    assert_eq!(saved.remote_id.as_deref(), Some("7001"));

    let after = get_by_id(&app_state.db, local_id).unwrap().unwrap();
    assert!(after.is_synced);
    assert_eq!(after.remote_id.as_deref(), Some("7001"));
    // `expect(0)` on the POST mock is verified when `server` drops.
}
