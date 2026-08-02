//! Session lifecycle and the enforcement loop.
//!
//! One background task runs for the life of the process. While no session is
//! active it wakes rarely and does nothing; while a session runs it ticks once
//! a second and enforces the rules against whatever the user just brought to
//! the foreground.
//!
//! **The safety property that makes this tolerable:** enforcement only ever
//! looks at the *foreground* application. It never walks the process table
//! hunting for things to close, so an over-broad allow-list can annoy the user
//! but cannot take the machine down. The single exception is the opt-in kill
//! sweep at session start, which matches explicit `block` + `kill` rules only
//! and never runs in strict mode.

use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::rules::{AppAction, AppDecision, SiteDecision};
use super::{apps, notify, overlay, rules};
use crate::cache::{self, focus::FocusRuleRow, Db};
use crate::state::AppState;

// -----------------------------------------------------------------------------
// Settings keys
// -----------------------------------------------------------------------------

pub const KEY_STRICT_APPS: &str = "focus_strict_apps";
pub const KEY_STRICT_SITES: &str = "focus_strict_sites";
pub const KEY_BLOCK_NOTIFICATIONS: &str = "focus_block_notifications";
pub const KEY_SHORTCUT_ON: &str = "focus_shortcut_on";
pub const KEY_SHORTCUT_OFF: &str = "focus_shortcut_off";
pub const KEY_DEFAULT_DURATION: &str = "focus_default_duration_min";
/// Unix seconds at which a session started before the app was closed should
/// end. Lets a running session survive a restart instead of silently lapsing.
pub const KEY_ACTIVE_UNTIL: &str = "focus_active_until";

/// Session length used when the caller doesn't pass one. Zero means "until I
/// stop it".
pub const DEFAULT_DURATION_MIN: i64 = 0;

/// How often the loop wakes while a session is running.
const TICK_ACTIVE: Duration = Duration::from_secs(1);
/// How often it wakes while idle. Long enough to be free, short enough that a
/// session started from another window is picked up promptly.
const TICK_IDLE: Duration = Duration::from_secs(5);

// -----------------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------------

/// User-configured behaviour, persisted in `app_settings`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusSettings {
    /// Allow-list mode for applications: block anything not explicitly allowed.
    pub strict_apps: bool,
    /// Allow-list mode for websites.
    pub strict_sites: bool,
    pub block_notifications: bool,
    /// macOS Shortcut run when a session starts.
    pub shortcut_on: Option<String>,
    /// macOS Shortcut run when a session ends.
    pub shortcut_off: Option<String>,
    /// Minutes pre-filled in the start control. `0` = open-ended.
    pub default_duration_min: i64,
}

impl Default for FocusSettings {
    fn default() -> Self {
        Self {
            strict_apps: false,
            strict_sites: false,
            block_notifications: true,
            shortcut_on: None,
            shortcut_off: None,
            default_duration_min: DEFAULT_DURATION_MIN,
        }
    }
}

/// Live session state. Lives in [`AppState`] behind an `RwLock`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusRuntime {
    pub active: bool,
    /// Unix seconds.
    pub started_at: Option<i64>,
    /// Unix seconds, or `None` for an open-ended session.
    pub ends_at: Option<i64>,
    /// Bumped on every change to the session **or** the ruleset. The browser
    /// extension long-polls on this so it can install new rules immediately.
    pub generation: u64,
    /// Shortcut that must run to undo the notification silencing this session
    /// turned on, or `None` if it never turned any on.
    ///
    /// Captured at start instead of recomputed at stop: the user can flip the
    /// silencing toggle or rename the Shortcut while a session runs, and
    /// deriving the undo from current settings would then either skip it (and
    /// strand the Mac in Do Not Disturb) or run the wrong one. Not part of the
    /// wire shape — nothing outside this module needs it.
    #[serde(skip)]
    pub dnd_undo_shortcut: Option<String>,
}

/// Shortcut that undoes what [`start`] turned on, given the settings in force
/// at the time and whether the "on" Shortcut actually ran.
fn undo_shortcut_for(settings: &FocusSettings, engaged: bool) -> Option<String> {
    if !engaged {
        return None;
    }
    settings
        .shortcut_off
        .clone()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

/// What both the frontend and the browser extension see.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusStateDto {
    #[serde(flatten)]
    pub runtime: FocusRuntime,
    #[serde(flatten)]
    pub settings: FocusSettings,
}

// -----------------------------------------------------------------------------
// Settings persistence
// -----------------------------------------------------------------------------

fn read_bool(db: &Db, key: &str, default: bool) -> bool {
    match cache::settings::get(db, key) {
        Ok(Some(v)) => v == "1" || v.eq_ignore_ascii_case("true"),
        _ => default,
    }
}

fn read_string(db: &Db, key: &str) -> Option<String> {
    match cache::settings::get(db, key) {
        Ok(Some(v)) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

pub fn load_settings(db: &Db) -> FocusSettings {
    let defaults = FocusSettings::default();
    FocusSettings {
        strict_apps: read_bool(db, KEY_STRICT_APPS, defaults.strict_apps),
        strict_sites: read_bool(db, KEY_STRICT_SITES, defaults.strict_sites),
        block_notifications: read_bool(db, KEY_BLOCK_NOTIFICATIONS, defaults.block_notifications),
        shortcut_on: read_string(db, KEY_SHORTCUT_ON),
        shortcut_off: read_string(db, KEY_SHORTCUT_OFF),
        default_duration_min: read_string(db, KEY_DEFAULT_DURATION)
            .and_then(|v| v.parse().ok())
            .unwrap_or(defaults.default_duration_min)
            .max(0),
    }
}

pub fn save_settings(db: &Db, settings: &FocusSettings) -> Result<(), String> {
    let bool_str = |b: bool| if b { "1" } else { "0" };
    let write = |key: &str, value: &str| -> Result<(), String> {
        cache::settings::set(db, key, value).map_err(|e| e.to_string())
    };
    write(KEY_STRICT_APPS, bool_str(settings.strict_apps))?;
    write(KEY_STRICT_SITES, bool_str(settings.strict_sites))?;
    write(
        KEY_BLOCK_NOTIFICATIONS,
        bool_str(settings.block_notifications),
    )?;
    write(
        KEY_DEFAULT_DURATION,
        &settings.default_duration_min.max(0).to_string(),
    )?;
    for (key, value) in [
        (KEY_SHORTCUT_ON, settings.shortcut_on.as_deref()),
        (KEY_SHORTCUT_OFF, settings.shortcut_off.as_deref()),
    ] {
        match value.map(str::trim).filter(|v| !v.is_empty()) {
            Some(v) => write(key, v)?,
            None => cache::settings::remove(db, key).map_err(|e| e.to_string())?,
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// State access
// -----------------------------------------------------------------------------

fn now_s() -> i64 {
    Utc::now().timestamp()
}

fn runtime_snapshot(state: &AppState) -> FocusRuntime {
    state
        .focus
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Read the current state as the frontend and extension see it.
pub fn state_dto(state: &AppState) -> FocusStateDto {
    FocusStateDto {
        runtime: runtime_snapshot(state),
        settings: load_settings(&state.db),
    }
}

/// `true` while a session is running. Used to gate Tracker's own
/// notifications.
pub fn is_active(state: &AppState) -> bool {
    runtime_snapshot(state).active
}

/// Bump the generation counter and wake every long-poller. Call after any
/// change to the session or the ruleset.
pub fn bump_generation<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    let dto = {
        let mut guard = state.focus.write().unwrap_or_else(|e| e.into_inner());
        guard.generation = guard.generation.wrapping_add(1);
        FocusStateDto {
            runtime: guard.clone(),
            settings: load_settings(&state.db),
        }
    };
    state.focus_notify.notify_waiters();
    let _ = app.emit("focus-changed", &dto);
}

// -----------------------------------------------------------------------------
// Session lifecycle
// -----------------------------------------------------------------------------

/// Start a session. `duration_min` of `None` or `0` means open-ended.
pub fn start<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    duration_min: Option<i64>,
) -> Result<FocusStateDto, String> {
    let settings = load_settings(&state.db);
    let now = now_s();
    let minutes = duration_min.unwrap_or(settings.default_duration_min).max(0);
    let ends_at = if minutes > 0 {
        Some(now + minutes * 60)
    } else {
        None
    };

    // Silence notifications before publishing the session, so the recorded
    // undo reflects what actually ran.
    let engaged =
        settings.block_notifications && notify::run_focus_shortcut(settings.shortcut_on.as_deref());
    if settings.block_notifications && !engaged {
        tracing::info!("focus: system Do Not Disturb not automated on this platform");
    }

    {
        let mut guard = state.focus.write().unwrap_or_else(|e| e.into_inner());
        guard.active = true;
        guard.started_at = Some(now);
        guard.ends_at = ends_at;
        guard.dnd_undo_shortcut = undo_shortcut_for(&settings, engaged);
    }
    // Persist so a crash or restart mid-session doesn't silently drop it.
    let _ = cache::settings::set(
        &state.db,
        KEY_ACTIVE_UNTIL,
        &ends_at.unwrap_or(0).to_string(),
    );
    overlay::reset_throttle();

    // Opt-in kill sweep: only apps with an explicit `block` + `kill` rule, and
    // never in strict mode where the allow-list would sweep far too widely.
    let enabled_rules = cache::focus::list_enabled(&state.db).unwrap_or_default();
    kill_sweep(&enabled_rules);

    bump_generation(app, state);
    Ok(state_dto(state))
}

/// End the session and undo everything it turned on.
pub fn stop<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<FocusStateDto, String> {
    let undo = {
        let mut guard = state.focus.write().unwrap_or_else(|e| e.into_inner());
        guard.active = false;
        guard.started_at = None;
        guard.ends_at = None;
        guard.dnd_undo_shortcut.take()
    };
    let _ = cache::settings::remove(&state.db, KEY_ACTIVE_UNTIL);

    // Undo exactly what this session turned on — see `dnd_undo_shortcut`.
    notify::run_focus_shortcut(undo.as_deref());
    overlay::hide(app);

    bump_generation(app, state);
    Ok(state_dto(state))
}

/// Restore a session that was running when the app last closed. A session
/// whose end time has already passed is discarded rather than resurrected.
pub fn restore<R: Runtime>(app: &AppHandle<R>, state: &AppState) {
    let Some(raw) = read_string(&state.db, KEY_ACTIVE_UNTIL) else {
        return;
    };
    let Ok(ends_at) = raw.parse::<i64>() else {
        let _ = cache::settings::remove(&state.db, KEY_ACTIVE_UNTIL);
        return;
    };
    let now = now_s();
    if ends_at != 0 && ends_at <= now {
        let _ = cache::settings::remove(&state.db, KEY_ACTIVE_UNTIL);
        return;
    }
    {
        let mut guard = state.focus.write().unwrap_or_else(|e| e.into_inner());
        guard.active = true;
        guard.started_at = Some(now);
        guard.ends_at = if ends_at == 0 { None } else { Some(ends_at) };
    }
    bump_generation(app, state);
    tracing::info!("focus: restored an in-progress session");
}

/// Terminate already-running apps that carry an explicit `block` + `kill`
/// rule. Safe-listed apps are skipped by [`rules::decide_app`].
fn kill_sweep(enabled_rules: &[FocusRuleRow]) {
    let wants_kill = enabled_rules
        .iter()
        .any(|r| r.kind == "app" && r.mode == "block" && r.action == "kill");
    if !wants_kill {
        return;
    }
    for app in apps::running_apps() {
        // `strict = false` on purpose: a sweep driven by an allow-list would
        // close everything the user forgot to whitelist.
        if let AppDecision::Blocked(AppAction::Kill) = rules::decide_app(&app, enabled_rules, false)
        {
            let name = app.display_name();
            if apps::kill_pid(app.pid) {
                tracing::info!("focus: terminated {name} at session start");
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Enforcement
// -----------------------------------------------------------------------------

/// One enforcement pass. Cheap and side-effect-free when nothing matches.
pub fn tick<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let snapshot = runtime_snapshot(&state);
    if !snapshot.active {
        return;
    }

    let now = now_s();
    if snapshot.ends_at.is_some_and(|end| now >= end) {
        let _ = stop(app, &state);
        return;
    }

    let settings = load_settings(&state.db);
    let enabled_rules = cache::focus::list_enabled(&state.db).unwrap_or_default();

    let Some(front) = apps::frontmost_app() else {
        return;
    };

    match rules::decide_app(&front, &enabled_rules, settings.strict_apps) {
        AppDecision::Blocked(action) => {
            enforce_app(app, &front, action);
            // The app is on its way out; inspecting its tabs would be moot.
            return;
        }
        AppDecision::Allowed => {}
    }

    enforce_browser(&front, &enabled_rules, settings.strict_sites);
}

fn enforce_app<R: Runtime>(app: &AppHandle<R>, ident: &rules::AppIdent, action: AppAction) {
    let name = ident.display_name();
    // A kill we lack the rights for (elevated process, another user) falls
    // back to hiding rather than silently doing nothing.
    let killed = matches!(action, AppAction::Kill) && apps::kill_pid(ident.pid);
    if !killed {
        apps::hide_pid(ident.pid);
    }
    overlay::flash(
        app,
        overlay::OverlayNotice {
            app_name: name,
            killed,
        },
        Utc::now().timestamp_millis(),
    );
}

/// macOS only: if the foreground app is a browser we can drive, check its
/// front tab and rewrite it when blocked. Everywhere else this is the
/// extension's job.
fn enforce_browser(front: &rules::AppIdent, enabled_rules: &[FocusRuleRow], strict_sites: bool) {
    let Some(bundle) = front.bundle_id.as_deref() else {
        return;
    };
    let Some(browser) = super::browsers::browser_for_bundle(bundle) else {
        return;
    };
    let Some(url) = super::browsers::read_active_url(browser) else {
        return;
    };
    // Never redirect our own block page — that would loop forever.
    if super::is_blocked_page(&url) {
        return;
    }
    if rules::decide_site(&url, enabled_rules, strict_sites) != SiteDecision::Blocked {
        return;
    }
    let target = super::blocked_page_url(&url);
    if !super::browsers::set_active_url(browser, &target) {
        tracing::debug!("focus: could not rewrite {} tab", browser.app_name);
    }
}

// -----------------------------------------------------------------------------
// Background loop
// -----------------------------------------------------------------------------

/// Start the enforcement loop. Call once from `setup`.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let active = app
                .try_state::<AppState>()
                .map(|state| runtime_snapshot(&state).active)
                .unwrap_or(false);

            if active {
                // Enforcement blocks on OS calls (and on `osascript`), so keep
                // it off the async worker threads.
                let handle = app.clone();
                let joined = tauri::async_runtime::spawn_blocking(move || tick(&handle)).await;
                if let Err(e) = joined {
                    tracing::warn!("focus: enforcement tick panicked: {e}");
                }
            }

            tokio::time::sleep(if active { TICK_ACTIVE } else { TICK_IDLE }).await;
        }
    });
}

/// Called on the way out so a session doesn't leave Do Not Disturb switched on
/// after Tracker quits. The session itself stays persisted — restarting picks
/// it back up via [`restore`].
pub fn shutdown(state: &AppState) {
    let undo = {
        let mut guard = state.focus.write().unwrap_or_else(|e| e.into_inner());
        guard.dnd_undo_shortcut.take()
    };
    notify::run_focus_shortcut(undo.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("focus-engine.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn settings_default_when_nothing_is_stored() {
        let db = open_db();
        assert_eq!(load_settings(&db), FocusSettings::default());
    }

    #[test]
    fn settings_round_trip() {
        let db = open_db();
        let settings = FocusSettings {
            strict_apps: true,
            strict_sites: true,
            block_notifications: false,
            shortcut_on: Some("Focus On".into()),
            shortcut_off: Some("Focus Off".into()),
            default_duration_min: 50,
        };
        save_settings(&db, &settings).unwrap();
        assert_eq!(load_settings(&db), settings);
    }

    #[test]
    fn blank_shortcut_names_are_cleared_not_stored() {
        let db = open_db();
        save_settings(
            &db,
            &FocusSettings {
                shortcut_on: Some("Focus On".into()),
                ..FocusSettings::default()
            },
        )
        .unwrap();
        save_settings(
            &db,
            &FocusSettings {
                shortcut_on: Some("   ".into()),
                ..FocusSettings::default()
            },
        )
        .unwrap();
        assert_eq!(load_settings(&db).shortcut_on, None);
    }

    #[test]
    fn undo_shortcut_is_none_when_nothing_was_engaged() {
        let settings = FocusSettings {
            shortcut_off: Some("Focus Off".into()),
            ..FocusSettings::default()
        };
        assert_eq!(undo_shortcut_for(&settings, false), None);
    }

    #[test]
    fn undo_shortcut_is_captured_when_silencing_engaged() {
        let settings = FocusSettings {
            shortcut_off: Some("  Focus Off  ".into()),
            ..FocusSettings::default()
        };
        assert_eq!(
            undo_shortcut_for(&settings, true),
            Some("Focus Off".to_string())
        );
    }

    #[test]
    fn undo_shortcut_ignores_a_blank_name() {
        let settings = FocusSettings {
            shortcut_off: Some("   ".into()),
            ..FocusSettings::default()
        };
        assert_eq!(undo_shortcut_for(&settings, true), None);
    }

    #[test]
    fn negative_duration_is_clamped_to_open_ended() {
        let db = open_db();
        save_settings(
            &db,
            &FocusSettings {
                default_duration_min: -30,
                ..FocusSettings::default()
            },
        )
        .unwrap();
        assert_eq!(load_settings(&db).default_duration_min, 0);
    }
}
