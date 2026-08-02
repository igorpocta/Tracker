//! The "that app is blocked right now" banner.
//!
//! A small, click-through window pinned near the top of the primary monitor.
//! It is deliberately not a full-screen blocker: the enforcement already
//! happened (the app was hidden or terminated), so this only needs to explain
//! *why* the window the user just clicked vanished.
//!
//! The window is created lazily on first use and then reused — building a
//! webview costs far more than showing one.

use std::sync::OnceLock;

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Runtime, WebviewUrl,
    WebviewWindowBuilder,
};

pub const OVERLAY_LABEL: &str = "focus-overlay";

const OVERLAY_WIDTH: f64 = 360.0;
const OVERLAY_HEIGHT: f64 = 92.0;
/// Gap below the top edge of the screen (below the menu bar on macOS).
const OVERLAY_TOP_MARGIN: f64 = 48.0;
/// How long the banner stays up.
const OVERLAY_VISIBLE_MS: u64 = 2_500;
/// Don't re-flash for the same app more often than this — the engine ticks
/// every second and an app the user keeps clicking would strobe.
const FLASH_COOLDOWN_MS: i64 = 6_000;

/// Payload delivered to the overlay webview.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OverlayNotice {
    /// Human name of the app that was just blocked.
    pub app_name: String,
    /// `true` when the app was terminated rather than hidden.
    pub killed: bool,
}

type Cooldown = std::sync::Mutex<Option<(String, i64)>>;
static LAST_FLASH: OnceLock<Cooldown> = OnceLock::new();

fn cooldown() -> &'static Cooldown {
    LAST_FLASH.get_or_init(|| std::sync::Mutex::new(None))
}

/// Should we show the banner for `app_name` at `now_ms`? Also records the
/// decision, so repeated calls within the cooldown are suppressed.
///
/// Split out from [`flash`] so the throttling is testable without a webview.
pub fn should_flash(app_name: &str, now_ms: i64) -> bool {
    let mut guard = match cooldown().lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let recent = guard
        .as_ref()
        .is_some_and(|(name, at)| name == app_name && now_ms - at < FLASH_COOLDOWN_MS);
    if recent {
        return false;
    }
    *guard = Some((app_name.to_string(), now_ms));
    true
}

/// Forget the throttle state — called when a session ends so the first block
/// of the next session always shows.
pub fn reset_throttle() {
    let mut guard = match cooldown().lock() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    *guard = None;
}

/// Build the overlay window up front, from `setup` — i.e. on the main thread.
///
/// Enforcement runs on a blocking worker, and creating a webview off the main
/// thread is platform-dependent at best. Doing it here also means the first
/// blocked app gets an instant banner instead of waiting for a webview boot.
pub fn prewarm<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = ensure_window(app) {
        tracing::warn!("focus overlay: could not pre-create window: {e}");
    }
}

fn ensure_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::WebviewWindow<R>> {
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        return Ok(win);
    }
    let win = WebviewWindowBuilder::new(
        app,
        OVERLAY_LABEL,
        WebviewUrl::App("focus-overlay.html".into()),
    )
    .title("Tracker — Focus")
    .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .visible(false)
    .build()?;
    // Click-through: the banner must never steal a click from whatever the
    // user does next.
    let _ = win.set_ignore_cursor_events(true);
    Ok(win)
}

fn position_top_center<R: Runtime>(win: &tauri::WebviewWindow<R>) {
    let Ok(Some(monitor)) = win.primary_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let position = monitor.position().to_logical::<f64>(scale);
    let x = position.x + (size.width - OVERLAY_WIDTH) / 2.0;
    let y = position.y + OVERLAY_TOP_MARGIN;
    let _ = win.set_size(LogicalSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT));
    let _ = win.set_position(LogicalPosition::new(x, y));
}

/// Show the banner for `notice`, then hide it again after a couple of seconds.
/// Throttled per app name; safe to call on every engine tick.
pub fn flash<R: Runtime>(app: &AppHandle<R>, notice: OverlayNotice, now_ms: i64) {
    if !should_flash(&notice.app_name, now_ms) {
        return;
    }
    let win = match ensure_window(app) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("focus overlay: window unavailable: {e}");
            return;
        }
    };
    position_top_center(&win);
    // The webview may still be booting on the very first flash; emitting
    // before `show` is fine because the frontend also fetches the last notice
    // on mount.
    let _ = app.emit_to(OVERLAY_LABEL, "focus-overlay:notice", &notice);
    let _ = win.show();

    let handle = win.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(OVERLAY_VISIBLE_MS)).await;
        let _ = handle.hide();
    });
}

/// Hide the banner immediately (session ended).
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.hide();
    }
    reset_throttle();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cooldown is process-global, so these tests must not interleave.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn repeat_flashes_for_the_same_app_are_throttled() {
        let _guard = serial();
        reset_throttle();
        assert!(should_flash("Slack", 1_000));
        assert!(!should_flash("Slack", 2_000));
        assert!(should_flash("Slack", 1_000 + FLASH_COOLDOWN_MS));
    }

    #[test]
    fn a_different_app_flashes_immediately() {
        let _guard = serial();
        reset_throttle();
        assert!(should_flash("Slack", 1_000));
        assert!(should_flash("Discord", 1_100));
    }

    #[test]
    fn reset_clears_the_cooldown() {
        let _guard = serial();
        reset_throttle();
        assert!(should_flash("Slack", 1_000));
        reset_throttle();
        assert!(should_flash("Slack", 1_100));
    }
}
