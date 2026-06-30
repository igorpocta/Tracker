//! Integration tests for the composite `move_worklog` operation (Phase 15).

use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::IssueRow;
use tracker_lib::cache::worklogs::{upsert_from_remote, WorklogRow};
use tracker_lib::cache::Db;
use tracker_lib::jira::worklog_ops::{move_worklog, MoveWorklogArgs, MoveWorklogError};
use tracker_lib::jira::JiraClient;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret-token";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client =
        JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string()).expect("client builds");
    (server, client)
}

/// `issues_v2` has an FK to `connections`. Seed one Jira connection plus the
/// two issue rows the move test exercises (OLD-1 / NEW-2) so the upserts
/// downstream find their parent.
fn fresh_db(base_url: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("ops.db")).unwrap();
    let config = format!(r#"{{"base_url":"{}","email":"{}"}}"#, base_url, EMAIL);
    let conn_id = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "test",
            enabled: true,
            config_json: &config,
        },
    )
    .expect("seed connection");
    for key in ["OLD-1", "NEW-2"] {
        tracker_lib::cache::issues::upsert(
            &db,
            &IssueRow {
                connection_id: conn_id,
                issue_id: key.into(),
                issue_key: key.into(),
                name: "x".into(),
                ..Default::default()
            },
        )
        .expect("seed issue");
    }
    (dir, db, conn_id)
}

/// Seed the local cache with a synced (remote-origin) row representing the
/// old worklog. The provider-neutral schema drops `source`/`author_account_id`
/// /`pending_assignment` in favour of `is_synced` + `connection_id`.
fn seed_old_row(db: &Db, conn_id: i64, remote_id: &str, issue_key: &str) -> i64 {
    let row = WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("Old comment".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800,
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some(remote_id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: Some("Old summary".into()),
    };
    upsert_from_remote(db, &row).unwrap()
}

#[tokio::test]
async fn move_worklog_happy_path() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db(&server.uri());
    let _old_id = seed_old_row(&db, conn_id, "5001", "OLD-1");

    // POST new on NEW-2: succeed.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/NEW-2/worklog"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 1800
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "6001",
            "issueId": "20002",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    // DELETE old on OLD-1: 204.
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/OLD-1/worklog/5001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "OLD-1",
        old_worklog_id: "5001",
        new_issue_key: "NEW-2",
        started,
        time_spent_seconds: 1800,
        comment: Some("Moved"),
        fallback_connection_id: None,
    };

    let res = move_worklog(&client, &db, args).await.expect("ok");
    assert_eq!(res.new_worklog_id, "6001");
    assert_eq!(res.new_row.issue_key.as_deref(), Some("NEW-2"));

    // The new row should be in the DB.
    let by_new = tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "6001").unwrap();
    assert!(by_new.is_some());
    // The old row should be gone.
    let by_old = tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "5001").unwrap();
    assert!(by_old.is_none(), "old row should have been hard-deleted");
}

#[tokio::test]
async fn move_worklog_uncached_new_issue_uses_fallback_connection() {
    // Regression: moving to an issue key not yet in issues_v2 left the new row
    // with connection_id = None, which upsert_from_remote rejects — so after
    // the upstream POST+DELETE both succeeded, the move errored and the old
    // local row was never deleted (orphan/duplicate). The fallback connection
    // (the old row's) must be used.
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db(&server.uri());
    let _old_id = seed_old_row(&db, conn_id, "5001", "OLD-1");
    // NOTE: "UNCACHED-9" is deliberately NOT seeded into issues_v2.

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/UNCACHED-9/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "6002",
            "issueId": "20009",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/OLD-1/worklog/5001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "OLD-1",
        old_worklog_id: "5001",
        new_issue_key: "UNCACHED-9",
        started,
        time_spent_seconds: 1800,
        comment: Some("Moved"),
        fallback_connection_id: Some(conn_id),
    };

    let res = move_worklog(&client, &db, args)
        .await
        .expect("move should succeed despite the new issue being uncached");
    assert_eq!(res.new_worklog_id, "6002");
    assert_eq!(res.new_row.connection_id, Some(conn_id));
    assert!(
        tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "6002")
            .unwrap()
            .is_some(),
        "new row persisted via fallback connection"
    );
    assert!(
        tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "5001")
            .unwrap()
            .is_none(),
        "old row deleted"
    );
}

#[tokio::test]
async fn move_worklog_create_failed_leaves_old_intact() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db(&server.uri());
    let _old_id = seed_old_row(&db, conn_id, "5001", "OLD-1");

    // POST new on NEW-2: 500.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/NEW-2/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&server)
        .await;

    // No DELETE should be issued — wiremock will fail the test if one is.

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "OLD-1",
        old_worklog_id: "5001",
        new_issue_key: "NEW-2",
        started,
        time_spent_seconds: 1800,
        comment: None,
        fallback_connection_id: None,
    };

    let err = move_worklog(&client, &db, args).await.unwrap_err();
    assert!(
        matches!(err, MoveWorklogError::CreateFailed(_)),
        "got {err:?}"
    );

    // Old row should still be in the DB.
    let by_old = tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "5001")
        .unwrap()
        .expect("old row should remain");
    assert_eq!(by_old.issue_key.as_deref(), Some("OLD-1"));
}

#[tokio::test]
async fn move_worklog_delete_failed_returns_new_id_for_recovery() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db(&server.uri());
    let _old_id = seed_old_row(&db, conn_id, "5001", "OLD-1");

    // POST new on NEW-2: succeed.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/NEW-2/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "6001",
            "issueId": "20002",
            "timeSpentSeconds": 900
        })))
        .expect(1)
        .mount(&server)
        .await;

    // DELETE old on OLD-1: 500 (network glitch).
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/OLD-1/worklog/5001"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "OLD-1",
        old_worklog_id: "5001",
        new_issue_key: "NEW-2",
        started,
        time_spent_seconds: 900,
        comment: None,
        fallback_connection_id: None,
    };

    let err = move_worklog(&client, &db, args).await.unwrap_err();
    match err {
        MoveWorklogError::DeleteAfterCreate {
            new_worklog_id,
            old_issue_key,
            ..
        } => {
            assert_eq!(new_worklog_id, "6001");
            assert_eq!(old_issue_key, "OLD-1");
        }
        other => panic!("unexpected error: {other:?}"),
    }

    // Both rows should now exist: the new (inserted) AND the old (untouched).
    assert!(
        tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "5001")
            .unwrap()
            .is_some(),
        "old row preserved"
    );
    assert!(
        tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "6001")
            .unwrap()
            .is_some(),
        "new row inserted"
    );
}

#[tokio::test]
async fn move_worklog_delete_404_is_treated_as_success() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db(&server.uri());
    let _old_id = seed_old_row(&db, conn_id, "5001", "OLD-1");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/NEW-2/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "6001"
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/OLD-1/worklog/5001"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "OLD-1",
        old_worklog_id: "5001",
        new_issue_key: "NEW-2",
        started,
        time_spent_seconds: 900,
        comment: None,
        fallback_connection_id: None,
    };

    let res = move_worklog(&client, &db, args).await.expect("ok");
    assert_eq!(res.new_worklog_id, "6001");
    // Old row should be removed locally too (404 means "already gone").
    assert!(
        tracker_lib::cache::worklogs::get_by_remote_id_any(&db, "5001")
            .unwrap()
            .is_none()
    );
}
