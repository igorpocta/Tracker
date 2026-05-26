//! Multi-tenant Freelo routing — pins that mutation paths target the
//! Freelo connection that actually owns the work-report, not the first
//! one the AppState happens to hold, AND that a missing `sync_user_id`
//! produces an explicit error instead of an `unwrap_or(0)` invalid POST.
//!
//! Before this batch:
//!   - `stop_timer_inner`, `commit_pending_delete`, and the three
//!     audit-reconstruct paths each picked the FIRST Freelo client via
//!     `find_map`. With two Freelos configured, a delete or restore for
//!     a work-report owned by the second tenant could silently land on
//!     the first tenant.
//!   - `stop_timer_inner` additionally fell back to
//!     `cfg.sync_user_id.unwrap_or(0)`, which Freelo's API rejected with
//!     a generic 400 instead of the more helpful "finish setup first".

use tempfile::TempDir;
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::IssueRow;
use tracker_lib::cache::{self, Db};
use tracker_lib::commands::connections::FreeloConnectionConfig;
use tracker_lib::commands::worklog::crud::{
    resolve_freelo_client_with_user_for_issue, resolve_freelo_service_for_issue,
};
use tracker_lib::freelo::worklog_service_impl::FreeloService;
use tracker_lib::freelo::FreeloClient;
use tracker_lib::state::{ActiveConnection, AppState, ProviderClient};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const EMAIL_A: &str = "alice@example.com";
const EMAIL_B: &str = "bob@example.com";

/// Set up an isolated AppState with two Freelo connections, each
/// pointed at its own wiremock server. By default both connections
/// have a cached `sync_user_id`; tests that need the missing-user-id
/// branch pass `None` for one of them.
async fn two_freelo_state(
    sync_user_id_a: Option<i64>,
    sync_user_id_b: Option<i64>,
) -> (TempDir, AppState, MockServer, MockServer, i64, i64) {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open(&dir.path().join("t.db")).expect("open db");

    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    let cfg_a_json = serde_json::to_string(&FreeloConnectionConfig {
        base_url: server_a.uri(),
        email: EMAIL_A.into(),
        selected_project_ids: vec![],
        sync_user_id: sync_user_id_a,
        color: None,
    })
    .unwrap();
    let cfg_b_json = serde_json::to_string(&FreeloConnectionConfig {
        base_url: server_b.uri(),
        email: EMAIL_B.into(),
        selected_project_ids: vec![],
        sync_user_id: sync_user_id_b,
        color: None,
    })
    .unwrap();

    let conn_a = insert_conn(
        &db,
        NewConnection {
            provider: "freelo",
            name: "tenant-a",
            enabled: true,
            config_json: &cfg_a_json,
        },
    )
    .expect("seed A");
    let conn_b = insert_conn(
        &db,
        NewConnection {
            provider: "freelo",
            name: "tenant-b",
            enabled: true,
            config_json: &cfg_b_json,
        },
    )
    .expect("seed B");

    let client_a =
        FreeloClient::new(server_a.uri(), EMAIL_A.into(), "key-a".into()).expect("client A");
    let client_b =
        FreeloClient::new(server_b.uri(), EMAIL_B.into(), "key-b".into()).expect("client B");

    let svc_a = FreeloService::new(
        client_a,
        FreeloConnectionConfig {
            base_url: server_a.uri(),
            email: EMAIL_A.into(),
            selected_project_ids: vec![],
            sync_user_id: sync_user_id_a,
            color: None,
        },
    );
    let svc_b = FreeloService::new(
        client_b,
        FreeloConnectionConfig {
            base_url: server_b.uri(),
            email: EMAIL_B.into(),
            selected_project_ids: vec![],
            sync_user_id: sync_user_id_b,
            color: None,
        },
    );

    let state = AppState::new(db, dir.path().to_path_buf());
    {
        let mut conns = state
            .connections
            .write()
            .expect("AppState.connections RwLock poisoned");
        conns.push(ActiveConnection {
            id: conn_a,
            kind: "freelo".into(),
            name: "tenant-a".into(),
            enabled: true,
            client: ProviderClient::Freelo(svc_a),
        });
        conns.push(ActiveConnection {
            id: conn_b,
            kind: "freelo".into(),
            name: "tenant-b".into(),
            enabled: true,
            client: ProviderClient::Freelo(svc_b),
        });
    }

    // Seed one Freelo task per tenant. Use the `FREELO-` prefix the
    // resolver matches on; pin different IDs so the issue keys are
    // distinct.
    let issue_a = IssueRow {
        connection_id: conn_a,
        issue_id: "1001".into(),
        issue_key: "FREELO-1001".into(),
        name: "task on tenant A".into(),
        ..Default::default()
    };
    let issue_b = IssueRow {
        connection_id: conn_b,
        issue_id: "2002".into(),
        issue_key: "FREELO-2002".into(),
        name: "task on tenant B".into(),
        ..Default::default()
    };
    cache::issues::upsert(&state.db, &issue_a).expect("issue A");
    cache::issues::upsert(&state.db, &issue_b).expect("issue B");

    (dir, state, server_a, server_b, conn_a, conn_b)
}

#[tokio::test]
async fn resolver_routes_each_freelo_task_to_its_own_tenant() {
    let (_dir, state, _server_a, _server_b, conn_a, conn_b) =
        two_freelo_state(Some(1), Some(2)).await;

    let (resolved_a, svc_a) =
        resolve_freelo_service_for_issue(&state, "FREELO-1001").expect("resolves A");
    assert_eq!(resolved_a, conn_a);
    assert_eq!(svc_a.config.email, EMAIL_A);

    let (resolved_b, svc_b) =
        resolve_freelo_service_for_issue(&state, "FREELO-2002").expect("resolves B");
    assert_eq!(resolved_b, conn_b);
    assert_eq!(svc_b.config.email, EMAIL_B);
}

#[tokio::test]
async fn resolver_with_user_returns_user_id_for_each_tenant() {
    let (_dir, state, _server_a, _server_b, conn_a, conn_b) =
        two_freelo_state(Some(11), Some(22)).await;

    let (cid_a, _, user_a) =
        resolve_freelo_client_with_user_for_issue(&state, "FREELO-1001").expect("ok A");
    assert_eq!(cid_a, conn_a);
    assert_eq!(user_a, 11);

    let (cid_b, _, user_b) =
        resolve_freelo_client_with_user_for_issue(&state, "FREELO-2002").expect("ok B");
    assert_eq!(cid_b, conn_b);
    assert_eq!(user_b, 22);
}

#[tokio::test]
async fn resolver_errors_when_issue_is_unknown_and_multiple_freelos_exist() {
    let (_dir, state, _server_a, _server_b, _, _) = two_freelo_state(Some(11), Some(22)).await;

    let err = resolve_freelo_service_for_issue(&state, "FREELO-9999")
        .expect_err("ambiguous fallback must be rejected");
    assert!(
        err.contains("více možných připojení"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn resolver_with_user_errors_when_sync_user_id_is_missing() {
    // Tenant A is configured but its `sync_user_id` hasn't been
    // discovered yet (no initial sync has run). Pre-fix the timer
    // path would have fallen back to `.unwrap_or(0)` and made a POST
    // that Freelo's API rejects with a generic 400. Post-fix the
    // resolver returns a typed error the UI can render as
    // "finish setup first".
    let (_dir, state, _server_a, _server_b, _, _) = two_freelo_state(None, Some(22)).await;

    let err = resolve_freelo_client_with_user_for_issue(&state, "FREELO-1001")
        .expect_err("must error on missing user id");
    assert!(
        err.contains("user id"),
        "expected user-id error, got: {err}"
    );
}

#[tokio::test]
async fn resolver_actually_hits_the_owning_tenants_server() {
    // The hard end-to-end check: call /users/me through the resolved
    // client and assert wiremock's `.expect(1)` is satisfied on the
    // OWNING server (not the other one).
    let (_dir, state, server_a, server_b, _, _) = two_freelo_state(Some(11), Some(22)).await;

    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": "success",
            "user": { "id": 11 }
        })))
        .expect(1)
        .mount(&server_a)
        .await;
    Mock::given(method("GET"))
        .and(path("/users/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": "success",
            "user": { "id": 22 }
        })))
        .expect(1)
        .mount(&server_b)
        .await;

    let (_, svc_a) = resolve_freelo_service_for_issue(&state, "FREELO-1001").unwrap();
    let me_a = svc_a.client.me().await.expect("me A");
    assert_eq!(me_a.id, 11);

    let (_, svc_b) = resolve_freelo_service_for_issue(&state, "FREELO-2002").unwrap();
    let me_b = svc_b.client.me().await.expect("me B");
    assert_eq!(me_b.id, 22);

    // wiremock `.expect(1)` on each server verifies both got hit
    // exactly once — mis-routing would leave one at zero and the
    // other at two, and the harness would fail.
}
