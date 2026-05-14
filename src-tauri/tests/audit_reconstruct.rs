//! Integration tests for the Phase 16 reconstruction helpers
//! (`jira::reconstruct::*`). Wiremock backs Jira; sqlite-on-disk backs the
//! cache. We seed audit entries directly via `cache::audit::record` and then
//! drive the helpers, asserting both the Jira side-effects and the audit
//! linkage rows that get written back.

use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::{
    get_by_id as audit_get_by_id, list as audit_list, record as audit_record, AuditEvent, AuditOp,
};
use tracker_lib::cache::worklogs::{
    get_by_jira_id, mark_tombstoned_by_jira_id, upsert_from_jira, WorklogRow,
};
use tracker_lib::cache::Db;
use tracker_lib::jira::reconstruct::{
    restore_deleted_worklog, retry_failed_audit_action, revert_worklog_update, ReconstructError,
};
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
    let db = Db::open(&dir.path().join("audit.db")).unwrap();
    (dir, db)
}

fn sample_row(jira_id: &str, issue_key: &str) -> WorklogRow {
    WorklogRow {
        id: None,
        issue_key: issue_key.to_string(),
        issue_id: Some("10001".into()),
        summary: Some("A summary".into()),
        duration_s: 1800,
        started_at: 1_700_000_000,
        logged_at: 1_700_000_000,
        comment: Some("Original comment".into()),
        jira_worklog_id: Some(jira_id.to_string()),
        author_account_id: Some("me-acc".into()),
        source: "jira".into(),
        updated_at_jira: Some(1_700_000_000),
        pending_delete_at: None,
        tombstoned_at: None,
    }
}

/// Seed a `delete` audit entry with the supplied before-row snapshot.
fn seed_delete_audit(db: &Db, before: &WorklogRow) -> i64 {
    audit_record(
        db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Delete,
            issue_key: Some(before.issue_key.as_str()),
            worklog_id: before.jira_worklog_id.as_deref(),
            before: Some(before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap()
}

// -----------------------------------------------------------------------------
// restore_deleted_worklog
// -----------------------------------------------------------------------------

#[tokio::test]
async fn restore_deleted_worklog_recreates_from_before_json_via_jira_post() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let before = sample_row("5001", "DEV-792");
    let audit_id = seed_delete_audit(&db, &before);

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 1800
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "9999",
            "issueId": "10001",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    let saved = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .expect("restore ok");
    assert_eq!(saved.issue_key, "DEV-792");
    assert_eq!(saved.duration_s, 1800);
    assert_eq!(saved.jira_worklog_id.as_deref(), Some("9999"));
    assert_eq!(saved.comment.as_deref(), Some("Original comment"));

    // Local cache should have the new row keyed by the new Jira id.
    let cached = get_by_jira_id(&db, "9999").unwrap().expect("cached");
    assert_eq!(cached.issue_key, "DEV-792");
}

#[tokio::test]
async fn restore_records_linked_audit_event() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let before = sample_row("5001", "DEV-792");
    let audit_id = seed_delete_audit(&db, &before);

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "9999",
            "issueId": "10001",
            "timeSpentSeconds": 1800
        })))
        .mount(&server)
        .await;

    restore_deleted_worklog(&client, &db, audit_id)
        .await
        .expect("restore ok");

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    // Top entry should be the restore (newest).
    let restore_entry = entries
        .iter()
        .find(|e| e.op == "restore")
        .expect("restore audit row");
    assert!(restore_entry.success);
    assert_eq!(restore_entry.source_audit_id, Some(audit_id));
    assert_eq!(restore_entry.worklog_id.as_deref(), Some("9999"));
}

#[tokio::test]
async fn restore_works_for_sync_tombstone_entries_too() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let before = sample_row("5001", "DEV-792");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::SyncTombstone,
            issue_key: Some("DEV-792"),
            worklog_id: Some("5001"),
            before: Some(&before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "8888",
            "issueId": "10001",
            "timeSpentSeconds": 1800
        })))
        .mount(&server)
        .await;

    let saved = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .expect("restore ok");
    assert_eq!(saved.jira_worklog_id.as_deref(), Some("8888"));
}

#[tokio::test]
async fn restore_rejects_non_delete_audit_entry() {
    let (_s, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let row = sample_row("5001", "K-1");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Create,
            issue_key: Some("K-1"),
            worklog_id: Some("5001"),
            before: None,
            after: Some(&row),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let err = restore_deleted_worklog(&client, &db, audit_id).await.unwrap_err();
    assert!(matches!(err, ReconstructError::WrongOp), "got {err:?}");
}

#[tokio::test]
async fn restore_records_failure_audit_when_jira_returns_500() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let before = sample_row("5001", "DEV-792");
    let audit_id = seed_delete_audit(&db, &before);

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = restore_deleted_worklog(&client, &db, audit_id).await.unwrap_err();
    assert!(matches!(err, ReconstructError::Jira(_)), "got {err:?}");

    // The failure must have been audited with source_audit_id linkage.
    let failed_entries: Vec<_> = audit_list(&db, 50, None, None, true)
        .unwrap()
        .into_iter()
        .filter(|e| e.op == "restore")
        .collect();
    assert_eq!(failed_entries.len(), 1);
    assert_eq!(failed_entries[0].source_audit_id, Some(audit_id));
}

// -----------------------------------------------------------------------------
// revert_worklog_update
// -----------------------------------------------------------------------------

#[tokio::test]
async fn revert_worklog_update_pushes_before_back_to_jira() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    // Seed the cache row representing the current (post-update) state.
    let mut current = sample_row("5001", "DEV-792");
    current.duration_s = 5400;
    current.comment = Some("Edited comment".into());
    upsert_from_jira(&db, &current).unwrap();

    // Seed an `update` audit entry whose before_json is the original snapshot.
    let before = sample_row("5001", "DEV-792"); // duration 1800, "Original comment"
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Update,
            issue_key: Some("DEV-792"),
            worklog_id: Some("5001"),
            before: Some(&before),
            after: Some(&current),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    // PUT should be issued with the *before* values.
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-792/worklog/5001"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 1800
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "5001",
            "issueId": "10001",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    let after = revert_worklog_update(&client, &db, audit_id).await.expect("revert ok");
    assert_eq!(after.duration_s, 1800);
    assert_eq!(after.comment.as_deref(), Some("Original comment"));

    // A `revert` audit entry should be present with source linkage.
    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let revert_entry = entries.iter().find(|e| e.op == "revert").expect("revert row");
    assert!(revert_entry.success);
    assert_eq!(revert_entry.source_audit_id, Some(audit_id));
}

#[tokio::test]
async fn revert_emits_error_when_worklog_missing_in_jira() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    // Row exists locally but Jira returns 404 (deleted upstream).
    let current = sample_row("5001", "DEV-792");
    upsert_from_jira(&db, &current).unwrap();

    let before = sample_row("5001", "DEV-792");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Update,
            issue_key: Some("DEV-792"),
            worklog_id: Some("5001"),
            before: Some(&before),
            after: Some(&current),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-792/worklog/5001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = revert_worklog_update(&client, &db, audit_id).await.unwrap_err();
    assert!(matches!(err, ReconstructError::WorklogGone), "got {err:?}");
}

#[tokio::test]
async fn revert_errors_when_local_row_is_tombstoned() {
    let (_s, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    // Row exists but is tombstoned — i.e. deleted.
    let row = sample_row("5001", "DEV-792");
    upsert_from_jira(&db, &row).unwrap();
    mark_tombstoned_by_jira_id(&db, "5001", Utc::now().timestamp()).unwrap();

    let before = sample_row("5001", "DEV-792");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Update,
            issue_key: Some("DEV-792"),
            worklog_id: Some("5001"),
            before: Some(&before),
            after: Some(&row),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let err = revert_worklog_update(&client, &db, audit_id).await.unwrap_err();
    assert!(matches!(err, ReconstructError::WorklogGone), "got {err:?}");
}

// -----------------------------------------------------------------------------
// retry_failed_audit_action
// -----------------------------------------------------------------------------

#[tokio::test]
async fn retry_failed_create_uses_after_json_args() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let intended = sample_row("(pending)", "DEV-792");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Create,
            issue_key: Some("DEV-792"),
            worklog_id: None,
            before: None,
            after: Some(&intended),
            success: false,
            error: Some("401 Unauthorized"),
            source_audit_id: None,
        },
    )
    .unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .and(body_partial_json(json!({ "timeSpentSeconds": 1800 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "7777",
            "issueId": "10001",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = retry_failed_audit_action(&client, &db, audit_id)
        .await
        .expect("retry ok");
    assert_eq!(result.get("op").and_then(|v| v.as_str()), Some("create"));
    assert_eq!(result.get("worklog_id").and_then(|v| v.as_str()), Some("7777"));

    // Linked retry audit entry.
    let retry_entry = audit_get_by_id(&db, audit_id + 1).unwrap().expect("retry row");
    assert_eq!(retry_entry.op, "retry");
    assert_eq!(retry_entry.source_audit_id, Some(audit_id));
    assert!(retry_entry.success);
}

#[tokio::test]
async fn retry_rejects_successful_audit_entry() {
    let (_s, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    let row = sample_row("5001", "K-1");
    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Create,
            issue_key: Some("K-1"),
            worklog_id: Some("5001"),
            before: None,
            after: Some(&row),
            success: true, // <-- not failed
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let err = retry_failed_audit_action(&client, &db, audit_id).await.unwrap_err();
    assert!(matches!(err, ReconstructError::AuditUnsuccessful(_)), "got {err:?}");
}

#[tokio::test]
async fn retry_failed_delete_marks_local_tombstoned() {
    let (server, client) = server_and_client().await;
    let (_d, db) = fresh_db();

    // Seed a local row that still exists (the prior delete failed).
    let row = sample_row("5001", "DEV-792");
    upsert_from_jira(&db, &row).unwrap();

    let audit_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Delete,
            issue_key: Some("DEV-792"),
            worklog_id: Some("5001"),
            before: Some(&row),
            after: None,
            success: false,
            error: Some("network"),
            source_audit_id: None,
        },
    )
    .unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/DEV-792/worklog/5001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    retry_failed_audit_action(&client, &db, audit_id).await.expect("retry ok");

    let cached = get_by_jira_id(&db, "5001").unwrap().expect("row present");
    assert!(cached.tombstoned_at.is_some());
}
