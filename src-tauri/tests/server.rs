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

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::{json, Value};
use tauri::Manager;
use tempfile::TempDir;
use tower::ServiceExt;

use tracker_lib::cache::Db;
use tracker_lib::commands::rounding::{
    set_rounding_interval_minutes_inner, set_rounding_mode_inner,
};
use tracker_lib::commands::timer::start_timer_inner;
use tracker_lib::config::JiraConfig;
use tracker_lib::server::{build_router, status_body, ServerState};
use tracker_lib::state::AppState;

/// Shared token used by every authed request below. Real installs use a
/// fresh UUID v4 generated on first launch; tests pin a deterministic
/// value so the helper can stamp it into the `Authorization` header.
const TEST_BEARER: &str = "test-bearer-token-aaaa-bbbb-cccc";

fn build_test_router(state: ServerState<tauri::test::MockRuntime>) -> Router {
    build_router(state, Arc::new(TEST_BEARER.to_string()))
}

/// Builds a `GET <uri>` request with the test bearer token attached.
fn authed_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {TEST_BEARER}"))
        .body(Body::empty())
        .unwrap()
}

/// Builds a `POST <uri>` request with bearer + JSON body.
fn authed_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {TEST_BEARER}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

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
    let router = build_test_router(state.clone());

    assert!(state.last_heartbeat.read().unwrap().is_none());

    let response = router.oneshot(authed_get("/status")).await.unwrap();

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
    let router = build_test_router(state);

    let response = router.oneshot(authed_get("/jira-host")).await.unwrap();

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
    let router = build_test_router(state);

    let response = router.oneshot(authed_get("/jira-host")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["base_url"], "https://acme.atlassian.net");
    assert_eq!(body["email"], "user@example.com");
}

#[tokio::test]
async fn timer_state_is_null_when_no_active_timer() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state);

    let response = router.oneshot(authed_get("/timer-state")).await.unwrap();

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
    let router = build_test_router(state);

    let response = router.oneshot(authed_get("/timer-state")).await.unwrap();

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
    let router = build_test_router(state);

    let response = router.oneshot(authed_get("/active-ticket")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-9");
}

#[tokio::test]
async fn start_then_stop_timer_flow() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state.clone());

    // 1. POST /start-timer
    let response = router
        .clone()
        .oneshot(authed_post(
            "/start-timer",
            json!({ "issue_key": "ACME-42", "started_at_ms": 100_000 }),
        ))
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
        .oneshot(authed_post("/stop-timer", json!({})))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-42");
    // No Jira client configured → jira_worklog_id is null.
    assert!(body["jira_worklog_id"].is_null());
}

#[tokio::test]
async fn stop_timer_applies_rounding_and_comment_fallback_like_main_flow() {
    let (app, _dir) = fresh_state();
    {
        let app_state = app.state::<AppState>();
        set_rounding_mode_inner(&app_state.db, "up").unwrap();
        set_rounding_interval_minutes_inner(&app_state.db, 15).unwrap();
        let started_at_ms = (chrono::Utc::now().timestamp() - 14 * 60) * 1000;
        start_timer_inner(&app_state.db, "ACME-43", started_at_ms, Some("draft note")).unwrap();
    }

    let state = fresh_server_state(&app);
    let router = build_test_router(state);
    let response = router
        .oneshot(authed_post("/stop-timer", json!({})))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["duration_s"], 15 * 60);
    assert_eq!(body["comment"], "draft note");
}

#[tokio::test]
async fn visible_ticket_round_trip() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state.clone());

    // Initially empty.
    let response = router
        .clone()
        .oneshot(authed_get("/visible-ticket"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert!(body.is_null());

    // Now POST one.
    let response = router
        .clone()
        .oneshot(authed_post(
            "/visible-ticket",
            json!({
                "issue_key": "ACME-1",
                "summary": "Investigate widget",
                "url": "https://acme.atlassian.net/browse/ACME-1",
                "seen_at": null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // And read it back.
    let response = router.oneshot(authed_get("/visible-ticket")).await.unwrap();
    let body = body_json(response).await;
    assert_eq!(body["issue_key"], "ACME-1");
    assert!(body["seen_at"].is_i64());
}

// -----------------------------------------------------------------------------
// Bearer-token guard tests — these are the regression pins for the
// "any web page can hit 127.0.0.1:27420" class of bug.
// -----------------------------------------------------------------------------

/// Every protected endpoint must reject requests without an Authorization
/// header. Pre-fix, all of these were reachable from any browser tab.
#[tokio::test]
async fn endpoints_require_bearer_token() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state.clone());

    for uri in [
        "/status",
        "/jira-host",
        "/active-ticket",
        "/timer-state",
        "/visible-ticket",
    ] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "GET {uri} without Authorization should be 401"
        );
    }

    // And the POST mutators.
    for (uri, body) in [
        ("/start-timer", json!({ "issue_key": "ACME-1" })),
        ("/stop-timer", json!({})),
        (
            "/visible-ticket",
            json!({ "issue_key": "ACME-1", "summary": null, "url": null, "seen_at": null }),
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "POST {uri} without Authorization should be 401"
        );
    }
}

/// A bearer header with the wrong value must also fail — defends against
/// a hostile script bruteforcing or guessing prefixes.
#[tokio::test]
async fn wrong_bearer_token_is_rejected() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/status")
                .header("authorization", "Bearer not-the-right-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Pre-fix this assertion checked for the permissive `Access-Control-
/// Allow-Origin: *`. That layer is gone on purpose — without an
/// allow-origin response header, the browser refuses the cross-origin
/// fetch from a regular web tab. We assert ABSENCE of the header here
/// so a future revert of the CorsLayer would fail loudly.
#[tokio::test]
async fn no_permissive_cors_header_on_responses() {
    let (app, _dir) = fresh_state();
    let state = fresh_server_state(&app);
    let router = build_test_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/status")
                .header("authorization", format!("Bearer {TEST_BEARER}"))
                .header("origin", "https://attacker.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "Access-Control-Allow-Origin must not leak; got: {:?}",
        response.headers().get("access-control-allow-origin")
    );
}
