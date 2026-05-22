//! Characterization tests for the soft-delete lifecycle:
//! `commands::worklog::delete_worklog` + `undo_delete_worklog`.
//!
//! Same approach as `tests/update_worklog_int.rs`: the Tauri commands are
//! tightly bound to `AppState` + `AppHandle`, so we drive the *primitives*
//! they call (`cache::worklogs::mark_pending_delete`, `clear_pending_delete`,
//! `mark_tombstoned`, `cache::audit::record(AuditOp::Delete | Undo)`,
//! `JiraClient::delete_worklog`) and verify the observable lifecycle:
//!
//!   1. `delete_worklog` marks `pending_delete_at = now` and writes a
//!      success audit row tagged `delete`.
//!   2. Within the undo window, `undo_delete_worklog` clears the flag and
//!      writes a separate audit row tagged `undo` — the worklog row is
//!      otherwise untouched (not tombstoned).
//!   3. Without an undo, the background `commit_pending_delete` task issues
//!      a Jira DELETE and tombstones the row.
//!
//! We don't sleep — the production code's `tokio::time::sleep(UNDO_WINDOW_MS)`
//! is just a delay between (1) and (3); the *behavior* in (3) is what we
//! characterize.

use chrono::Utc;
use tempfile::TempDir;
use tracker_lib::cache::audit::{self, list as audit_list, AuditEvent, AuditOp};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{upsert as issue_upsert, IssueRow};
use tracker_lib::cache::worklogs::{
    clear_pending_delete, get_by_id, get_pending_delete_by_remote_id_any, mark_pending_delete,
    mark_tombstoned, upsert_from_remote, WorklogRow,
};
use tracker_lib::cache::Db;
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

fn fresh_db_with_conn(base_url: &str, issue_key: &str) -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("lifecycle.db")).unwrap();
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

fn seed_synced_row(db: &Db, conn_id: i64, remote_id: &str, issue_key: &str) -> WorklogRow {
    let row = WorklogRow {
        id: None,
        connection_id: Some(conn_id),
        issue_key: Some(issue_key.to_string()),
        description: Some("To be deleted".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_000 + 1800,
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
    let mut saved = row;
    saved.id = Some(id);
    saved
}

#[tokio::test]
async fn delete_worklog_sets_pending_delete_and_records_audit() {
    let (_s, _c) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn("http://example.invalid", "DEV-200");
    let before = seed_synced_row(&db, conn_id, "6001", "DEV-200");
    let local_id = before.id.expect("seeded id");

    let now_s = Utc::now().timestamp();
    mark_pending_delete(&db, local_id, now_s).expect("mark pending");

    let after = get_by_id(&db, local_id).unwrap().expect("row");
    assert!(
        after.pending_delete_at.is_some(),
        "pending_delete_at should be set"
    );
    assert!(after.tombstoned_at.is_none(), "not yet tombstoned");

    // Mirror `audit_success(AuditOp::Delete, ...)` from the command.
    audit::record(
        &db,
        AuditEvent {
            occurred_at: now_s,
            op: AuditOp::Delete,
            issue_key: Some("DEV-200"),
            worklog_id: Some("6001"),
            before: Some(&before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    let del = entries
        .iter()
        .find(|e| e.op == "delete")
        .expect("delete audit row");
    assert!(del.success);
    assert_eq!(del.worklog_id.as_deref(), Some("6001"));
    assert!(del.before_json.is_some(), "before snapshot stored");
    assert!(del.after_json.is_none(), "no after on a delete");
}

#[tokio::test]
async fn undo_delete_worklog_clears_pending_and_audits_undo() {
    let (_s, _c) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn("http://example.invalid", "DEV-201");
    let before = seed_synced_row(&db, conn_id, "6002", "DEV-201");
    let local_id = before.id.expect("seeded");

    let now_s = Utc::now().timestamp();
    mark_pending_delete(&db, local_id, now_s).unwrap();
    // Audit the delete first (the command does this before the undo).
    audit::record(
        &db,
        AuditEvent {
            occurred_at: now_s,
            op: AuditOp::Delete,
            issue_key: Some("DEV-201"),
            worklog_id: Some("6002"),
            before: Some(&before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    // Now the user undoes within the window.
    clear_pending_delete(&db, local_id).expect("clear pending");
    audit::record(
        &db,
        AuditEvent {
            occurred_at: Utc::now().timestamp(),
            op: AuditOp::Undo,
            issue_key: before.issue_key.as_deref(),
            worklog_id: Some("6002"),
            before: Some(&before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let after = get_by_id(&db, local_id)
        .unwrap()
        .expect("row still present");
    assert!(
        after.pending_delete_at.is_none(),
        "pending flag cleared after undo"
    );
    assert!(
        after.tombstoned_at.is_none(),
        "must not be tombstoned after undo"
    );

    let entries = audit_list(&db, 50, None, None, false).unwrap();
    // Both a delete and an undo audit row should now exist.
    assert!(
        entries.iter().any(|e| e.op == "delete"),
        "delete audit kept"
    );
    let undo = entries
        .iter()
        .find(|e| e.op == "undo")
        .expect("undo audit row");
    assert!(undo.success);
    assert_eq!(undo.worklog_id.as_deref(), Some("6002"));
}

/// Past the undo window, the background `commit_pending_delete` task issues a
/// Jira DELETE and tombstones the row. We characterize that observable end
/// state by driving the same primitives directly.
#[tokio::test]
async fn commit_pending_delete_path_calls_jira_delete_and_tombstones_row() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-202");
    let before = seed_synced_row(&db, conn_id, "6003", "DEV-202");
    let local_id = before.id.expect("seeded");

    let pending_at = Utc::now().timestamp();
    mark_pending_delete(&db, local_id, pending_at).unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/DEV-202/worklog/6003"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    client
        .delete_worklog("DEV-202", "6003")
        .await
        .expect("jira delete ok");

    // Tombstone after the remote DELETE succeeds (mirrors commit_pending_delete).
    let now_s = Utc::now().timestamp();
    mark_tombstoned(&db, local_id, now_s).unwrap();
    audit::record(
        &db,
        AuditEvent {
            occurred_at: now_s,
            op: AuditOp::Delete,
            issue_key: Some("DEV-202"),
            worklog_id: Some("6003"),
            before: Some(&before),
            after: None,
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();

    let row = get_by_id(&db, local_id).unwrap().expect("row remains");
    assert!(
        row.tombstoned_at.is_some(),
        "tombstoned_at must be set after commit"
    );
    assert!(
        row.pending_delete_at.is_none(),
        "pending flag cleared by mark_tombstoned"
    );
}

/// Jira returning 404 on the DELETE is treated as "already gone" — the row
/// still becomes tombstoned and a success audit is recorded.
#[tokio::test]
async fn commit_pending_delete_treats_404_as_already_gone() {
    let (server, client) = server_and_client().await;
    let (_d, db, conn_id) = fresh_db_with_conn(&server.uri(), "DEV-203");
    let before = seed_synced_row(&db, conn_id, "6004", "DEV-203");
    let local_id = before.id.expect("seeded");

    mark_pending_delete(&db, local_id, Utc::now().timestamp()).unwrap();

    Mock::given(method("DELETE"))
        .and(path("/rest/api/3/issue/DEV-203/worklog/6004"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let err = client.delete_worklog("DEV-203", "6004").await.unwrap_err();
    // The command treats this branch as success (Ok(()) | WorklogNotFound).
    assert!(
        matches!(err, tracker_lib::jira::JiraError::WorklogNotFound),
        "got {err:?}"
    );

    // Same downstream effect as the 204 branch:
    let now_s = Utc::now().timestamp();
    mark_tombstoned(&db, local_id, now_s).unwrap();
    let row = get_by_id(&db, local_id).unwrap().expect("row");
    assert!(row.tombstoned_at.is_some());
}

#[tokio::test]
async fn pending_delete_lookup_prefers_the_row_currently_in_undo_window() {
    let (_s, _c) = server_and_client().await;
    let (_d, db, conn_a) = fresh_db_with_conn("http://example.invalid", "DEV-205");
    let cfg_b = r#"{"base_url":"http://example.invalid","email":"bob@example.com"}"#;
    let conn_b = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "other-tenant",
            enabled: true,
            config_json: cfg_b,
        },
    )
    .expect("seed connection B");
    issue_upsert(
        &db,
        &IssueRow {
            connection_id: conn_b,
            issue_id: "DEV-206".to_string(),
            issue_key: "DEV-206".to_string(),
            name: "Other".into(),
            ..Default::default()
        },
    )
    .expect("seed issue B");

    let row_a = seed_synced_row(&db, conn_a, "shared-undo-id", "DEV-205");
    let row_b = seed_synced_row(&db, conn_b, "shared-undo-id", "DEV-206");

    mark_pending_delete(&db, row_b.id.unwrap(), Utc::now().timestamp()).unwrap();

    let pending = get_pending_delete_by_remote_id_any(&db, "shared-undo-id")
        .unwrap()
        .expect("pending row");
    assert_eq!(pending.id, row_b.id);
    assert_eq!(pending.issue_key.as_deref(), Some("DEV-206"));

    let untouched = get_by_id(&db, row_a.id.unwrap()).unwrap().unwrap();
    assert!(untouched.pending_delete_at.is_none());
}
