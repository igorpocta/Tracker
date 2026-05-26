//! Multi-tenant Jira routing — pins that mutation paths target the
//! Jira connection that actually owns the issue, not the first one
//! the AppState happens to hold.
//!
//! Before this batch the helper `state.jira_client_cloned()` returned
//! the FIRST Jira client in the connections list, and every mutation
//! flow in `commands/worklog/{crud,audit}.rs` + `commands/timer.rs` +
//! `commands/issues.rs::refresh_cache` used it. With two Jiras
//! configured (e.g. SAB Jira + personal Jira) a POST/PUT/DELETE for
//! an issue belonging to the SECOND tenant would silently land on the
//! FIRST tenant. This test pins the fix: every helper now resolves
//! through `commands::worklog::crud::resolve_jira_client_for_issue`
//! which honours `issues_v2.connection_id`.

use tempfile::TempDir;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::IssueRow;
use tracker_lib::cache::worklogs::{upsert_from_remote, WorklogRow};
use tracker_lib::cache::{self, Db};
use tracker_lib::commands::worklog::crud::{
    resolve_cached_worklog_for_issue_and_remote_id, resolve_jira_client_for_issue,
};
use tracker_lib::jira::JiraClient;
use tracker_lib::state::{ActiveConnection, AppState, ProviderClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Spin up an isolated AppState with two Jira connections, each
/// pointed at its own wiremock server. Returns the state plus both
/// mock servers and connection ids so the test can mount endpoints
/// and assert routing.
async fn two_jira_state() -> (TempDir, AppState, MockServer, MockServer, i64, i64) {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open(&dir.path().join("t.db")).expect("open db");

    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let cfg_a = format!(
        r#"{{"base_url":"{}","email":"alice@example.com"}}"#,
        server_a.uri()
    );
    let cfg_b = format!(
        r#"{{"base_url":"{}","email":"bob@example.com"}}"#,
        server_b.uri()
    );

    let conn_a = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "tenant-a",
            enabled: true,
            config_json: &cfg_a,
        },
    )
    .expect("seed A");
    let conn_b = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "tenant-b",
            enabled: true,
            config_json: &cfg_b,
        },
    )
    .expect("seed B");

    let client_a = JiraClient::new(server_a.uri(), "alice@example.com".into(), "token-a".into())
        .expect("client A");
    let client_b = JiraClient::new(server_b.uri(), "bob@example.com".into(), "token-b".into())
        .expect("client B");

    let state = AppState::new(db, dir.path().to_path_buf());
    {
        let mut conns = state
            .connections
            .write()
            .expect("AppState.connections RwLock poisoned");
        conns.push(ActiveConnection {
            id: conn_a,
            kind: "jira".into(),
            name: "tenant-a".into(),
            enabled: true,
            client: ProviderClient::Jira(client_a),
        });
        conns.push(ActiveConnection {
            id: conn_b,
            kind: "jira".into(),
            name: "tenant-b".into(),
            enabled: true,
            client: ProviderClient::Jira(client_b),
        });
    }

    // Seed one issue per tenant: ACME-1 on connection A, BRAVO-1 on B.
    let issue_a = IssueRow {
        connection_id: conn_a,
        issue_id: "ACME-1".into(),
        issue_key: "ACME-1".into(),
        name: "issue on tenant A".into(),
        ..Default::default()
    };
    let issue_b = IssueRow {
        connection_id: conn_b,
        issue_id: "BRAVO-1".into(),
        issue_key: "BRAVO-1".into(),
        name: "issue on tenant B".into(),
        ..Default::default()
    };
    cache::issues::upsert(&state.db, &issue_a).expect("issue A");
    cache::issues::upsert(&state.db, &issue_b).expect("issue B");

    (dir, state, server_a, server_b, conn_a, conn_b)
}

#[tokio::test]
async fn resolver_routes_each_issue_to_its_own_tenant() {
    let (_dir, state, server_a, server_b, conn_a, conn_b) = two_jira_state().await;

    // ACME-1 belongs to tenant A.
    let (resolved_a, client_a) =
        resolve_jira_client_for_issue(&state, "ACME-1").expect("resolves A");
    assert_eq!(resolved_a, conn_a);
    // base_url() ends with the server's port, so a string contains check is
    // the simplest way to assert "this client points at server A".
    assert!(
        client_a.base_url().starts_with(&server_a.uri()),
        "ACME-1 resolved to {} expected {}",
        client_a.base_url(),
        server_a.uri()
    );

    // BRAVO-1 belongs to tenant B.
    let (resolved_b, client_b) =
        resolve_jira_client_for_issue(&state, "BRAVO-1").expect("resolves B");
    assert_eq!(resolved_b, conn_b);
    assert!(
        client_b.base_url().starts_with(&server_b.uri()),
        "BRAVO-1 resolved to {} expected {}",
        client_b.base_url(),
        server_b.uri()
    );
}

#[tokio::test]
async fn resolver_actually_hits_the_owning_tenants_server() {
    // The hard guarantee we want: the resolved JiraClient performs HTTP
    // round-trips against the OWNING tenant's server, never the other one.
    // We mount distinct response bodies on each server's `/myself` and
    // assert by the body which one served the call.
    let (_dir, state, server_a, server_b, _, _) = two_jira_state().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "tenant-A-account",
            "emailAddress": "alice@example.com",
            "displayName": "Alice",
        })))
        .expect(1)
        .mount(&server_a)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accountId": "tenant-B-account",
            "emailAddress": "bob@example.com",
            "displayName": "Bob",
        })))
        .expect(1)
        .mount(&server_b)
        .await;

    let (_, client_a) = resolve_jira_client_for_issue(&state, "ACME-1").unwrap();
    let me_a = client_a.myself().await.expect("myself A");
    assert_eq!(me_a.account_id, "tenant-A-account");

    let (_, client_b) = resolve_jira_client_for_issue(&state, "BRAVO-1").unwrap();
    let me_b = client_b.myself().await.expect("myself B");
    assert_eq!(me_b.account_id, "tenant-B-account");

    // Wiremock `.expect(1)` on each server verifies both got hit exactly
    // once — if `resolve_jira_client_for_issue` mis-routed, one server
    // would get zero hits and the other two, and the harness would fail.
}

#[tokio::test]
async fn resolver_errors_when_issue_is_unknown_and_multiple_jiras_exist() {
    let (_dir, state, _server_a, _server_b, _conn_a, _conn_b) = two_jira_state().await;

    let err = resolve_jira_client_for_issue(&state, "UNKNOWN-1")
        .expect_err("ambiguous fallback must be rejected");
    assert!(
        err.contains("více možných připojení"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn resolver_falls_back_when_exactly_one_jira_exists() {
    let (_dir, state, server_a, _server_b, conn_a, conn_b) = two_jira_state().await;
    {
        let mut conns = state.connections.write().unwrap();
        conns.retain(|c| c.id != conn_b);
    }

    let (resolved, client) =
        resolve_jira_client_for_issue(&state, "UNKNOWN-1").expect("single-tenant fallback");
    assert_eq!(resolved, conn_a);
    assert!(client.base_url().starts_with(&server_a.uri()));
}

#[tokio::test]
async fn resolver_errors_when_cached_issue_points_to_disabled_connection() {
    let (_dir, state, _server_a, _server_b, conn_a, _conn_b) = two_jira_state().await;
    {
        let mut conns = state.connections.write().unwrap();
        let conn = conns.iter_mut().find(|c| c.id == conn_a).unwrap();
        conn.enabled = false;
    }

    let err =
        resolve_jira_client_for_issue(&state, "ACME-1").expect_err("disabled owner must error");
    assert!(err.contains("není aktivní"), "unexpected error: {err}");
}

#[tokio::test]
async fn resolver_errors_when_issue_is_on_freelo() {
    // A Freelo issue routed through the Jira resolver should fail loudly
    // rather than silently land on a Jira tenant.
    let (_dir, state, _, _, _, _) = two_jira_state().await;

    // Seed a Freelo-style key. Even though the issues table doesn't have
    // a Freelo connection, the resolver inspects the `FREELO-` prefix
    // (see `freelo::is_freelo_key`) and refuses to hand back a Jira
    // client for it — the typed resolver explicitly errors when the
    // resolved provider isn't Jira.
    let err =
        resolve_jira_client_for_issue(&state, "FREELO-12345").expect_err("Freelo key must error");
    assert!(
        err.to_lowercase().contains("freelo") || err.to_lowercase().contains("žádné aktivní"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn cached_worklog_lookup_is_scoped_by_issue_connection() {
    let (_dir, state, _server_a, _server_b, conn_a, conn_b) = two_jira_state().await;

    let row_a = WorklogRow {
        id: None,
        connection_id: Some(conn_a),
        issue_key: Some("ACME-1".into()),
        description: Some("tenant A row".into()),
        started_at: 1_700_000_000,
        ended_at: 1_700_000_900,
        logged_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        is_synced: true,
        synced_at: Some(1_700_000_000),
        remote_id: Some("shared-remote-id".into()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    let row_b = WorklogRow {
        id: None,
        connection_id: Some(conn_b),
        issue_key: Some("BRAVO-1".into()),
        description: Some("tenant B row".into()),
        started_at: 1_700_001_000,
        ended_at: 1_700_001_900,
        logged_at: 1_700_001_000,
        updated_at: 1_700_001_000,
        is_synced: true,
        synced_at: Some(1_700_001_000),
        remote_id: Some("shared-remote-id".into()),
        pending_delete_at: None,
        tombstoned_at: None,
        summary: None,
    };
    upsert_from_remote(&state.db, &row_a).expect("seed A worklog");
    upsert_from_remote(&state.db, &row_b).expect("seed B worklog");

    let resolved_a =
        resolve_cached_worklog_for_issue_and_remote_id(&state, "ACME-1", "shared-remote-id")
            .expect("resolve A row");
    assert_eq!(resolved_a.connection_id, Some(conn_a));
    assert_eq!(resolved_a.issue_key.as_deref(), Some("ACME-1"));

    let resolved_b =
        resolve_cached_worklog_for_issue_and_remote_id(&state, "BRAVO-1", "shared-remote-id")
            .expect("resolve B row");
    assert_eq!(resolved_b.connection_id, Some(conn_b));
    assert_eq!(resolved_b.issue_key.as_deref(), Some("BRAVO-1"));
}
