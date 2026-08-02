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

    let collect = |mode: &str| -> Vec<String> {
        rules
            .iter()
            .filter(|r| r.kind == "site" && r.mode == mode)
            .map(|r| r.pattern.clone())
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

/// Render the block page.
///
/// Server-rendered rather than a static page plus a JSON API: the allow-list
/// is the user's own browsing profile, and a public endpoint serving it would
/// hand that list to every page running on the machine.
pub fn render_blocked_page(
    original_url: Option<&str>,
    ends_at: Option<i64>,
    tiles: &[BlockedTile],
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

    format!(
        r#"<!doctype html>
<html lang="cs">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Focus mode je aktivní — Tracker</title>
<style>
  :root {{
    color-scheme: light dark;
    --bg: #f4f5f7; --surface: #ffffff; --text: #17181c; --muted: #6b7280;
    --border: #e3e5e9; --accent: #0f766e;
  }}
  @media (prefers-color-scheme: dark) {{
    :root {{
      --bg: #121316; --surface: #1c1e22; --text: #ecedef; --muted: #9aa1ab;
      --border: #2b2e34; --accent: #2dd4bf;
    }}
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; min-height: 100vh; display: flex; align-items: center;
    justify-content: center; padding: 32px; background: var(--bg);
    color: var(--text);
    font: 15px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  }}
  main {{
    width: 100%; max-width: 560px; background: var(--surface);
    border: 1px solid var(--border); border-radius: 16px; padding: 36px 32px;
    box-shadow: 0 1px 3px rgba(0,0,0,.08);
    text-align: center;
  }}
  .brand {{
    font-style: italic; font-weight: 600; font-size: 26px; color: var(--accent);
    margin: 0 0 20px;
  }}
  h1 {{ font-size: 22px; margin: 0 0 8px; }}
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
  .tile:hover {{ border-color: var(--accent); }}
  .mono {{
    flex: 0 0 28px; height: 28px; border-radius: 8px; background: var(--accent);
    color: #fff; display: flex; align-items: center; justify-content: center;
    font-weight: 600; font-size: 13px;
  }}
  .tile-label {{ font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  footer {{ margin-top: 28px; font-size: 12px; color: var(--muted); }}
</style>
</head>
<body>
<main>
  <p class="brand">Tracker</p>
  <h1>Focus mode je aktivní</h1>
  {host_line}
  {countdown}
  {tiles_html}
  <footer>Až Focus skončí, stránka se sama vrátí zpět.</footer>
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
    let (ends_at, tiles) = match state.app.try_state::<AppState>() {
        Some(app_state) => {
            let ends_at = app_state
                .focus
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .ends_at;
            (ends_at, blocked_tiles(&app_state))
        }
        None => (None, Vec::new()),
    };

    let html = render_blocked_page(original.as_deref(), ends_at, &tiles);
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
        );
        assert!(!html.contains("<img src=x"));
        assert!(html.contains("x.com"));
    }

    #[test]
    fn rendered_page_embeds_the_return_url_as_json() {
        let html = render_blocked_page(Some("https://x.com/a\"b"), None, &[]);
        assert!(html.contains(r#"var RETURN_TO = "https://x.com/a\"b""#));
    }

    #[test]
    fn return_url_cannot_close_the_script_element() {
        let html = render_blocked_page(
            Some("https://x.com/</script><img src=x onerror=alert(1)>"),
            None,
            &[],
        );
        assert!(!html.contains("</script><img"));
        assert!(html.contains("\\u003c/script\\u003e"));
        // Exactly one real closing tag: the one we wrote.
        assert_eq!(html.matches("</script>").count(), 1);
    }

    #[test]
    fn rendered_page_refuses_a_non_http_return_url() {
        let html = render_blocked_page(Some("javascript:alert(1)"), None, &[]);
        assert!(html.contains("var RETURN_TO = null"));
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
        let html = render_blocked_page(Some("https://reddit.com"), Some(1_700_000_000), &tiles);
        assert!(html.contains("https://team.atlassian.net"));
        assert!(html.contains("https://docs.rs"));
        assert!(html.contains("data-ends-at=\"1700000000\""));
    }

    #[test]
    fn page_without_tiles_omits_the_section() {
        let html = render_blocked_page(Some("https://reddit.com"), None, &[]);
        assert!(!html.contains("Kam můžete"));
        assert!(html.contains("Focus mode je aktivní"));
    }
}
