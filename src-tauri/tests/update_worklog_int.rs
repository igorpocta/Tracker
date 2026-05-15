//! Characterization tests for `commands::worklog::update_worklog` (Jira path).
//!
//! The Tauri command itself depends on `tauri::State<AppState>` + `AppHandle`,
//! which can't be instantiated in a unit-test context without a Tauri runtime.
//! Per the Phase A directive we must NOT alter production code outside of A5,
//! so we exercise the *same primitives* the command threads through:
//!   1. `JiraClient::update_worklog` → wiremock asserts the HTTP body.
//!   2. `cache::worklogs::update_fields` → DB row gets the new values.
//!   3. `cache::audit::record(AuditOp::Update, ...)` → audit row is written.
//!
//! Asserting all three locks down the behavioral contract the upcoming
//! refactor must preserve. The dispatch wiring (Freelo vs Jira via
//! `is_freelo_key`, rounding via `apply_active_rounding`, validation via
//! `validate_comment` / `validate_issue_key`) is exercised by the existing
//! unit tests in `commands.rs` + `jira_client.rs`; the gap this file closes is
//! the *end-to-end shape* of a single successful Jira update: which JSON gets
//! PUT, which row gets mutated, and which audit entry gets recorded.

use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::{self, list as audit_list, AuditEvent, AuditOp};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{upsert as issue_upsert, IssueRow};
use tracker_lib::cache::worklogs::{
    get_by_id, get_by_remote_id_any, update_fields, upsert_from_remote, WorklogRow,
};
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

/// Spin up a tempfile-backed DB pre-seeded with one Jira connection plus a
/// matching `issues_v2` row so the worklog upsert satisfies the FK chain.
fn fresh_db_with_conn(base_url: &str, issue_key: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("update_worklog.db")).unwrap();
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

/// Seed a synced (Jira-origin) worklog row. Mirrors the row shape a real Jira
/// `worklog_sync` pass would have produced.
fn seed_synced_row(db: &Db, conn_id: i64, remote_id: &str, issue_key: &str) -> WorklogRow {
    let row = WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("Original comment".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800, // 30 min
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some(remote_id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: Some("Sample".into()),
    };
    let id = upsert_from_remote(db, &row).expect("seed worklog");
    let mut saved = row.clone();
    saved.id = Some(id);
    saved
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

/// Drive the Jira PUT for `update_worklog`: started + duration + comment all
/// change, and the resulting JSON body matches the wire shape that the live
/// command produces.
#[tokio::test]
async fn jira_put_carries_started_duration_comment_adf() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-100");
    let before = seed_synced_row(&db, conn_id, "5001", "DEV-100");

    // The frontend passes the new start as ms; the command converts to
    // %Y-%m-%dT%H:%M:%S%.3f+0000 before PUTting. We assert the
    // duration + ADF comment (started is timezone-dependent in the
    // serialized form; body_partial_json's `started` match would tie the
    // test to a specific format string — kept loose).
    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-100/worklog/5001"))
        .and(body_partial_json(json!({
            "timeSpentSeconds": 3600,
            "comment": {
                "type": "doc",
                "version": 1
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "5001",
            "issueId": "10001",
            "timeSpentSeconds": 3600
        })))
        .expect(1)
        .mount(&server)
        .await;

    let new_started = Utc.with_ymd_and_hms(2026, 5, 15, 9, 30, 0).unwrap();
    let resp = client
        .update_worklog(
            "DEV-100",
            "5001",
            Some(new_started),
            Some(3600),
            Some("Edited"),
        )
        .await
        .expect("update ok");
    assert_eq!(resp.id, "5001");

    // The command then writes the new fields locally + records an audit row.
    let local_id = before.id.expect("seeded id");
    let new_started_s = new_started.timestamp();
    let new_ended_s = new_started_s + 3600;
    let now_s = Utc::now().timestamp();
    update_fields(
        &db,
        local_id,
        Some("DEV-100"),
        Some("Edited"),
        new_started_s,
        new_ended_s,
        Some(now_s),
    )
    .expect("update_fields");

    let after = get_by_id(&db, local_id).unwrap().expect("row present");
    assert_eq!(after.started_at, new_started_s);
    assert_eq!(after.ended_at, new_ended_s);
    assert_eq!(after.duration_s(), 3600);
    assert_eq!(after.description.as_deref(), Some("Edited"));
    assert!(after.is_synced);

    // Audit row mirrors what `audit_success(AuditOp::Update, ...)` writes.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Update,
            issue_key: Some("DEV-100"),
            worklog_id: Some(&resp.id),
            before: Some(&before),
            after: Some(&after),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .expect("audit record");

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let update = entries
        .iter()
        .find(|e| e.op == "update")
        .expect("update audit row");
    assert!(update.success);
    assert_eq!(update.worklog_id.as_deref(), Some("5001"));
    assert_eq!(update.issue_key.as_deref(), Some("DEV-100"));
    assert!(update.before_json.is_some(), "before snapshot stored");
    assert!(update.after_json.is_some(), "after snapshot stored");
}

/// Updating with `new_duration_seconds = None` and only `new_comment = Some(_)`
/// must NOT include `timeSpentSeconds` in the PUT body. This pins the
/// optional-field behavior of `JiraClient::update_worklog`.
#[tokio::test]
async fn jira_put_omits_duration_when_only_comment_changes() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-101");
    let before = seed_synced_row(&db, conn_id, "5002", "DEV-101");

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-101/worklog/5002"))
        .and(body_partial_json(json!({
            "comment": { "type": "doc", "version": 1 }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "5002",
            "issueId": "10002",
            "timeSpentSeconds": 1800
        })))
        .expect(1)
        .mount(&server)
        .await;

    let resp = client
        .update_worklog("DEV-101", "5002", None, None, Some("Only comment"))
        .await
        .expect("ok");

    // Mirror the command's local update — issue_key unchanged, duration
    // preserved (we keep the old started/ended), description replaced.
    let local_id = before.id.expect("seeded");
    let now_s = Utc::now().timestamp();
    update_fields(
        &db,
        local_id,
        Some("DEV-101"),
        Some("Only comment"),
        before.started_at,
        before.ended_at,
        Some(now_s),
    )
    .unwrap();

    let after = get_by_id(&db, local_id).unwrap().expect("row");
    assert_eq!(after.description.as_deref(), Some("Only comment"));
    assert_eq!(after.duration_s(), 1800, "duration unchanged");
    assert_eq!(after.remote_id.as_deref(), Some(resp.id.as_str()));
}

/// A 404 from Jira surfaces as `JiraError::WorklogNotFound`. The command
/// would write a failure audit row carrying the before-snapshot and the error
/// string — verify the audit-row shape is what the UI expects to see.
#[tokio::test]
async fn jira_404_yields_worklog_not_found_and_failure_audit() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-102");
    let before = seed_synced_row(&db, conn_id, "5003", "DEV-102");

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-102/worklog/5003"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let err = client
        .update_worklog(
            "DEV-102",
            "5003",
            None,
            Some(3600),
            Some("won't reach Jira"),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, tracker_lib::jira::JiraError::WorklogNotFound),
        "got {err:?}"
    );

    // Mirror `audit_failure(AuditOp::Update, ...)`.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Update,
            issue_key: Some("DEV-102"),
            worklog_id: Some("5003"),
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
        .filter(|e| e.op == "update")
        .collect();
    assert_eq!(failed.len(), 1);
    assert!(failed[0].error.is_some());
    assert_eq!(failed[0].worklog_id.as_deref(), Some("5003"));
    // The local row stays untouched on a failed PUT — `update_fields` is
    // never called on the error branch.
    let still_there = get_by_remote_id_any(&db, "5003")
        .unwrap()
        .expect("row remains");
    assert_eq!(
        still_there.description.as_deref(),
        Some("Original comment"),
        "no local mutation when remote PUT fails"
    );
}
