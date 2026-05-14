//! Integration tests for the local HTTP server.
//!
//! These tests sidestep `tokio::net::TcpListener::bind` (and the real port
//! 27420) by driving the axum router through `tower::ServiceExt::oneshot`,
//! which constructs an in-memory `Service` and feeds it `Request`s.
//!
//! Two flavours of test live here:
//!
//! 1. Pure-body tests that exercise the simple, AppHandle-free helpers
//!    such as `status_body()`.
//! 2. Full-router tests that build a real `ServerState` (with a Tauri
//!    `MockRuntime` AppHandle) and exercise each endpoint end-to-end.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tauri::Manager;
use tempfile::TempDir;
use tower::ServiceExt;

use tracker_lib::cache::Db;
use tracker_lib::commands::timer::start_timer_inner;
use tracker_lib::config::JiraConfig;
use tracker_lib::server::{build_router, status_body, ServerState};
use tracker_lib::state::AppState;

/// Construct a fresh `(AppHandle, ServerState, Db reference)` triple that
/// tests can use. The `TempDir` is returned so the caller can keep it alive
/// for the test's lifetime — dropping it deletes the SQLite file.
fn fresh_state() -> (tauri::App<tauri::test::MockRuntime>, TempDir) {
    let app = tauri::test::mock_app();
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let app_data_dir = dir.path().to_path_buf();
    let state = AppState::new(db, app_data_dir);
    app.handle().manage(state);
    (app, dir)
}

fn fresh_server_state(
    app: &tauri::App<tauri::test::MockRuntime>,
) -> ServerState<tauri::test::MockRuntime> {
    ServerState::<tauri::test::MockRuntime>::new(app.handle().clone())
}

async fn body_json(response: axum::response::Response) -> Value {
    let body = response.into_body();
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(&bytes).unwrap()
}

// -----------------------------------------------------------------------------
// Pure-body tests.
// -----------------------------------------------------------------------------

#[test]
fn status_body_reports_ok_and_version() {
    let body = status_body();
    assert!(body.ok);
    assert!(!body.version.is_empty());
}

// -----------------------------------------------------------------------------
// Router-driven tests.
// -----------------------------------------------------------------------------

#[tokio::test]
async fn status_endpoint_returns_ok_and_bumps_heartbeat() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state.clone());

    assert!(state.last_heartbeat.read().unwrap().is_none());

    let response = router
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["ok"], true);
    assert!(body["version"].is_string());

    assert!(state.last_heartbeat.read().unwrap().is_some());
}

#[tokio::test]
async fn jira_host_returns_404_when_not_configured() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/jira-host")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn jira_host_returns_config_when_present() {
    let (app, _dir) = fresh_state();
    let app_state = app.state::<AppState>();
    *app_state.jira_config.write().unwrap() = Some(JiraConfig {
        base_url: "https://acme.atlassian.net".into(),
        email: "user@example.com".into(),
    });

    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/jira-host")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["base_url"], "https://acme.atlassian.net");
    assert_eq!(body["email"], "user@example.com");
}

#[tokio::test]
async fn timer_state_is_null_when_no_active_timer() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/timer-state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_null());
}

#[tokio::test]
async fn timer_state_returns_running_timer() {
    let (app, _dir) = fresh_state();
    {
        let app_state = app.state::<AppState>();
        start_timer_inner(&app_state.db, "ACME-7", 1_000, None).unwrap();
    }

    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/timer-state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-7");
}

#[tokio::test]
async fn active_ticket_returns_issue_key() {
    let (app, _dir) = fresh_state();
    {
        let app_state = app.state::<AppState>();
        start_timer_inner(&app_state.db, "ACME-9", 1_000, None).unwrap();
    }

    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/active-ticket")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-9");
}

#[tokio::test]
async fn start_then_stop_timer_flow() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state.clone());

    // 1. POST /start-timer
    let start_req = serde_json::to_vec(&json!({
        "issue_key": "ACME-42",
        "started_at_ms": 100_000,
    }))
    .unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/start-timer")
                .header("content-type", "application/json")
                .body(Body::from(start_req))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-42");
    assert_eq!(body["started_at"], 100_000);

    // Heartbeat should have been bumped at least once.
    assert!(state.last_heartbeat.read().unwrap().is_some());

    // 2. POST /stop-timer
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/stop-timer")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-42");
    // No Jira client configured → jira_worklog_id is null.
    assert!(body["jira_worklog_id"].is_null());
}

#[tokio::test]
async fn cors_headers_are_permissive() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/status")
                .header("origin", "chrome-extension://abcdef")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let cors = response
        .headers()
        .get("access-control-allow-origin")
        .expect("CORS header present");
    // tower-http's permissive layer reflects the origin or returns "*".
    assert!(cors == "*" || cors == "chrome-extension://abcdef");
}

#[tokio::test]
async fn visible_ticket_round_trip() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_router(state.clone());

    // Initially empty.
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/visible-ticket")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_null());

    // Now POST one.
    let payload = serde_json::to_vec(&json!({
        "issue_key": "ACME-1",
        "summary": "Investigate widget",
        "url": "https://acme.atlassian.net/browse/ACME-1",
        "seen_at": null
    }))
    .unwrap();
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/visible-ticket")
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // And read it back.
    let response = router
        .oneshot(
            Request::builder()
                .uri("/visible-ticket")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-1");
    assert!(body["seen_at"].is_i64());
}
