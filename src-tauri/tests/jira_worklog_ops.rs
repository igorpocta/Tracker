//! Integration tests for the composite `move_worklog` operation (Phase 15).

use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::worklogs::{upsert_from_jira, WorklogRow};
use tracker_lib::cache::Db;
use tracker_lib::jira::worklog_ops::{move_worklog, MoveWorklogArgs, MoveWorklogError};
use tracker_lib::jira::JiraClient;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret-token";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client = JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string())
        .expect("client builds");
    (server, client)
}

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("ops.db")).unwrap();
    (dir, db)
}

/// Seed the local cache with a "jira"-source row representing the old worklog.
fn seed_old_row(db: &Db, jira_id: &str, issue_key: &str) -> i64 {
    let row = WorklogRow {
        id: None,
        issue_key: issue_key.to_string(),
        issue_id: Some("10001".into()),
        summary: Some("Old summary".into()),
        duration_s: 1800,
        started_at: 1_700_000_000,
        logged_at: 1_700_000_000,
        comment: Some("Old comment".into()),
        jira_worklog_id: Some(jira_id.to_string()),
        author_account_id: Some("me-acc".into()),
        source: "jira".to_string(),
        updated_at_jira: Some(1_700_000_000),
        pending_delete_at: None,
        tombstoned_at: None,
    };
    upsert_from_jira(db, &row).unwrap()
}

#[tokio::test]
async fn move_worklog_happy_path() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();
    let _old_id = seed_old_row(&db, "5001", "OLD-1");

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
        author_account_id: Some("me-acc"),
    };

    let res = move_worklog(&client, &db, args).await.expect("ok");
    assert_eq!(res.new_worklog_id, "6001");
    assert_eq!(res.new_row.issue_key, "NEW-2");

    // The new row should be in the DB.
    let by_new =
        tracker_lib::cache::worklogs::get_by_jira_id(&db, "6001").unwrap();
    assert!(by_new.is_some());
    // The old row should be gone.
    let by_old =
        tracker_lib::cache::worklogs::get_by_jira_id(&db, "5001").unwrap();
    assert!(by_old.is_none(), "old row should have been hard-deleted");
}

#[tokio::test]
async fn move_worklog_create_failed_leaves_old_intact() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();
    let _old_id = seed_old_row(&db, "5001", "OLD-1");

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
        author_account_id: Some("me-acc"),
    };

    let err = move_worklog(&client, &db, args).await.unwrap_err();
    assert!(matches!(err, MoveWorklogError::CreateFailed(_)), "got {err:?}");

    // Old row should still be in the DB.
    let by_old = tracker_lib::cache::worklogs::get_by_jira_id(&db, "5001")
        .unwrap()
        .expect("old row should remain");
    assert_eq!(by_old.issue_key, "OLD-1");
}

#[tokio::test]
async fn move_worklog_delete_failed_returns_new_id_for_recovery() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();
    let _old_id = seed_old_row(&db, "5001", "OLD-1");

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
        author_account_id: Some("me-acc"),
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
        tracker_lib::cache::worklogs::get_by_jira_id(&db, "5001")
            .unwrap()
            .is_some(),
        "old row preserved"
    );
    assert!(
        tracker_lib::cache::worklogs::get_by_jira_id(&db, "6001")
            .unwrap()
            .is_some(),
        "new row inserted"
    );
}

#[tokio::test]
async fn move_worklog_delete_404_is_treated_as_success() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();
    let _old_id = seed_old_row(&db, "5001", "OLD-1");

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
        author_account_id: Some("me-acc"),
    };

    let res = move_worklog(&client, &db, args).await.expect("ok");
    assert_eq!(res.new_worklog_id, "6001");
    // Old row should be removed locally too (404 means "already gone").
    assert!(
        tracker_lib::cache::worklogs::get_by_jira_id(&db, "5001")
            .unwrap()
            .is_none()
    );
}
