//! Phase A6 — characterization tests for `freelo::reconstruct::*`.
//!
//! Mirrors `tests/audit_reconstruct.rs` (Jira) but against wiremock Freelo.
//! Uses the same `snapshot_json` + `insert_audit_raw` helpers because
//! `WorklogRow`'s Serialize impl emits duplicate `description`/`comment`
//! aliases that round-trip through `parse_row` would reject (same bug
//! observed in the Jira version — workaround documented in commit a346ee6).

use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::list as audit_list;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::worklogs::{
    get_by_remote_id_any, mark_tombstoned_by_remote_id, upsert_from_remote, WorklogRow,
};
use tracker_lib::cache::Db;
use tracker_lib::freelo::reconstruct::{
    restore_deleted_worklog, retry_failed_audit_action, revert_worklog_update, ReconstructError,
};
use tracker_lib::freelo::FreeloClient;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL: &str = "alice@example.com";
const KEY: &str = "freelo-api-key";

/// Hand-rolled JSON snapshot that uses only the canonical field names —
/// avoids the duplicate `description`/`comment` alias collision that the
/// production `Serialize` impl emits.
fn snapshot_json(r: &WorklogRow) -> String {
    serde_json::to_string(&serde_json::json!({
        "id": r.id,
        "connection_id": r.connection_id,
        "issue_key": r.issue_key,
        "description": r.description,
        "started_at": r.started_at,
        "ended_at": r.ended_at,
        "logged_at": r.logged_at,
        "updated_at": r.updated_at,
        "is_synced": r.is_synced,
        "synced_at": r.synced_at,
        "remote_id": r.remote_id,
        "pending_delete_at": r.pending_delete_at,
        "tombstoned_at": r.tombstoned_at,
        "summary": r.summary,
    }))
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn insert_audit_raw(
    db: &Db,
    occurred_at: i64,
    op: &str,
    issue_key: Option<&str>,
    worklog_id: Option<&str>,
    before: Option<&WorklogRow>,
    after: Option<&WorklogRow>,
    success: bool,
    error: Option<&str>,
) -> i64 {
    let before_json = before.map(snapshot_json);
    let after_json = after.map(snapshot_json);
    let conn = db.pool().get().unwrap();
    conn.execute(
        "INSERT INTO audit_log (
            occurred_at, op, issue_key, worklog_id, before_json, after_json,
            success, error, source_audit_id
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            occurred_at,
            op,
            issue_key,
            worklog_id,
            before_json,
            after_json,
            if success { 1 } else { 0 },
            error,
            None::<i64>,
        ],
    )
    .unwrap();
    conn.last_insert_rowid()
}

async fn server_and_client() -> (MockServer, FreeloClient) {
    let server = MockServer::start().await;
    let client = FreeloClient::new(server.uri(), EMAIL.into(), KEY.into()).unwrap();
    (server, client)
}

fn seed_freelo_connection(db: &Db, base_url: &str) -> i64 {
    let cfg = format!(r#"{{"base_url":"{}","email":"{}"}}"#, base_url, EMAIL);
    insert_conn(
        db,
        NewConnection {
            provider: "freelo",
            name: "test-freelo",
            enabled: true,
            config_json: &cfg,
        },
    )
    .expect("seed freelo connection")
}

fn seed_issue(db: &Db, conn_id: i64, key: &str) {
    tracker_lib::cache::issues::upsert(
        db,
        &tracker_lib::cache::issues::IssueRow {
            connection_id: conn_id,
            issue_id: key.to_string(),
            issue_key: key.to_string(),
            name: "Freelo task".into(),
            ..Default::default()
        },
    )
    .expect("seed issue");
}

fn fresh_db_with_conn(base_url: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("freelo_audit.db")).unwrap();
    let conn_id = seed_freelo_connection(&db, base_url);
    (dir, db, conn_id)
}

fn sample_row(conn_id: i64, remote_id: &str, issue_key: &str) -> WorklogRow {
    WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("Original Freelo note".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800, // 30 min → 30 Freelo minutes
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some(remote_id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: Some("Freelo task".into()),
    }
}

// -----------------------------------------------------------------------------
// restore_deleted_worklog
// -----------------------------------------------------------------------------

#[tokio::test]
async fn freelo_restore_recreates_via_create_work_report() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let before = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "delete",
        before.issue_key.as_deref(),
        before.remote_id.as_deref(),
        Some(&before),
        None,
        true,
        None,
    );

    // Freelo's create endpoint is `POST /task/{task_id}/work-reports` and the
    // body carries `minutes` (Freelo's native time unit), `date_reported`
    // (ISO date), and an optional `note`.
    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .and(body_partial_json(json!({ "minutes": 30 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 9999,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2023-11-14",
            "user_id": 1
        })))
        .expect(1)
        .mount(&server)
        .await;

    let saved = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .expect("restore ok");
    assert_eq!(saved.issue_key.as_deref(), Some("FREELO-42"));
    assert_eq!(saved.duration_s(), 1800);
    assert_eq!(saved.remote_id.as_deref(), Some("9999"));
    assert_eq!(saved.description.as_deref(), Some("Original Freelo note"));

    // Cache row keyed by new id.
    let cached = get_by_remote_id_any(&db, "9999").unwrap().expect("cached");
    assert_eq!(cached.issue_key.as_deref(), Some("FREELO-42"));
}

#[tokio::test]
async fn freelo_restore_records_linked_audit_event() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let before = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "delete",
        Some("FREELO-42"),
        Some("9001"),
        Some(&before),
        None,
        true,
        None,
    );

    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 9999,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2023-11-14",
            "user_id": 1
        })))
        .mount(&server)
        .await;

    restore_deleted_worklog(&client, &db, audit_id)
        .await
        .expect("restore ok");

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let restore_entry = entries
        .iter()
        .find(|e| e.op == "restore")
        .expect("restore audit row");
    assert!(restore_entry.success);
    assert_eq!(restore_entry.source_audit_id, Some(audit_id));
    assert_eq!(restore_entry.worklog_id.as_deref(), Some("9999"));
}

#[tokio::test]
async fn freelo_restore_rejects_non_delete_audit_entry() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());

    let row = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("FREELO-42"),
        Some("9001"),
        None,
        Some(&row),
        true,
        None,
    );

    let err = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ReconstructError::WrongOp), "got {err:?}");
}

#[tokio::test]
async fn freelo_restore_records_failure_audit_when_500() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let before = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "delete",
        Some("FREELO-42"),
        Some("9001"),
        Some(&before),
        None,
        true,
        None,
    );

    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ReconstructError::Freelo(_)), "got {err:?}");

    let failed: Vec<_> = audit_list(&db, 50, None, None, true)
        .unwrap()
        .into_iter()
        .filter(|e| e.op == "restore")
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].source_audit_id, Some(audit_id));
}

// -----------------------------------------------------------------------------
// revert_worklog_update
// -----------------------------------------------------------------------------

#[tokio::test]
async fn freelo_revert_pushes_before_back_via_post() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    // The cache holds the post-update state.
    let mut current = sample_row(conn_id, "9001", "FREELO-42");
    current.ended_at = current.started_at + 5400; // 90 min
    current.description = Some("Edited note".into());
    upsert_from_remote(&db, &current).unwrap();

    let before = sample_row(conn_id, "9001", "FREELO-42"); // 30 min, original note
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "update",
        Some("FREELO-42"),
        Some("9001"),
        Some(&before),
        Some(&current),
        true,
        None,
    );

    // Freelo's update is `POST /work-reports/{id}` with `minutes` body.
    Mock::given(method("POST"))
        .and(path("/work-reports/9001"))
        .and(body_partial_json(json!({ "minutes": 30 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 9001,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2023-11-14",
            "user_id": 1
        })))
        .expect(1)
        .mount(&server)
        .await;

    let after = revert_worklog_update(&client, &db, audit_id)
        .await
        .expect("revert ok");
    assert_eq!(after.duration_s(), 1800);
    assert_eq!(after.description.as_deref(), Some("Original Freelo note"));

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let rev = entries
        .iter()
        .find(|e| e.op == "revert")
        .expect("revert row");
    assert!(rev.success);
    assert_eq!(rev.source_audit_id, Some(audit_id));
}

#[tokio::test]
async fn freelo_revert_emits_worklog_gone_on_404() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let current = sample_row(conn_id, "9001", "FREELO-42");
    upsert_from_remote(&db, &current).unwrap();

    let before = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "update",
        Some("FREELO-42"),
        Some("9001"),
        Some(&before),
        Some(&current),
        true,
        None,
    );

    Mock::given(method("POST"))
        .and(path("/work-reports/9001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = revert_worklog_update(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ReconstructError::WorklogGone), "got {err:?}");
}

#[tokio::test]
async fn freelo_revert_errors_when_local_row_tombstoned() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let row = sample_row(conn_id, "9001", "FREELO-42");
    upsert_from_remote(&db, &row).unwrap();
    mark_tombstoned_by_remote_id(&db, conn_id, "9001", Utc::now().timestamp()).unwrap();

    let before = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "update",
        Some("FREELO-42"),
        Some("9001"),
        Some(&before),
        Some(&row),
        true,
        None,
    );

    let err = revert_worklog_update(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ReconstructError::WorklogGone), "got {err:?}");
}

// -----------------------------------------------------------------------------
// retry_failed_audit_action
// -----------------------------------------------------------------------------

#[tokio::test]
async fn freelo_retry_failed_create_uses_after_json() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let intended = sample_row(conn_id, "(pending)", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("FREELO-42"),
        None,
        None,
        Some(&intended),
        false,
        Some("401 Unauthorized"),
    );

    Mock::given(method("POST"))
        .and(path("/task/42/work-reports"))
        .and(body_partial_json(json!({ "minutes": 30 })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 7777,
            "task_id": 42,
            "minutes": 30,
            "date_reported": "2023-11-14",
            "user_id": 1
        })))
        .expect(1)
        .mount(&server)
        .await;

    let result = retry_failed_audit_action(&client, &db, audit_id)
        .await
        .expect("retry ok");
    assert_eq!(result.get("op").and_then(|v| v.as_str()), Some("create"));
    assert_eq!(
        result.get("worklog_id").and_then(|v| v.as_str()),
        Some("7777")
    );

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let retry = entries.iter().find(|e| e.op == "retry").expect("retry row");
    assert!(retry.success);
    assert_eq!(retry.source_audit_id, Some(audit_id));
}

#[tokio::test]
async fn freelo_retry_rejects_successful_audit() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());

    let row = sample_row(conn_id, "9001", "FREELO-42");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("FREELO-42"),
        Some("9001"),
        None,
        Some(&row),
        true, // success
        None,
    );

    let err = retry_failed_audit_action(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ReconstructError::AuditUnsuccessful(_)),
        "got {err:?}"
    );
}

#[tokio::test]
async fn freelo_retry_failed_delete_tombstones_local() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "FREELO-42");

    let row = sample_row(conn_id, "9001", "FREELO-42");
    upsert_from_remote(&db, &row).unwrap();

    let audit_id = insert_audit_raw(
        &db,
        100,
        "delete",
        Some("FREELO-42"),
        Some("9001"),
        Some(&row),
        None,
        false,
        Some("network"),
    );

    Mock::given(method("DELETE"))
        .and(path("/work-reports/9001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    retry_failed_audit_action(&client, &db, audit_id)
        .await
        .expect("retry ok");

    let cached = get_by_remote_id_any(&db, "9001").unwrap().expect("present");
    assert!(
        cached.tombstoned_at.is_some(),
        "tombstoned after retry-delete"
    );
}
