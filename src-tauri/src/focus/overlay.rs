//! The "that app is blocked right now" banner.
//!
//! A small, click-through window pinned near the top of the primary monitor.
//! It is deliberately not a full-screen blocker: the enforcement already
//! happened (the app was hidden or terminated), so this only needs to explain
//! *why* the window the user just clicked vanished.
//!
//! The window is created lazily on first use and then reused — building a
//! webview costs far more than showing one.

use std::sync::atomic::{AtomicU64, Ordering};
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

/// Incremented per banner shown. The auto-hide task captures its own value and
/// stands down if a newer banner has replaced it — otherwise blocking a second
/// app within the display window meant the first banner's timer cut the second
/// one short.
static FLASH_SEQ: AtomicU64 = AtomicU64::new(0);

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
///
/// The whole body is hopped onto the main thread. Enforcement runs on a
/// blocking worker, and building a webview off the main thread is
/// platform-dependent at best — which is why the window is created here, on
/// first use, rather than eagerly at startup: a user who never touches Focus
/// mode should not carry a webview process for the life of the app.
pub fn flash<R: Runtime>(app: &AppHandle<R>, notice: OverlayNotice, now_ms: i64) {
    if !should_flash(&notice.app_name, now_ms) {
        return;
    }
    let handle = app.clone();
    let dispatched = app.run_on_main_thread(move || {
        let win = match ensure_window(&handle) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("focus overlay: window unavailable: {e}");
                return;
            }
        };
        position_top_center(&win);
        let _ = handle.emit_to(OVERLAY_LABEL, "focus-overlay:notice", &notice);
        let _ = win.show();

        let seq = claim_banner();
        let hide_handle = win.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(OVERLAY_VISIBLE_MS)).await;
            // A newer banner owns the window now; leave it alone.
            if owns_banner(seq) {
                let _ = hide_handle.hide();
            }
        });
    });
    if let Err(e) = dispatched {
        tracing::warn!("focus overlay: could not reach the main thread: {e}");
    }
}

/// Take ownership of the banner, returning the token the auto-hide task must
/// still hold when it fires.
fn claim_banner() -> u64 {
    FLASH_SEQ.fetch_add(1, Ordering::SeqCst) + 1
}

/// Is `seq` still the banner on screen?
fn owns_banner(seq: u64) -> bool {
    FLASH_SEQ.load(Ordering::SeqCst) == seq
}

/// Hide the banner immediately (session ended).
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    // Retire any pending auto-hide too, so a task scheduled a moment ago can't
    // fire against a banner the next session puts up.
    claim_banner();
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
    fn a_superseded_banner_declines_to_hide_the_new_one() {
        let _guard = serial();
        let first = claim_banner();
        assert!(owns_banner(first));
        let second = claim_banner();
        // The first banner's timer is now stale and must stand down; without
        // this, blocking a second app cut the second banner short.
        assert!(!owns_banner(first));
        assert!(owns_banner(second));
    }

    #[test]
    fn claiming_the_banner_retires_every_earlier_token() {
        let _guard = serial();
        let stale = claim_banner();
        for _ in 0..3 {
            claim_banner();
        }
        assert!(!owns_banner(stale));
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
