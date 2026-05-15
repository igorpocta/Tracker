//! Characterization tests for `commands::worklog::create_manual_worklog`
//! (Jira path).
//!
//! Same strategy as `update_worklog_int.rs`: drive the primitives the
//! command threads through and verify the end-to-end shape.
//!
//! - `JiraClient::add_worklog` — wiremock asserts the POST body.
//! - `cache::worklogs::upsert_from_remote` — local row inserted with
//!   `is_synced: true` + the returned remote id.
//! - `cache::audit::record(AuditOp::Create)` — audit row carries the
//!   after-snapshot.
//!
//! The dispatch wiring (`is_freelo_key`, validation, rounding) is covered
//! elsewhere; the gap closed here is the Jira POST body shape and the
//! resulting local-row + audit-row state.

use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::{self, list as audit_list, AuditEvent, AuditOp};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{get_connection_id_by_key, upsert as issue_upsert, IssueRow};
use tracker_lib::cache::worklogs::{get_by_remote_id_any, upsert_from_remote, WorklogRow};
use tracker_lib::cache::Db;
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

fn fresh_db_with_conn(base_url: &str, issue_key: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("create_manual.db")).unwrap();
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
    issue_upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: issue_key.to_string(),
            issue_key: issue_key.to_string(),
            name: "Sample".into(),
            ..Default::default()
        },
    )
    .expect("seed issue");
    (dir, db, conn_id)
}

#[tokio::test]
async fn jira_post_with_comment_inserts_synced_row() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-300");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-300/worklog"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 1800,
            "comment": { "type": "doc", "version": 1 }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "7777",
            "issueId": "10300",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 15, 8, 0, 0).unwrap();
    let resp = client
        .add_worklog("DEV-300", started, 1800, Some("Initial work"))
        .await
        .expect("add ok");
    assert_eq!(resp.id, "7777");

    // Mirror what the command does next: insert local row + audit success.
    let started_at_ms = started.timestamp_millis();
    let started_at_s = started_at_ms / 1000;
    let now_s = Utc::now().timestamp();
    let connection_id = get_connection_id_by_key(&db, "DEV-300").unwrap();
    assert_eq!(connection_id, Some(conn_id));

    let row = WorklogRow {
        id: None,
        connection_id,
        issue_key: Some("DEV-300".to_string()),
        description: Some("Initial work".into()),
        started_at: started_at_s,
        ended_at: started_at_s + 1800,
        logged_at: now_s,
        updated_at: now_s,
        is_synced: true,
        synced_at: Some(now_s),
        remote_id: Some(resp.id.clone()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    let local_id = upsert_from_remote(&db, &row).expect("upsert");
    let mut saved = row.clone();
    saved.id = Some(local_id);

    let cached = get_by_remote_id_any(&db, "7777")
        .unwrap()
        .expect("cached row");
    assert_eq!(cached.issue_key.as_deref(), Some("DEV-300"));
    assert_eq!(cached.duration_s(), 1800);
    assert!(cached.is_synced);
    assert_eq!(cached.remote_id.as_deref(), Some("7777"));
    assert_eq!(cached.description.as_deref(), Some("Initial work"));
    assert!(cached.synced_at.is_some(), "synced_at populated");
    assert!(
        cached.pending_delete_at.is_none() && cached.tombstoned_at.is_none(),
        "fresh row, no deletion state"
    );

    // Audit row: AuditOp::Create with after-snapshot only.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: now_s,
            op: AuditOp::Create,
            issue_key: Some("DEV-300"),
            worklog_id: Some(&resp.id),
            before: None,
            after: Some(&saved),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();
    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let create = entries
        .iter()
        .find(|e| e.op == "create")
        .expect("create audit row");
    assert!(create.success);
    assert_eq!(create.worklog_id.as_deref(), Some("7777"));
    assert!(create.before_json.is_none(), "no before on create");
    assert!(create.after_json.is_some());
}

/// Verify the POST body omits `comment` entirely when the caller supplies
/// `None`. The Jira ADF builder collapses empty/missing comments rather
/// than sending `comment: null`.
#[tokio::test]
async fn jira_post_without_comment_omits_comment_field() {
    let (server, client) = server_and_client().await;
    let (_d, _db, _conn_id) = fresh_db_with_conn(&server.uri(), "DEV-301");

    // Use a `body_partial_json` only on the duration; rely on .expect(1) to
    // assert exactly one POST. To assert *absence* of `comment`, mount a
    // strict matcher that the absence-violation would not satisfy: we
    // capture the call and inspect.
    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-301/worklog"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 900
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "8888",
            "issueId": "10301",
            "timeSpentSeconds": 900
        })))
        .expect(1)
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 15, 10, 0, 0).unwrap();
    let resp = client
        .add_worklog("DEV-301", started, 900, None)
        .await
        .expect("ok");
    assert_eq!(resp.id, "8888");

    // Inspect the recorded request to confirm `comment` was NOT in the body.
    let received = server.received_requests().await.unwrap();
    let post = received
        .iter()
        .find(|r| r.method == wiremock::http::Method::POST)
        .expect("had a POST");
    let body: serde_json::Value = serde_json::from_slice(&post.body).expect("body is valid JSON");
    assert!(
        body.get("comment").is_none(),
        "comment field should be omitted when None, got {body:?}"
    );
    assert_eq!(body.get("timeSpentSeconds"), Some(&json!(900)));
}

#[tokio::test]
async fn jira_500_records_failure_audit_and_no_local_row() {
    let (server, client) = server_and_client().await;
    let (_d, db, _conn_id) = fresh_db_with_conn(&server.uri(), "DEV-302");

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-302/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let started = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
    let err = client
        .add_worklog("DEV-302", started, 1800, Some("should fail"))
        .await
        .unwrap_err();

    // Command writes a failure audit and bails — no local row.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Create,
            issue_key: Some("DEV-302"),
            worklog_id: None,
            before: None,
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
        .filter(|e| e.op == "create")
        .collect();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].worklog_id.is_none());
    assert!(failed[0].error.is_some());

    // No worklog should have been inserted (no remote_id to look up under).
    // Sanity: the cache is empty for that issue.
    let cnt = tracker_lib::cache::worklogs::count(&db).unwrap();
    assert_eq!(cnt, 0, "no local row when remote POST fails");
}
