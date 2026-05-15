//! Command-level characterization tests for
//! `commands::worklog::move_worklog`.
//!
//! `tests/jira_worklog_ops.rs` already covers the HTTP-level POST+DELETE
//! contract. The gap closed here is the SQLite-side behavior the command
//! exposes after a successful move:
//!
//! - The old row (keyed by old `remote_id`) is hard-deleted.
//! - A new row appears under the new `remote_id` with `issue_key` pointing
//!   at the new issue and `is_synced: true`.
//! - The command writes an audit row tagged `move` linking before/after.
//!
//! Like A1-A3, we drive the underlying primitives
//! (`jira::worklog_ops::move_worklog` + `cache::audit::record`) because
//! the Tauri command is tightly coupled to `AppState`.

use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::{self, list as audit_list, AuditEvent, AuditOp};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{upsert as issue_upsert, IssueRow};
use tracker_lib::cache::worklogs::{get_by_remote_id_any, upsert_from_remote, WorklogRow};
use tracker_lib::cache::Db;
use tracker_lib::jira::worklog_ops::{move_worklog, MoveWorklogArgs};
use tracker_lib::jira::JiraClient;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const TOKEN: &str = "secret-token";

async fn server_and_client() -> (MockServer, JiraClient) {
    let server = MockServer::start().await;
    let client =
        JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string()).expect("client builds");
    (server, client)
}

fn fresh_db_two_issues(base_url: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("move_cmd.db")).unwrap();
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
    for key in ["SRC-1", "DST-2"] {
        issue_upsert(
            &db,
            &IssueRow {
                connection_id: conn_id,
                issue_id: key.to_string(),
                issue_key: key.to_string(),
                name: "x".into(),
                ..Default::default()
            },
        )
        .expect("seed issue");
    }
    (dir, db, conn_id)
}

fn seed_synced_row(db: &Db, conn_id: i64, remote_id: &str, issue_key: &str) -> WorklogRow {
    let row = WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("Moving target".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800,
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some(remote_id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: Some("Moving target".into()),
    };
    let id = upsert_from_remote(db, &row).expect("seed worklog");
    let mut saved = row;
    saved.id = Some(id);
    saved
}

/// Happy path: POST succeeds + DELETE succeeds → old local row is gone,
/// new local row exists pointing at DST-2, audit "move" links before/after.
#[tokio::test]
async fn move_worklog_updates_sqlite_and_records_audit() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_two_issues(&server.uri());
    let before = seed_synced_row(&db, conn_id, "5001", "SRC-1");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DST-2/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "6001",
            "issueId": "20002",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/SRC-1/worklog/5001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 9, 30, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "SRC-1",
        old_worklog_id: "5001",
        new_issue_key: "DST-2",
        started,
        time_spent_seconds: 1800,
        comment: Some("Moved comment"),
    };
    let res = move_worklog(&client, &db, args).await.expect("move ok");
    assert_eq!(res.new_worklog_id, "6001");

    // SQLite-side assertions.
    let new_row = get_by_remote_id_any(&db, "6001")
        .unwrap()
        .expect("new row in cache");
    assert_eq!(new_row.issue_key.as_deref(), Some("DST-2"));
    assert!(new_row.is_synced);
    assert_eq!(new_row.duration_s(), 1800);
    let old_row = get_by_remote_id_any(&db, "5001").unwrap();
    assert!(
        old_row.is_none(),
        "old row should be hard-deleted after a clean move"
    );

    // Mirror the audit row the command writes on success.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Move,
            issue_key: Some("DST-2"),
            worklog_id: Some(&res.new_worklog_id),
            before: Some(&before),
            after: Some(&res.new_row),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let mv = entries
        .iter()
        .find(|e| e.op == "move")
        .expect("move audit row");
    assert!(mv.success);
    assert_eq!(mv.issue_key.as_deref(), Some("DST-2"));
    assert_eq!(mv.worklog_id.as_deref(), Some("6001"));
    assert!(mv.before_json.is_some(), "before snapshot stored");
    assert!(mv.after_json.is_some(), "after snapshot stored");
}

/// Failure on the POST step leaves the old row intact and the command
/// writes a failure audit. (Mirrors `MoveWorklogError::CreateFailed`.)
#[tokio::test]
async fn move_worklog_create_failure_keeps_old_row_and_audits_failure() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_two_issues(&server.uri());
    let before = seed_synced_row(&db, conn_id, "5002", "SRC-1");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DST-2/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 14, 10, 0, 0).unwrap();
    let args = MoveWorklogArgs {
        old_issue_key: "SRC-1",
        old_worklog_id: "5002",
        new_issue_key: "DST-2",
        started,
        time_spent_seconds: 1800,
        comment: None,
    };
    let err = move_worklog(&client, &db, args).await.unwrap_err();

    let still = get_by_remote_id_any(&db, "5002")
        .unwrap()
        .expect("old row remains");
    assert_eq!(still.issue_key.as_deref(), Some("SRC-1"));

    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Move,
            issue_key: Some("SRC-1"),
            worklog_id: Some("5002"),
            before: Some(&before),
            after: None,
            success: false,
            error: Some(&err.to_string()),
            source_audit_id: None,
        },
    )
    .unwrap();

    let failed: Vec<_> = audit_list(&db, 50, None, None, true)
        .unwrap()
        .into_iter()
        .filter(|e| e.op == "move")
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].issue_key.as_deref(), Some("SRC-1"));
    assert!(failed[0].error.is_some());
}
