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
    extract::{Query, RawQuery, Request, State},
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
    build_stop_record_plan, get_timer_state_inner, record_local_stop, start_timer_inner,
    ActiveTimerState,
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
    /// Secret embedded in the block page and required by `POST /focus/allow`.
    ///
    /// The block page is served without a bearer token — it has to be, a
    /// browser landing on it cannot present one. That is harmless while the
    /// page only *reads*, but allow-listing a site is a mutation, and any
    /// local web page can `fetch` loopback. Without a check, a page could
    /// simply un-block itself.
    ///
    /// The nonce closes that: it lives in the page body, and the response
    /// carries no CORS headers, so a cross-origin script cannot read it back.
    /// Only something that can actually see the block page can act on it.
    pub page_nonce: Arc<String>,
}

impl<R: Runtime> Clone for ServerState<R> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            last_heartbeat: self.last_heartbeat.clone(),
            visible_ticket: self.visible_ticket.clone(),
            page_nonce: self.page_nonce.clone(),
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
            page_nonce: Arc::new(uuid::Uuid::new_v4().to_string()),
        }
    }

    /// Record "we just got a request" — bumps `last_heartbeat` to now.
    pub fn bump_heartbeat(&self) {
        let now = Utc::now().timestamp();
        *self
            .last_heartbeat
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(now);
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
/// Two public routes sit outside the bearer-token wall, because the *browser*
/// requests them and a browser cannot be handed the token:
///
/// * `/blocked` is the page a blocked tab lands on. It is rendered
///   server-side, so no local page can pull the rule list out of it.
/// * `/focus/ping` reports only whether a session is running, which is
///   already obvious to anyone looking at the screen.
pub fn build_router<R: Runtime>(state: ServerState<R>, bearer_token: Arc<String>) -> Router {
    let authenticated = Router::new()
        .route("/status", get(status_handler::<R>))
        .route("/jira-host", get(jira_host_handler::<R>))
        .route("/active-ticket", get(active_ticket_handler::<R>))
        .route("/timer-state", get(timer_state_handler::<R>))
        .route("/visible-ticket", get(get_visible_ticket_handler::<R>))
        .route("/visible-ticket", post(post_visible_ticket_handler::<R>))
        .route("/start-timer", post(start_timer_handler::<R>))
        .route("/stop-timer", post(stop_timer_handler::<R>))
        .route("/focus/state", get(focus_state_handler::<R>))
        .route_layer(middleware::from_fn_with_state(bearer_token, require_bearer))
        .with_state(state.clone());

    let public = Router::new()
        .route("/blocked", get(blocked_page_handler::<R>))
        .route("/focus/ping", get(focus_ping_handler::<R>))
        .route("/focus/allow", post(focus_allow_handler::<R>))
        .with_state(state);

    authenticated.merge(public)
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
    // HTTP bridge start has no explicit tenant → connection resolves from the
    // issue key at stop time (None here).
    match start_timer_inner(&app_state.db, &req.issue_key, started_at, None, None) {
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
    let now = now_ms();
    let plan = build_stop_record_plan(&app_state.db, &timer, now, comment.as_deref());

    // Record the local row synchronously so the extension gets an
    // immediate response.
    let row = match record_local_stop(
        &app_state.db,
        &timer,
        now,
        plan.effective_comment.as_deref(),
        None,
        Some(plan.duration_s),
    ) {
        Ok(r) => r,
        Err(e) => return err_response(StatusCode::INTERNAL_SERVER_ERROR, e),
    };

    use tauri::Emitter;
    let _ = state.app.emit("worklog-saved", &row);
    let _ = state.app.emit("timer-stopped", &row);

    // Fire-and-forget upstream push so the row actually lands on the
    // provider. Pre-fix the handler stopped here with a "the desktop UI
    // will catch up via the next refresh" comment, but refresh is
    // pull-only — local rows without `remote_id` would stay stuck until
    // the user clicked "Synchronizovat" by hand. Re-use the same helper
    // the Tauri command runs, so the dispatch (Jira / Freelo per
    // connection + audit linkage) is identical to the desktop flow.
    //
    // Errors are logged but not surfaced — the extension caller already
    // got its 200 with the local row; a transient network blip will be
    // retried by the startup-flush task next launch.
    if let Some(local_id) = row.id {
        let app_handle = state.app.clone();
        tauri::async_runtime::spawn(async move {
            let Some(app_state) = app_handle.try_state::<AppState>() else {
                tracing::warn!("/stop-timer flush: app state missing, skipping push");
                return;
            };
            if let Err(e) = crate::commands::worklog::crud::push_local_worklog_inner(
                &app_handle,
                &app_state,
                local_id,
            )
            .await
            {
                tracing::warn!("/stop-timer flush: upstream push failed: {e}");
            }
        });
    }

    Json(row).into_response()
}

async fn get_visible_ticket_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    state.bump_heartbeat();
    let v = state
        .visible_ticket
        .read()
        .unwrap_or_else(|e| e.into_inner())
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
        .unwrap_or_else(|e| e.into_inner()) = Some(ticket.clone());
    Json(ticket).into_response()
}

// -----------------------------------------------------------------------------
// Focus mode.
// -----------------------------------------------------------------------------

/// Longest a `/focus/state` long-poll is allowed to park. Kept under the
/// browser's own idle timeouts so the extension's service worker isn't held
/// open indefinitely.
const FOCUS_MAX_WAIT_SECS: u64 = 30;

#[derive(Debug, Default, Deserialize)]
pub struct FocusStateQuery {
    /// Seconds to wait for a change before replying. `0`/absent = reply now.
    pub wait: Option<u64>,
    /// Generation the caller already has. When it differs from ours we reply
    /// immediately — this closes the race where a change lands between the
    /// caller's last reply and its next request.
    #[serde(rename = "gen")]
    pub generation: Option<u64>,
}

/// The ruleset as the browser extension consumes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRulesResponse {
    pub active: bool,
    pub generation: u64,
    /// Allow-list mode: block every site that isn't explicitly allowed.
    pub strict_sites: bool,
    /// Site patterns to block.
    pub block: Vec<String>,
    /// Site patterns to allow (also the exceptions to `block`).
    pub allow: Vec<String>,
    /// Where blocked requests should be redirected.
    pub blocked_page: String,
}

fn focus_rules_response(app_state: &AppState) -> FocusRulesResponse {
    let runtime = app_state
        .focus
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let settings = crate::focus::engine::load_settings(&app_state.db);
    let rules = crate::cache::focus::list_enabled(&app_state.db).unwrap_or_default();

    // The extension only understands an explicit `*.` prefix. Whether a bare
    // pattern sweeps subdomains is decided here, by `normalize_site_pattern`,
    // so the rule lives in exactly one place — expand it on the way out rather
    // than teaching the browser half to work it out again and risk the two
    // disagreeing about what is blocked.
    let collect = |mode: &str| -> Vec<String> {
        rules
            .iter()
            .filter(|r| r.kind == "site" && r.mode == mode)
            .map(|r| expand_site_pattern(&r.pattern))
            .collect()
    };

    let allow = collect("allow");
    FocusRulesResponse {
        active: runtime.active,
        generation: runtime.generation,
        // The *effective* flag, not the stored one. An empty allow-list means
        // the user hasn't filled it in yet, and publishing strict mode anyway
        // would have the extension redirect every page they open.
        strict_sites: settings.strict_sites && !allow.is_empty(),
        block: collect("block"),
        allow,
        blocked_page: format!("http://127.0.0.1:{SERVER_PORT}/blocked"),
    }
}

/// Rewrite a stored pattern into the form the extension matches on: `*.host`
/// when subdomains are covered, plain `host` when they are not.
///
/// Falls back to the pattern verbatim if it cannot be parsed — the extension
/// then drops it, which is the safe direction.
pub fn expand_site_pattern(pattern: &str) -> String {
    match crate::focus::rules::normalize_site_pattern(pattern) {
        Some(p) => {
            let prefix = if p.covers_subdomains { "*." } else { "" };
            let path = if p.path == "/" { "" } else { &p.path };
            format!("{prefix}{}{path}", p.host)
        }
        None => pattern.to_string(),
    }
}

async fn focus_state_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    Query(query): Query<FocusStateQuery>,
) -> Response {
    state.bump_heartbeat();

    // Grab the waker before any await so the Tauri state guard never crosses
    // a suspension point.
    let notify = match state.app.try_state::<AppState>() {
        Some(s) => s.focus_notify.clone(),
        None => return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing"),
    };

    let wait = query.wait.unwrap_or(0).min(FOCUS_MAX_WAIT_SECS);
    if wait > 0 {
        loop {
            // Register as a waiter BEFORE reading the generation. `Notify`
            // only wakes waiters that are already registered, so reading
            // first would drop a change landing in between and stall the
            // caller for the whole wait window.
            let notified = notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            let current = {
                let Some(app_state) = state.app.try_state::<AppState>() else {
                    break;
                };
                // Bound to a local so the read guard drops before the Tauri
                // state guard — the block's tail expression would otherwise
                // outlive `app_state`.
                let generation = app_state
                    .focus
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .generation;
                generation
            };
            if query.generation != Some(current) {
                break;
            }
            let timed_out = tokio::time::timeout(std::time::Duration::from_secs(wait), notified)
                .await
                .is_err();
            if timed_out {
                break;
            }
        }
    }

    let Some(app_state) = state.app.try_state::<AppState>() else {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing");
    };
    Json(focus_rules_response(&app_state)).into_response()
}

async fn focus_ping_handler<R: Runtime>(State(state): State<ServerState<R>>) -> Response {
    // Deliberately does NOT bump the heartbeat: this is the block page
    // polling, not the extension, and it would fake an "extension connected"
    // signal in Settings.
    let active = state
        .app
        .try_state::<AppState>()
        .map(|s| crate::focus::engine::is_active(&s))
        .unwrap_or(false);
    Json(serde_json::json!({ "active": active })).into_response()
}

/// Header carrying the block page's nonce on an allow-list request.
const FOCUS_NONCE_HEADER: &str = "x-focus-nonce";

#[derive(Debug, Deserialize)]
pub struct AllowRequest {
    /// Pattern to allow, as edited by the user on the block page.
    pub pattern: String,
}

/// `POST /focus/allow` — add an allow rule from the block page.
///
/// Gated on the nonce rather than the bearer token: the browser cannot
/// present the token, but it *can* echo back a value it read out of the page
/// it is displaying. See [`ServerState::page_nonce`].
async fn focus_allow_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AllowRequest>,
) -> Response {
    let provided = headers
        .get(FOCUS_NONCE_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(provided.as_bytes(), state.page_nonce.as_bytes()) {
        return err_response(StatusCode::FORBIDDEN, "neplatný požadavek");
    }

    let Some(app_state) = state.app.try_state::<AppState>() else {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, "app state missing");
    };

    // Same validation the Settings panel goes through, so a rule added here
    // is indistinguishable from one typed in the app.
    let normalized =
        crate::commands::focus::normalize_rule_input("site", "allow", &req.pattern, "hide");
    let (kind, mode, pattern, action) = match normalized {
        Ok(v) => v,
        Err(e) => return err_response(StatusCode::BAD_REQUEST, e),
    };

    let insert = crate::cache::focus::upsert(
        &app_state.db,
        crate::cache::focus::NewFocusRule {
            kind: &kind,
            mode: &mode,
            pattern: &pattern,
            label: None,
            action: &action,
            enabled: true,
        },
        Utc::now().timestamp(),
    );
    if let Err(e) = insert {
        return err_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string());
    }

    // Wakes the extension's long-poll and invalidates the engine's rule
    // snapshot, so the page the user is looking at unblocks immediately.
    crate::focus::engine::bump_generation(&state.app, &app_state);

    Json(serde_json::json!({ "ok": true, "pattern": pattern })).into_response()
}

/// One "go here instead" tile on the block page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedTile {
    pub label: String,
    pub url: String,
}

/// Minimal HTML entity escaping. The block page interpolates a URL the user
/// was redirected from, which is attacker-influenced in the sense that any
/// page can link to a nasty URL and get it echoed back here.
pub fn html_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Make a JSON literal safe to sit inside a `<script>` element.
///
/// JSON encoding alone is not enough: `serde_json` leaves `<` and `>` intact,
/// so a URL containing `</script>` would close the block and everything after
/// it would be parsed as markup. The HTML parser doesn't look inside string
/// escapes, and `<` is the same character to JavaScript — so escaping the
/// three markup-significant characters closes the hole without changing the
/// value. U+2028/U+2029 are escaped too; they terminate lines in older
/// JavaScript parsers.
pub fn escape_json_for_script(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Tiles for the allow-listed sites, so a blocked tab offers somewhere useful
/// to go instead of being a dead end.
pub fn blocked_tiles(app_state: &AppState) -> Vec<BlockedTile> {
    crate::cache::focus::list_enabled(&app_state.db)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.kind == "site" && r.mode == "allow")
        .map(|r| BlockedTile {
            label: r.label.unwrap_or_else(|| r.pattern.clone()),
            url: format!("https://{}", r.pattern),
        })
        .collect()
}

/// Colours the block page should paint itself with, so it matches whatever
/// palette and theme the desktop is currently showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTheme {
    /// Primary accent, `#RRGGBB`.
    pub accent: String,
    /// `"auto"`, `"light"` or `"dark"` — mirrors the app's theme preference.
    pub mode: String,
}

impl Default for PageTheme {
    fn default() -> Self {
        // Aurora teal: the default palette's primary, so an install that has
        // never touched the palette picker looks unchanged.
        Self {
            accent: "#14B8A6".to_string(),
            mode: "auto".to_string(),
        }
    }
}

impl PageTheme {
    /// Read the active palette + theme out of `app_settings`.
    pub fn load(app_state: &AppState) -> Self {
        let defaults = Self::default();
        let (primary, _secondary) = crate::commands::prefs::get_accent_hex_inner(&app_state.db);
        let mode = crate::commands::prefs::get_theme_inner(&app_state.db)
            .unwrap_or_else(|_| defaults.mode.clone());
        Self {
            accent: primary.unwrap_or(defaults.accent),
            mode,
        }
    }

    /// `rgba(r, g, b, alpha)` derived from the accent, for tinted surfaces.
    ///
    /// The accent is validated as `#RRGGBB` before it is stored, so parsing
    /// cannot fail here — but a bad value would land in a stylesheet, so it
    /// falls back rather than unwrapping.
    pub fn accent_rgba(&self, alpha: f32) -> String {
        let body = self.accent.trim_start_matches('#');
        let parse = |i: usize| u8::from_str_radix(&body[i..i + 2], 16).ok();
        match (body.len() == 6)
            .then(|| (parse(0), parse(2), parse(4)))
            .and_then(|(r, g, b)| Some((r?, g?, b?)))
        {
            Some((r, g, b)) => format!("rgba({r}, {g}, {b}, {alpha})"),
            None => format!("rgba(20, 184, 166, {alpha})"),
        }
    }

    /// The surface palette, either as a `prefers-color-scheme` pair (auto) or
    /// pinned to the one the user chose.
    fn surface_css(&self) -> String {
        let light = "--bg: #f4f5f7; --surface: #ffffff; --text: #17181c; --muted: #6b7280; --border: #e3e5e9;";
        let dark = "--bg: #121316; --surface: #1c1e22; --text: #ecedef; --muted: #9aa1ab; --border: #2b2e34;";
        match self.mode.as_str() {
            "light" => format!(":root {{ color-scheme: light; {light} }}"),
            "dark" => format!(":root {{ color-scheme: dark; {dark} }}"),
            _ => format!(
                ":root {{ color-scheme: light dark; {light} }}\n  @media (prefers-color-scheme: dark) {{ :root {{ {dark} }} }}"
            ),
        }
    }
}

/// Render the block page.
///
/// Server-rendered rather than a static page plus a JSON API: the allow-list
/// is the user's own browsing profile, and a public endpoint serving it would
/// hand that list to every page running on the machine.
pub fn render_blocked_page(
    original_url: Option<&str>,
    ends_at: Option<i64>,
    tiles: &[BlockedTile],
    theme: &PageTheme,
    nonce: &str,
) -> String {
    let host = original_url
        .and_then(|u| crate::focus::rules::split_url(u).map(|(host, _)| host))
        .unwrap_or_default();
    let host_line = if host.is_empty() {
        String::new()
    } else {
        format!(
            "<p class=\"host\">Stránka <strong>{}</strong> je teď zablokovaná.</p>",
            html_escape(&host)
        )
    };

    // Deliberately understated: a text link, not a button. Un-blocking should
    // be available without being the thing the page invites you to do. Only
    // offered when we know which host to prefill.
    let allow_html = if host.is_empty() {
        String::new()
    } else {
        // Only worth suggesting the broader form when it differs from what is
        // already in the box — telling someone who blocked `qadata.cz` to
        // write `qadata.cz` reads like a bug.
        let broader = host.trim_start_matches("www.");
        let hint = if broader == host {
            "Uloží se jako trvalé pravidlo.".to_string()
        } else {
            format!(
                "Uloží se jako trvalé pravidlo. Pro celý web napište {}",
                html_escape(broader)
            )
        };
        format!(
            "<div class=\"allow\">\
             <button type=\"button\" id=\"allow-toggle\" class=\"linkish\">Povolit tuto stránku</button>\
             <div id=\"allow-panel\" hidden>\
             <form id=\"allow-form\">\
             <input id=\"allow-pattern\" value=\"{host}\" spellcheck=\"false\" autocapitalize=\"off\" autocomplete=\"off\">\
             <button type=\"submit\" class=\"allow-go\">Povolit</button>\
             </form>\
             <p id=\"allow-msg\" class=\"allow-msg\"></p>\
             <p class=\"allow-hint\">{hint}</p>\
             </div></div>",
            host = html_escape(&host),
            hint = hint,
        )
    };

    let countdown = match ends_at {
        Some(ts) => format!(
            "<p class=\"countdown\" data-ends-at=\"{ts}\">Zbývá <span id=\"remaining\">…</span></p>"
        ),
        None => "<p class=\"countdown\">Běží, dokud ho nezastavíte.</p>".to_string(),
    };

    let tiles_html = if tiles.is_empty() {
        String::new()
    } else {
        let items: String = tiles
            .iter()
            .map(|tile| {
                let initial = tile
                    .label
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into());
                format!(
                    "<a class=\"tile\" href=\"{url}\"><span class=\"mono\">{initial}</span>\
                     <span class=\"tile-label\">{label}</span></a>",
                    url = html_escape(&tile.url),
                    initial = html_escape(&initial),
                    label = html_escape(&tile.label),
                )
            })
            .collect();
        format!("<h2>Kam můžete</h2><div class=\"tiles\">{items}</div>")
    };

    let return_target = original_url
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .and_then(|u| serde_json::to_string(u).ok())
        .map(|json| escape_json_for_script(&json))
        .unwrap_or_else(|| "null".to_string());

    let nonce_json = serde_json::to_string(nonce)
        .map(|j| escape_json_for_script(&j))
        .unwrap_or_else(|_| "\"\"".to_string());
    let surface_css = theme.surface_css();
    let accent = html_escape(&theme.accent);
    let accent_soft = theme.accent_rgba(0.15);

    format!(
        r#"<!doctype html>
<html lang="cs">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Focus mode je aktivní — Tracker</title>
<style>
  {surface_css}
  /* Accent comes from the desktop's active palette, so the block page and the
     app are visibly the same product. */
  :root {{ --accent: {accent}; --accent-soft: {accent_soft}; }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; min-height: 100vh; display: flex; align-items: center;
    justify-content: center; padding: 32px; background: var(--bg);
    color: var(--text);
    font: 15px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  }}
  main {{
    width: 100%; max-width: 520px; background: var(--surface);
    border: 1px solid var(--border); border-radius: 18px; padding: 40px 36px;
    box-shadow: 0 10px 30px rgba(0,0,0,.07), 0 1px 2px rgba(0,0,0,.05);
    text-align: center;
  }}
  .brand {{
    font-style: italic; font-weight: 600; font-size: 24px; color: var(--accent);
    margin: 0 0 22px; letter-spacing: -0.01em;
  }}
  .badge {{
    width: 46px; height: 46px; margin: 0 auto 16px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    background: var(--accent-soft); color: var(--accent);
  }}
  h1 {{
    font-size: 20px; font-weight: 600; letter-spacing: -0.01em;
    margin: 0 0 10px;
  }}
  .host, .countdown {{ margin: 0 0 6px; color: var(--muted); font-size: 14px; }}
  h2 {{
    font-size: 12px; text-transform: uppercase; letter-spacing: .06em;
    color: var(--muted); margin: 28px 0 12px;
  }}
  /* `auto-fit` + `justify-content: center` so a couple of tiles sit in the
     middle instead of hugging the left edge of the card. */
  .tiles {{
    display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 200px));
    gap: 10px; justify-content: center;
  }}
  .tile {{
    display: flex; align-items: center; justify-content: center; gap: 10px; padding: 10px 12px;
    border: 1px solid var(--border); border-radius: 10px; text-decoration: none;
    color: var(--text); background: var(--bg); overflow: hidden;
  }}
  .tile {{ transition: border-color .15s, transform .15s; }}
  .tile:hover {{ border-color: var(--accent); transform: translateY(-1px); }}
  .mono {{
    flex: 0 0 28px; height: 28px; border-radius: 8px; background: var(--accent);
    color: #fff; display: flex; align-items: center; justify-content: center;
    font-weight: 600; font-size: 13px;
  }}
  .tile-label {{ font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  footer {{ margin-top: 24px; font-size: 12px; color: var(--muted); }}
  .allow {{ margin-top: 24px; padding-top: 20px; border-top: 1px solid var(--border); }}
  .linkish {{
    background: none; border: 0; padding: 0; font: inherit; font-size: 12px;
    color: var(--muted); text-decoration: underline; cursor: pointer;
  }}
  .linkish:hover {{ color: var(--accent); }}
  #allow-form {{
    display: flex; gap: 8px; justify-content: center; margin-top: 12px;
  }}
  #allow-pattern {{
    flex: 0 1 260px; padding: 7px 10px; border-radius: 8px; font: inherit;
    font-size: 13px; background: var(--bg); color: var(--text);
    border: 1px solid var(--border);
  }}
  #allow-pattern:focus {{ outline: none; border-color: var(--accent); }}
  .allow-go {{
    padding: 7px 14px; border-radius: 8px; border: 0; cursor: pointer;
    font: inherit; font-size: 13px; font-weight: 500;
    background: var(--accent); color: #fff; transition: filter .15s;
  }}
  .allow-go:hover {{ filter: brightness(1.08); }}
  .allow-msg {{ margin: 10px 0 0; font-size: 12px; color: var(--accent); min-height: 1em; }}
  .allow-hint {{ margin: 6px 0 0; font-size: 11px; color: var(--muted); }}
</style>
</head>
<body>
<main>
  <p class="brand">Tracker</p>
  <div class="badge" aria-hidden>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
         stroke-linecap="round" stroke-linejoin="round" width="22" height="22">
      <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/>
      <path d="m4.24 5.21 14.39 12.47"/>
    </svg>
  </div>
  <h1>Focus mode je aktivní</h1>
  {host_line}
  {countdown}
  {tiles_html}
  <footer>Až Focus skončí, stránka se sama vrátí zpět.</footer>
  {allow_html}
</main>
<script>
  var RETURN_TO = {return_target};
  var el = document.querySelector('.countdown[data-ends-at]');
  if (el) {{
    var endsAt = parseInt(el.getAttribute('data-ends-at'), 10) * 1000;
    var out = document.getElementById('remaining');
    var render = function () {{
      var left = Math.max(0, Math.round((endsAt - Date.now()) / 1000));
      var h = Math.floor(left / 3600), m = Math.floor((left % 3600) / 60), s = left % 60;
      out.textContent = (h ? h + ' h ' : '') + m + ' min ' + s + ' s';
    }};
    render();
    setInterval(render, 1000);
  }}
  var NONCE = {nonce_json};
  var toggle = document.getElementById('allow-toggle');
  var form = document.getElementById('allow-form');
  var panel = document.getElementById('allow-panel');
  if (toggle && form && panel) {{
    toggle.addEventListener('click', function () {{
      panel.hidden = !panel.hidden;
      toggle.setAttribute('aria-expanded', String(!panel.hidden));
      if (!panel.hidden) {{
        var input = document.getElementById('allow-pattern');
        input.focus();
        input.select();
      }}
    }});
    form.addEventListener('submit', function (e) {{
      e.preventDefault();
      var msg = document.getElementById('allow-msg');
      msg.textContent = 'Ukládám…';
      fetch('/focus/allow', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json', 'X-Focus-Nonce': NONCE }},
        body: JSON.stringify({{ pattern: document.getElementById('allow-pattern').value }})
      }})
        .then(function (r) {{ return r.json().then(function (d) {{ return {{ ok: r.ok, data: d }}; }}); }})
        .then(function (res) {{
          if (!res.ok) {{ msg.textContent = (res.data && res.data.error) || 'Nepodařilo se povolit.'; return; }}
          msg.textContent = 'Povoleno: ' + res.data.pattern;
          if (RETURN_TO) {{ setTimeout(function () {{ location.replace(RETURN_TO); }}, 500); }}
        }})
        .catch(function () {{ msg.textContent = 'Tracker neodpovídá.'; }});
    }});
  }}

  setInterval(function () {{
    fetch('/focus/ping', {{ cache: 'no-store' }})
      .then(function (r) {{ return r.json(); }})
      .then(function (data) {{
        if (!data.active) {{
          if (RETURN_TO) {{ location.replace(RETURN_TO); }} else {{ history.back(); }}
        }}
      }})
      .catch(function () {{ /* Tracker stopped — the extension clears its rules */ }});
  }}, 5000);
</script>
</body>
</html>"#
    )
}

/// `GET /blocked?u=<original url>`.
///
/// The `u` value is read straight off the raw query string rather than
/// through a parser, because the extension's `regexSubstitution` splices the
/// original URL in unencoded — query delimiters and all.
async fn blocked_page_handler<R: Runtime>(
    State(state): State<ServerState<R>>,
    RawQuery(raw): RawQuery,
) -> Response {
    let original = raw.as_deref().and_then(extract_u_param);
    let (ends_at, tiles, theme) = match state.app.try_state::<AppState>() {
        Some(app_state) => {
            let ends_at = app_state
                .focus
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .ends_at;
            (
                ends_at,
                blocked_tiles(&app_state),
                PageTheme::load(&app_state),
            )
        }
        None => (None, Vec::new(), PageTheme::default()),
    };

    let html = render_blocked_page(
        original.as_deref(),
        ends_at,
        &tiles,
        &theme,
        &state.page_nonce,
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

/// Pull the `u` parameter out of a raw query string, taking everything after
/// it verbatim. Returns `None` for anything that isn't an http(s) URL.
pub fn extract_u_param(raw_query: &str) -> Option<String> {
    let rest = if let Some(stripped) = raw_query.strip_prefix("u=") {
        stripped
    } else {
        let idx = raw_query.find("&u=")?;
        &raw_query[idx + 3..]
    };
    let decoded = crate::focus::percent_decode(rest);
    if decoded.starts_with("http://") || decoded.starts_with("https://") {
        Some(decoded)
    } else {
        None
    }
}

// -----------------------------------------------------------------------------
// Helpers.
// -----------------------------------------------------------------------------

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

#[cfg(test)]
mod focus_tests {
    use super::*;

    #[test]
    fn u_param_is_taken_verbatim_including_its_own_query() {
        assert_eq!(
            extract_u_param("u=https://reddit.com/r/x?sort=new&t=all"),
            Some("https://reddit.com/r/x?sort=new&t=all".to_string())
        );
    }

    #[test]
    fn u_param_can_follow_another_parameter() {
        assert_eq!(
            extract_u_param("from=ext&u=https://x.com/a"),
            Some("https://x.com/a".to_string())
        );
    }

    #[test]
    fn percent_encoded_u_param_is_decoded() {
        assert_eq!(
            extract_u_param("u=https%3A%2F%2Fx.com%2Fa%20b"),
            Some("https://x.com/a b".to_string())
        );
    }

    #[test]
    fn non_http_u_param_is_rejected() {
        assert_eq!(extract_u_param("u=javascript:alert(1)"), None);
        assert_eq!(extract_u_param("u=file:///etc/passwd"), None);
        assert_eq!(extract_u_param("other=1"), None);
    }

    #[test]
    fn html_escaping_neutralises_markup() {
        assert_eq!(
            html_escape("<script>\"x\"&'y'</script>"),
            "&lt;script&gt;&quot;x&quot;&amp;&#39;y&#39;&lt;/script&gt;"
        );
    }

    #[test]
    fn rendered_page_escapes_the_host_it_echoes_back() {
        let html = render_blocked_page(
            Some("https://x.com/<img src=x onerror=alert(1)>"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("x.com"));
    }

    #[test]
    fn rendered_page_embeds_the_return_url_as_json() {
        let html = render_blocked_page(
            Some("https://x.com/a\"b"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(html.contains(r#"var RETURN_TO = "https://x.com/a\"b""#));
    }

    #[test]
    fn return_url_cannot_close_the_script_element() {
        let html = render_blocked_page(
            Some("https://x.com/</script><img src=x onerror=alert(1)>"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(!html.contains("</script><img"));
        assert!(html.contains("\\u003c/script\\u003e"));
        // Exactly one real closing tag: the one we wrote.
        assert_eq!(html.matches("</script>").count(), 1);
    }

    #[test]
    fn rendered_page_refuses_a_non_http_return_url() {
        let html = render_blocked_page(
            Some("javascript:alert(1)"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(html.contains("var RETURN_TO = null"));
    }

    #[test]
    fn the_page_paints_itself_with_the_active_accent() {
        let theme = PageTheme {
            accent: "#EAB308".into(),
            mode: "auto".into(),
        };
        let html = render_blocked_page(Some("https://reddit.com"), None, &[], &theme, "test-nonce");
        assert!(html.contains("--accent: #EAB308"));
        assert!(html.contains("rgba(234, 179, 8, 0.15)"));
    }

    #[test]
    fn an_explicit_theme_pins_the_surface_instead_of_asking_the_browser() {
        let dark = PageTheme {
            mode: "dark".into(),
            ..PageTheme::default()
        };
        let html = render_blocked_page(None, None, &[], &dark, "test-nonce");
        assert!(html.contains("color-scheme: dark"));
        assert!(
            !html.contains("prefers-color-scheme"),
            "a pinned theme must not defer to the OS"
        );

        let auto = PageTheme::default();
        let html = render_blocked_page(None, None, &[], &auto, "test-nonce");
        assert!(html.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn a_malformed_accent_degrades_instead_of_emitting_broken_css() {
        let theme = PageTheme {
            accent: "not-a-colour".into(),
            mode: "auto".into(),
        };
        assert_eq!(theme.accent_rgba(0.15), "rgba(20, 184, 166, 0.15)");
    }

    #[test]
    fn the_allow_control_is_prefilled_with_the_blocked_host() {
        let html = render_blocked_page(
            Some("https://www.qadata.cz/x"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(html.contains("id=\"allow-pattern\" value=\"www.qadata.cz\""));
        // The hint points at the domain, which is what covers the whole site.
        assert!(html.contains("Pro celý web napište qadata.cz"));
    }

    #[test]
    fn a_domain_is_not_told_to_write_itself() {
        let html = render_blocked_page(
            Some("https://qadata.cz/"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(html.contains("Uloží se jako trvalé pravidlo."));
        assert!(
            !html.contains("Pro celý web"),
            "the box already holds the broadest form"
        );
    }

    #[test]
    fn a_domain_reaches_the_extension_as_an_explicit_wildcard() {
        // The browser half only understands `*.`; the desktop decides.
        assert_eq!(expand_site_pattern("seznam.cz"), "*.seznam.cz");
        assert_eq!(expand_site_pattern("*.seznam.cz"), "*.seznam.cz");
    }

    #[test]
    fn a_named_host_reaches_the_extension_unchanged() {
        assert_eq!(expand_site_pattern("www.seznam.cz"), "www.seznam.cz");
        assert_eq!(
            expand_site_pattern("reddit.com/r/rust"),
            "*.reddit.com/r/rust"
        );
    }

    #[test]
    fn an_unparseable_pattern_is_passed_through_for_the_extension_to_drop() {
        assert_eq!(expand_site_pattern("nonsense"), "nonsense");
    }

    #[test]
    fn only_the_trigger_shows_before_the_user_asks_to_allow() {
        let html = render_blocked_page(
            Some("https://www.qadata.cz/"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        // `class="…"` / `id="…"` rather than the bare word: the stylesheet
        // mentions every one of these selectors before the markup does.
        let toggle = html.find("id=\"allow-toggle\"").expect("trigger");
        let panel = html.find("id=\"allow-panel\" hidden").expect("panel");
        let input = html.find("id=\"allow-pattern\"").expect("input");
        let hint = html.find("class=\"allow-hint\"").expect("hint");

        assert!(toggle < panel, "the trigger is what shows first");
        // The input and the note both have to sit inside the collapsed panel;
        // the note used to hang outside it and was visible from the start.
        assert!(input > panel, "input must be inside the panel");
        assert!(hint > panel, "note must be inside the panel");
    }

    #[test]
    fn the_allow_control_carries_the_nonce_the_endpoint_demands() {
        let html = render_blocked_page(
            Some("https://reddit.com"),
            None,
            &[],
            &PageTheme::default(),
            "s3cr3t-nonce",
        );
        assert!(html.contains("var NONCE = \"s3cr3t-nonce\""));
        assert!(html.contains("X-Focus-Nonce"));
    }

    #[test]
    fn a_page_without_a_known_host_offers_nothing_to_allow() {
        let html = render_blocked_page(None, None, &[], &PageTheme::default(), "test-nonce");
        // The stylesheet always carries the selector; it is the markup that
        // must be absent.
        assert!(!html.contains("id=\"allow-pattern\""));
        assert!(!html.contains("id=\"allow-toggle\""));
    }

    #[test]
    fn a_hostile_host_cannot_break_out_of_the_prefilled_input() {
        // The quote lands in the authority, so it reaches the `value`
        // attribute rather than the path.
        let html = render_blocked_page(
            Some("https://x.com\" onfocus=alert(1) z=\"/"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(
            !html.contains("value=\"x.com\" onfocus="),
            "the quote must not close the attribute"
        );
        assert!(html.contains("&quot;"), "it should arrive escaped instead");
    }

    #[test]
    fn tiles_render_a_link_per_allowed_site() {
        let tiles = vec![
            BlockedTile {
                label: "Jira".into(),
                url: "https://team.atlassian.net".into(),
            },
            BlockedTile {
                label: "Docs".into(),
                url: "https://docs.rs".into(),
            },
        ];
        let html = render_blocked_page(
            Some("https://reddit.com"),
            Some(1_700_000_000),
            &tiles,
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(html.contains("https://team.atlassian.net"));
        assert!(html.contains("https://docs.rs"));
        assert!(html.contains("data-ends-at=\"1700000000\""));
    }

    #[test]
    fn page_without_tiles_omits_the_section() {
        let html = render_blocked_page(
            Some("https://reddit.com"),
            None,
            &[],
            &PageTheme::default(),
            "test-nonce",
        );
        assert!(!html.contains("Kam můžete"));
        assert!(html.contains("Focus mode je aktivní"));
    }
}
