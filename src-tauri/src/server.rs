//! Local HTTP server (skeleton for a future browser extension).
//!
//! The server binds to `127.0.0.1:27420` and exposes a tiny REST surface that
//! a Chrome / Firefox extension can call to:
//!   - confirm Tracker is running (`/status`),
//!   - learn the configured Jira host (`/jira-host`),
//!   - read the active timer / current ticket (`/active-ticket`,
//!     `/timer-state`),
//!   - drive the timer remotely (`POST /start-timer`, `POST /stop-timer`).
//!
//! For now the extension itself does not exist — this is the **landing pad**
//! so the desktop UI can already start showing "Last seen X seconds ago" with
//! some plumbing in place.
//!
//! ## Heartbeat tracking
//!
//! Every successful request bumps a shared `last_heartbeat` value. The
//! [`crate::commands::browser::get_extension_last_heartbeat`] command reads
//! it back, giving the frontend a "is the extension talking to us?" signal.
//!
//! ## Threading
//!
//! We deliberately spawn axum on its **own** Tokio runtime in a
//! dedicated OS thread instead of reusing Tauri's. That way the server
//! lifecycle is decoupled from any Tauri command runtime and we avoid any
//! "nested runtime" panics.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, RwLock};

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::commands::browser::VisibleTicket;
use crate::commands::timer::{
    get_timer_state_inner, record_local_stop, start_timer_inner, ActiveTimerState,
};
use crate::state::AppState;

/// Port the local HTTP server binds to. Hard-coded for now — the future
/// extension is expected to dial the same port.
pub const SERVER_PORT: u16 = 27420;

/// File name (under `appDataDir`) where the per-install bearer token for
/// the local HTTP server is stored. Generated on first launch.
const BRIDGE_TOKEN_FILE: &str = "browser-bridge-token";

/// Load the persisted bearer token, generating + writing a fresh UUID v4
/// the first time. Stored as plain text in `appDataDir` with `0600` on
/// Unix so other local users can't read it.
///
/// **Why a shared secret at all.** The server binds on loopback, but
/// "loopback" doesn't mean "only my code" — every web page the user has
/// open in any browser also runs on the same machine and can issue
/// `fetch("http://127.0.0.1:27420/...")`. Pre-fix the router was
/// unprotected AND used `CorsLayer::permissive()`, so any page could
/// read the configured Jira host + email and start/stop the timer for
/// any issue key. The bearer token blocks that — only software that
/// has been explicitly handed the token (i.e. the user pasted it into
/// their browser extension's settings) can hit the endpoints.
pub fn load_or_create_bridge_token(app_data_dir: &Path) -> std::io::Result<String> {
    let path = app_data_dir.join(BRIDGE_TOKEN_FILE);
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    std::fs::create_dir_all(app_data_dir).ok();
    std::fs::write(&path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }
    Ok(token)
}

/// Constant-time string compare. Avoids leaking the matched prefix length
/// via timing — irrelevant on localhost in practice, but cheap insurance
/// and keeps reviewers happy.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Axum middleware that requires `Authorization: Bearer <token>` on every
/// request. The expected token is captured into the middleware closure
/// when the router is built so each request doesn't re-read it from disk.
///
/// Responses are intentionally terse (`401` with empty body) — no leaking
/// of "did the token even have the right shape" details.
async fn require_bearer(
    State(expected): State<Arc<String>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let header_value = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());
    let provided = match header_value {
        Some(v) => v
            .strip_prefix("Bearer ")
            .or_else(|| v.strip_prefix("bearer "))
            .unwrap_or(""),
        None => return Err(StatusCode::UNAUTHORIZED),
    };
    if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

/// Shared state passed to every axum handler.
///
/// Cheap to clone (everything inside is wrapped in `Arc`), so axum's
/// `with_state` requirement is satisfied without contention.
///
/// Generic over the Tauri [`Runtime`] so integration tests can plug in
/// `tauri::test::MockRuntime` without spinning up an OS WebView.
pub struct ServerState<R: Runtime = tauri::Wry> {
    /// Handle back into the Tauri app, used to fetch the managed
    /// [`AppState`] (which owns the DB and the Jira client).
    pub app: AppHandle<R>,
    /// Unix-seconds timestamp of the last successful HTTP request, or
    /// `None` if nothing has ever called the server yet.
    pub last_heartbeat: Arc<RwLock<Option<i64>>>,
    /// Last ticket the browser extension reported as "currently visible".
    /// The extension is expected to `POST /visible-ticket` periodically.
    pub visible_ticket: Arc<RwLock<Option<VisibleTicket>>>,
}

impl<R: Runtime> Clone for ServerState<R> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            last_heartbeat: self.last_heartbeat.clone(),
            visible_ticket: self.visible_ticket.clone(),
        }
    }
}

impl<R: Runtime> ServerState<R> {
    /// Construct a fresh, empty state tied to the given Tauri app handle.
    pub fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            last_heartbeat: Arc::new(RwLock::new(None)),
            visible_ticket: Arc::new(RwLock::new(None)),
        }
    }

    /// Record "we just got a request" — bumps `last_heartbeat` to now.
    pub fn bump_heartbeat(&self) {
        let now = Utc::now().timestamp();
        *self
            .last_heartbeat
            .write()
            .expect("WidgetState.last_heartbeat RwLock poisoned") = Some(now);
    }
}

// -----------------------------------------------------------------------------
// Public entry: start the server in the background.
// -----------------------------------------------------------------------------

/// Spawn the axum server in a dedicated background thread and stash the
/// [`ServerState`] into the Tauri-managed state so other commands can read
/// the heartbeat / visible ticket.
///
/// This call returns immediately; the actual bind happens on a worker
/// thread. Bind failures are logged via `tracing` and do **not** abort
/// the Tauri setup — the desktop app should still run if (say) port 27420
/// is already taken by another instance.
pub fn start(app: AppHandle) {
    let server_state: ServerState<tauri::Wry> = ServerState::new(app.clone());
    let router_state = server_state.clone();

    // Load (or generate) the bearer token from `appDataDir`. Failure here is
    // logged but non-fatal: we'd rather see the desktop UI run and the
    // bridge disabled than abort startup. Without a token the server can't
    // come up safely, so we just skip the spawn.
    let app_data_dir = match app.path().app_data_dir() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("local HTTP server: app data dir unavailable, bridge disabled: {e}");
            app.manage(server_state);
            return;
        }
    };
    let token = match load_or_create_bridge_token(&app_data_dir) {
        Ok(t) => Arc::new(t),
        Err(e) => {
            tracing::warn!("local HTTP server: bridge token init failed, bridge disabled: {e}");
            app.manage(server_state);
            return;
        }
    };
    app.manage(BrowserBridgeToken(token.clone()));

    // Park the server state where other parts of the codebase can grab it.
    app.manage(server_state);

    std::thread::Builder::new()
        .name("tracker-http".into())
        .spawn(move || run_server(router_state, token))
        .expect("spawn local HTTP server thread");
}

/// Newtype wrapper so the bearer token can be `manage()`d into the Tauri
/// state map without colliding with other `Arc<String>` consumers.
pub struct BrowserBridgeToken(pub Arc<String>);

impl BrowserBridgeToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Build the axum router. Exposed separately from [`run_server`] so
/// integration tests can call handlers without binding to a port.
///
/// `bearer_token` is the per-install secret callers must present as
/// `Authorization: Bearer <token>`. Every route — including `/status`
/// — is gated behind it: even probing for the server's existence
/// requires the token, so a hostile web page can't enumerate
/// background heartbeats.
///
/// The `CorsLayer::permissive()` of the pre-fix version is gone on
/// purpose. Without an explicit CORS allow-list, browsers refuse the
/// cross-origin preflight, so an arbitrary web tab cannot reach these
/// endpoints (browser extensions with `host_permissions` bypass CORS
/// and CAN — that's the intended consumer).
pub fn build_router<R: Runtime>(state: ServerState<R>, bearer_token: Arc<String>) -> Router {
    Router::new()
        .route("/status", get(status_handler::<R>))
        .route("/jira-host", get(jira_host_handler::<R>))
        .route("/active-ticket", get(active_ticket_handler::<R>))
        .route("/timer-state", get(timer_state_handler::<R>))
        .route("/visible-ticket", get(get_visible_ticket_handler::<R>))
        .route("/visible-ticket", post(post_visible_ticket_handler::<R>))
        .route("/start-timer", post(start_timer_handler::<R>))
        .route("/stop-timer", post(stop_timer_handler::<R>))
        .route_layer(middleware::from_fn_with_state(bearer_token, require_bearer))
        .with_state(state)
}

fn run_server(state: ServerState<tauri::Wry>, bearer_token: Arc<String>) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!("local HTTP server: failed to build tokio runtime: {e}");
            return;
        }
    };

    rt.block_on(async move {
        let app = build_router(state, bearer_token);
        let addr: SocketAddr = ([127, 0, 0, 1], SERVER_PORT).into();
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                tracing::info!("local HTTP server listening on {addr}");
                if let Err(e) = axum::serve(listener, app.into_make_service()).await {
                    tracing::warn!("local HTTP server stopped: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("local HTTP server: bind to {addr} failed: {e}");
            }
        }
    });
}

// -----------------------------------------------------------------------------
// Response types.
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub ok: bool,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraHostResponse {
    pub base_url: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTimerRequest {
    pub issue_key: String,
    /// Optional override for the start time, in milliseconds since epoch.
    /// If omitted we use "now".
    pub started_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopTimerRequest {
    pub comment: Option<String>,
}

// -----------------------------------------------------------------------------
// Error helper: anything that goes wrong inside a handler funnels through here
// so we get consistent JSON and proper status codes.
// -----------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

fn err_response(status: StatusCode, message: impl Into<String>) -> Response {
    let body = ErrorBody {
        error: message.into(),
    };
    (status, Json(body)).into_response()
}

// -----------------------------------------------------------------------------
// Handlers.
// -----------------------------------------------------------------------------

/// Pure body of `/status` — `{ "ok": true, "version": "0.1.0" }`. Split out
/// from the handler so integration tests don't need a Tauri runtime.
pub fn status_body() -> StatusResponse {
    StatusResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    }
}

async fn status_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Json<StatusResponse> {
    state.bump_heartbeat();
    Json(status_body())
}

async fn jira_host_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    state.bump_heartbeat();
    let app_state = match state.app.try_state::<AppState>() {
        Some(s) => s,
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };
    let cfg = app_state.jira_config_cloned();
    match cfg {
        Some(c) => Json(JiraHostResponse {
            base_url: c.base_url,
            email: c.email,
        })
        .into_response(),
        None => err_response(StatusCode::NOT_FOUND, "jira not configured"),
    }
}

async fn active_ticket_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    state.bump_heartbeat();
    let app_state = match state.app.try_state::<AppState>() {
        Some(s) => s,
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };
    match get_timer_state_inner(&app_state.db, now_ms()) {
        Ok(Some(s)) => Json(serde_json::json!({ "issue_key": s.issue_key })).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn timer_state_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    state.bump_heartbeat();
    let app_state = match state.app.try_state::<AppState>() {
        Some(s) => s,
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };
    match get_timer_state_inner(&app_state.db, now_ms()) {
        Ok(snap) => Json::<Option<ActiveTimerState>>(snap).into_response(),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn start_timer_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    Json(req): Json<StartTimerRequest>,
) -> Response {
    state.bump_heartbeat();
    let app_state = match state.app.try_state::<AppState>() {
        Some(s) => s,
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };
    let started_at = req.started_at_ms.unwrap_or_else(now_ms);
    match start_timer_inner(&app_state.db, &req.issue_key, started_at, None) {
        Ok(snap) => {
            use tauri::Emitter;
            let _ = state.app.emit("timer-started", &snap);
            Json(snap).into_response()
        }
        Err(e) => err_response(StatusCode::BAD_REQUEST, e),
    }
}

async fn stop_timer_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    body: Option<Json<StopTimerRequest>>,
) -> Response {
    state.bump_heartbeat();
    let app_state = match state.app.try_state::<AppState>() {
        Some(s) => s,
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };

    // Pull the running timer (if any).
    let timer = match crate::cache::timer::get(&app_state.db) {
        Ok(Some(t)) => t,
        Ok(None) => return Json(serde_json::Value::Null).into_response(),
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let comment = body.and_then(|Json(b)| b.comment);

    // We deliberately don't talk to Jira here — that path lives in the
    // Tauri command (which already handles event emission, error surfacing,
    // etc.). The HTTP endpoint just records the local stop so the
    // extension gets a synchronous answer; the desktop UI will catch up
    // via the next refresh.
    let row = match record_local_stop(
        &app_state.db,
        &timer,
        now_ms(),
        comment.as_deref(),
        None,
        None,
    ) {
        Ok(r) => r,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    };

    use tauri::Emitter;
    let _ = state.app.emit("worklog-saved", &row);
    let _ = state.app.emit("timer-stopped", &row);

    Json(row).into_response()
}

async fn get_visible_ticket_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    state.bump_heartbeat();
    let v = state
        .visible_ticket
        .read()
        .expect("WidgetState.visible_ticket RwLock poisoned")
        .clone();
    Json::<Option<VisibleTicket>>(v).into_response()
}

async fn post_visible_ticket_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    Json(mut ticket): Json<VisibleTicket>,
) -> Response {
    state.bump_heartbeat();
    if ticket.seen_at.is_none() {
        ticket.seen_at = Some(Utc::now().timestamp());
    }
    *state
        .visible_ticket
        .write()
        .expect("WidgetState.visible_ticket RwLock poisoned") = Some(ticket.clone());
    Json(ticket).into_response()
}

// -----------------------------------------------------------------------------
// Helpers.
// -----------------------------------------------------------------------------

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}
