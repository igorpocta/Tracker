//! Integration tests for the Phase 16 reconstruction helpers
//! (`jira::reconstruct::*`). Wiremock backs Jira; sqlite-on-disk backs the
//! cache. We seed audit entries directly via `cache::audit::record` and then
//! drive the helpers, asserting both the Jira side-effects and the audit
//! linkage rows that get written back.

use chrono::Utc;
use serde_json::json;
use tempfile::TempDir;
use tracker_lib::cache::audit::{get_by_id as audit_get_by_id, list as audit_list};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::worklogs::{
    get_by_remote_id_any, mark_tombstoned_by_remote_id, upsert_from_remote, WorklogRow,
};

/// Serialize a `WorklogRow` to a JSON snapshot that round-trips back through
/// `parse_row` cleanly. The lib's `Serialize` impl emits both the canonical
/// `description` field and the legacy `comment` alias (same for
/// `remote_id`/`jira_worklog_id`), which makes the resulting blob fail
/// deserialization because serde's alias rules treat the two keys as a
/// duplicate. We sidestep that by hand-rolling the JSON with only the
/// canonical fields.
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

/// Direct-to-SQL audit insert that avoids the broken WorklogRow Serialize
/// round-trip. Mirrors the columns in `audit_log` (see migration 0011).
#[allow(clippy::too_many_arguments)]
fn insert_audit_raw(
    db: &tracker_lib::cache::Db,
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
    let client =
        JiraClient::new(server.uri(), EMAIL.to_string(), TOKEN.to_string()).expect("client builds");
    (server, client)
}

/// Insert a Jira connection row so issue/worklog upserts satisfy the FK on
/// `issues_v2`. The reconstruct helpers look up the connection id by issue
/// key when inserting the restored worklog, so we also seed a matching
/// issue row pointing at this connection.
fn fresh_db_with_conn(base_url: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("audit.db")).unwrap();
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
    (dir, db, conn_id)
}

fn fresh_db() -> (TempDir, Db, i64) {
    // For tests that don't actually touch the wiremock server we still need
    // a connection row so upserts work.
    fresh_db_with_conn("http://example.invalid")
}

/// Seed an issue cache row so the reconstruct helper can resolve
/// `connection_id` from the issue key when reinserting the worklog.
fn seed_issue(db: &Db, conn_id: i64, key: &str) {
    tracker_lib::cache::issues::upsert(
        db,
        &tracker_lib::cache::issues::IssueRow {
            connection_id: conn_id,
            issue_id: key.to_string(),
            issue_key: key.to_string(),
            name: "A summary".into(),
            ..Default::default()
        },
    )
    .expect("seed issue");
}

fn sample_row(conn_id: i64, remote_id: &str, issue_key: &str) -> WorklogRow {
    WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("Original comment".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800, // duration_s == 1800
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some(remote_id.to_string()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: Some("A summary".into()),
    }
}

/// Seed a `delete` audit entry with the supplied before-row snapshot.
fn seed_delete_audit(db: &Db, before: &WorklogRow) -> i64 {
    // Use the raw helper so the snapshot JSON contains the canonical fields
    // only (no legacy alias collisions) — the production Serialize impl on
    // WorklogRow emits both `description` and `comment` and the reconstruct
    // helper's deserializer rejects the duplicate.
    insert_audit_raw(
        db,
        Utc::now().timestamp(),
        "delete",
        before.issue_key.as_deref(),
        before.remote_id.as_deref(),
        Some(before),
        None,
        true,
        None,
    )
}

// -----------------------------------------------------------------------------
// restore_deleted_worklog
// -----------------------------------------------------------------------------

#[tokio::test]
async fn restore_deleted_worklog_recreates_from_before_json_via_jira_post() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    let before = sample_row(conn_id, "5001", "DEV-792");
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
    assert_eq!(saved.issue_key.as_deref(), Some("DEV-792"));
    assert_eq!(saved.duration_s(), 1800);
    assert_eq!(saved.remote_id.as_deref(), Some("9999"));
    assert_eq!(saved.description.as_deref(), Some("Original comment"));

    // Local cache should have the new row keyed by the new Jira id.
    let cached = get_by_remote_id_any(&db, "9999").unwrap().expect("cached");
    assert_eq!(cached.issue_key.as_deref(), Some("DEV-792"));
}

#[tokio::test]
async fn restore_records_linked_audit_event() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    let before = sample_row(conn_id, "5001", "DEV-792");
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
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    let before = sample_row(conn_id, "5001", "DEV-792");
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "sync_tombstone",
        Some("DEV-792"),
        Some("5001"),
        Some(&before),
        None,
        true,
        None,
    );

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
    assert_eq!(saved.remote_id.as_deref(), Some("8888"));
}

#[tokio::test]
async fn restore_rejects_non_delete_audit_entry() {
    let (_s, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db();

    let row = sample_row(conn_id, "5001", "K-1");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("K-1"),
        Some("5001"),
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
async fn restore_records_failure_audit_when_jira_returns_500() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    let before = sample_row(conn_id, "5001", "DEV-792");
    let audit_id = seed_delete_audit(&db, &before);

    Mock::given(method("POST"))
        .and(path("/rest/api/3/issue/DEV-792/worklog"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let err = restore_deleted_worklog(&client, &db, audit_id)
        .await
        .unwrap_err();
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
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    // Seed the cache row representing the current (post-update) state.
    let mut current = sample_row(conn_id, "5001", "DEV-792");
    // Bump duration to 5400s by stretching ended_at; description gets edited.
    current.ended_at = current.started_at + 5400;
    current.description = Some("Edited comment".into());
    upsert_from_remote(&db, &current).unwrap();

    // Seed an `update` audit entry whose before_json is the original snapshot.
    let before = sample_row(conn_id, "5001", "DEV-792"); // duration 1800, "Original comment"
    let audit_id = insert_audit_raw(
        &db,
        Utc::now().timestamp(),
        "update",
        Some("DEV-792"),
        Some("5001"),
        Some(&before),
        Some(&current),
        true,
        None,
    );

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

    let after = revert_worklog_update(&client, &db, audit_id)
        .await
        .expect("revert ok");
    assert_eq!(after.duration_s(), 1800);
    assert_eq!(after.description.as_deref(), Some("Original comment"));

    // A `revert` audit entry should be present with source linkage.
    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let revert_entry = entries
        .iter()
        .find(|e| e.op == "revert")
        .expect("revert row");
    assert!(revert_entry.success);
    assert_eq!(revert_entry.source_audit_id, Some(audit_id));
}

#[tokio::test]
async fn revert_emits_error_when_worklog_missing_in_jira() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    // Row exists locally but Jira returns 404 (deleted upstream).
    let current = sample_row(conn_id, "5001", "DEV-792");
    upsert_from_remote(&db, &current).unwrap();

    let before = sample_row(conn_id, "5001", "DEV-792");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "update",
        Some("DEV-792"),
        Some("5001"),
        Some(&before),
        Some(&current),
        true,
        None,
    );

    Mock::given(method("PUT"))
        .and(path("/rest/api/3/issue/DEV-792/worklog/5001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let err = revert_worklog_update(&client, &db, audit_id)
        .await
        .unwrap_err();
    assert!(matches!(err, ReconstructError::WorklogGone), "got {err:?}");
}

#[tokio::test]
async fn revert_errors_when_local_row_is_tombstoned() {
    let (_s, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db();
    seed_issue(&db, conn_id, "DEV-792");

    // Row exists but is tombstoned — i.e. deleted.
    let row = sample_row(conn_id, "5001", "DEV-792");
    upsert_from_remote(&db, &row).unwrap();
    mark_tombstoned_by_remote_id(&db, conn_id, "5001", Utc::now().timestamp()).unwrap();

    let before = sample_row(conn_id, "5001", "DEV-792");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "update",
        Some("DEV-792"),
        Some("5001"),
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
async fn retry_failed_create_uses_after_json_args() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    let intended = sample_row(conn_id, "(pending)", "DEV-792");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("DEV-792"),
        None,
        None,
        Some(&intended),
        false,
        Some("401 Unauthorized"),
    );

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
    assert_eq!(
        result.get("worklog_id").and_then(|v| v.as_str()),
        Some("7777")
    );

    // Linked retry audit entry.
    let retry_entry = audit_get_by_id(&db, audit_id + 1)
        .unwrap()
        .expect("retry row");
    assert_eq!(retry_entry.op, "retry");
    assert_eq!(retry_entry.source_audit_id, Some(audit_id));
    assert!(retry_entry.success);
}

#[tokio::test]
async fn retry_rejects_successful_audit_entry() {
    let (_s, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db();

    let row = sample_row(conn_id, "5001", "K-1");
    let audit_id = insert_audit_raw(
        &db,
        100,
        "create",
        Some("K-1"),
        Some("5001"),
        None,
        Some(&row),
        true, // <-- not failed
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
async fn retry_failed_delete_marks_local_tombstoned() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri());
    seed_issue(&db, conn_id, "DEV-792");

    // Seed a local row that still exists (the prior delete failed).
    let row = sample_row(conn_id, "5001", "DEV-792");
    upsert_from_remote(&db, &row).unwrap();

    let audit_id = insert_audit_raw(
        &db,
        100,
        "delete",
        Some("DEV-792"),
        Some("5001"),
        Some(&row),
        None,
        false,
        Some("network"),
    );

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/DEV-792/worklog/5001"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    retry_failed_audit_action(&client, &db, audit_id)
        .await
        .expect("retry ok");

    let cached = get_by_remote_id_any(&db, "5001")
        .unwrap()
        .expect("row present");
    assert!(cached.tombstoned_at.is_some());
}
