//! Popover window placement + show/hide helpers.
//!
//! The popover is configured in `tauri.conf.json` (label `"popover"`,
//! `decorations: false`, `transparent: true`, `alwaysOnTop: true`,
//! `skipTaskbar: true`) so it visually behaves like a menu-bar dropdown.
//!
//! `toggle()` takes the bounding rect of the tray icon reported by the
//! `TrayIconEvent::Click` event and centres the popover horizontally beneath
//! it. If we can't get a rect (e.g. fall-back commands invoked from the
//! frontend), we land in `show_centered` which positions near the top of the
//! primary monitor — visually rough but still usable.

use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Rect, Runtime, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

#[cfg(target_os = "macos")]
mod global_mouse_monitor;

/// Window label that must match `tauri.conf.json`.
pub const POPOVER_LABEL: &str = "popover";
/// Window label of the main app window.
pub const MAIN_LABEL: &str = "main";

/// Tiny gap (in CSS pixels / logical px) between the bottom of the tray icon
/// and the top of the popover. Keeps the popover from sitting flush against
/// the menu bar.
const POPOVER_GAP_PX: f64 = 6.0;

fn get_popover<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    app.get_webview_window(POPOVER_LABEL)
        .ok_or(tauri::Error::WebviewNotFound)
}

fn ensure_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        return Ok(win);
    }

    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_LABEL)
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;

    let win = WebviewWindowBuilder::from_config(app, &cfg)?.build()?;
    // The rebuilt window inherits `decorations: true` from `tauri.conf.json`
    // (macOS needs it for the Overlay traffic lights). Strip it again on
    // Windows or the native bar stacks on top of the custom `WindowTitlebar`.
    apply_main_window_chrome(&win);
    Ok(win)
}

/// Windows: drop the native OS title bar from the main window so only the
/// custom `WindowTitlebar` (rendered by the frontend) shows; keep the drop
/// shadow so the borderless window still has a visible edge + resize border.
///
/// Must run on EVERY path that materialises the main window — first launch
/// (`setup`) *and* every rebuild (`ensure_main_window`, e.g. the user closes
/// the window and reopens it from the tray). A rebuilt window inherits
/// `decorations: true` from `tauri.conf.json`, so skipping this on the rebuild
/// path is what produced the stacked "double title bar" on Windows.
///
/// No-op on macOS/Linux, where the native chrome is kept as configured.
pub fn apply_main_window_chrome<R: Runtime>(win: &WebviewWindow<R>) {
    #[cfg(windows)]
    {
        let _ = win.set_decorations(false);
        let _ = win.set_shadow(true);
    }
    #[cfg(not(windows))]
    {
        let _ = win;
    }
}

/// Ensure the main window exists, is de-minimized and focused.
///
/// This covers the macOS class of failures where the webview window can be
/// gone while the process, tray and popover are still alive.
pub fn focus_main_window<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<WebviewWindow<R>> {
    let win = ensure_main_window(app)?;
    win.unminimize().ok();
    win.show()?;
    win.set_focus()?;
    Ok(win)
}

/// Install a `WindowEvent::Focused(false)` listener on the popover that auto-
/// hides the window when focus is lost.
///
/// Why we do this on the Rust side rather than the JS side: when the user
/// clicks anywhere outside the popover, the OS shifts focus away from the
/// popover window — on macOS this fires `WindowEvent::Focused(false)`; on
/// Windows it maps to `WM_KILLFOCUS`. The webview's own `blur` event is less
/// reliable (e.g. it fires when the inner search input loses focus too).
///
/// Should be called exactly once during `tauri::Builder::setup`.
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let popover = get_popover(app)?;
    let handle = popover.clone();
    popover.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if handle.is_visible().unwrap_or(false) {
                let _ = handle.hide();
            }
        }
    });
    Ok(())
}

/// Pure decision helper for the auto-hide behaviour. Returns `true` iff the
/// given event indicates that the popover should be hidden.
///
/// Extracted so the unit tests don't need a full Tauri runtime.
#[must_use]
pub fn should_auto_hide(event: &WindowEvent, currently_visible: bool) -> bool {
    matches!(event, WindowEvent::Focused(false)) && currently_visible
}

/// Toggle popover visibility. Anchors to `tray_rect` when showing.
pub fn toggle<R: Runtime>(app: &AppHandle<R>, tray_rect: Rect) -> tauri::Result<()> {
    let popover = get_popover(app)?;
    if popover.is_visible().unwrap_or(false) {
        hide(&popover)
    } else {
        show_under(&popover, tray_rect)
    }
}

/// Toggle visibility without a tray rect (used by frontend-invoked commands).
pub fn toggle_centered<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let popover = get_popover(app)?;
    if popover.is_visible().unwrap_or(false) {
        hide(&popover)
    } else {
        show_centered(&popover)
    }
}

/// Show the popover anchored to `tray_rect`, centred horizontally.
///
/// The popover normally sits *below* the tray icon (macOS: menu bar at the top
/// → room underneath). On Windows the taskbar — and thus the tray — is usually
/// at the *bottom*, so anchoring below pushes the popover off-screen behind the
/// taskbar. We therefore flip it *above* the icon whenever the below position
/// would spill past the monitor's work area, and clamp X so an edge-hugging
/// tray (bottom-right on Windows) can't run the popover off the side.
pub fn show_under<R: Runtime>(popover: &WebviewWindow<R>, tray_rect: Rect) -> tauri::Result<()> {
    let size = popover.outer_size()?;
    let scale = popover.scale_factor().unwrap_or(1.0);

    // The tray rect's units (physical vs. logical) depend on the runtime, so
    // normalise everything to physical pixels before doing math.
    let tray_pos = tray_rect.position.to_physical::<f64>(scale);
    let tray_size = tray_rect.size.to_physical::<f64>(scale);

    let popover_w = size.width as f64;
    let popover_h = size.height as f64;
    let gap = POPOVER_GAP_PX * scale;

    let tray_top = tray_pos.y;
    let tray_bottom = tray_pos.y + tray_size.height;
    let tray_center_x = tray_pos.x + tray_size.width / 2.0;

    // Locate the monitor holding the tray icon so we can respect its work area
    // (taskbar-excluded). Fall back through current → primary monitor, then to
    // the legacy below-anchored behaviour if the runtime reports nothing.
    let monitor = popover
        .monitor_from_point(tray_center_x, tray_pos.y + tray_size.height / 2.0)
        .ok()
        .flatten()
        .or_else(|| popover.current_monitor().ok().flatten())
        .or_else(|| popover.primary_monitor().ok().flatten());

    let (x, y) = if let Some(m) = monitor {
        let wa = m.work_area();
        let wa_left = wa.position.x as f64;
        let wa_top = wa.position.y as f64;
        let wa_right = wa_left + wa.size.width as f64;
        let wa_bottom = wa_top + wa.size.height as f64;
        (
            popover_x(tray_center_x, popover_w, wa_left, wa_right),
            popover_y(tray_top, tray_bottom, popover_h, wa_top, wa_bottom, gap),
        )
    } else {
        (tray_center_x - popover_w / 2.0, tray_bottom + gap)
    };

    popover.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))?;

    popover.show()?;
    popover.set_focus()?;
    install_outside_click_monitor(popover);
    let _ = popover.emit("popover:opened", ());
    Ok(())
}

/// Pure vertical-placement decision for [`show_under`]. All values are physical
/// pixels. Returns the popover's top Y: anchored **below** the tray icon when
/// it fits inside the work area (taskbar/menu bar at the *top* → room
/// underneath), flipped **above** it otherwise (taskbar at the *bottom*).
///
/// The final `.min(...).max(...)` clamps the result fully inside the work area
/// so no taskbar edge is overlapped — this is what makes a *top* taskbar (pin
/// to `wa_top`), a left/right taskbar (full-height work area, flips near the
/// bottom) and a popover taller than the work area all land on-screen. The
/// work area itself is taskbar-aware for every edge (Windows `rcWork`), so a
/// single rule covers all four taskbar positions.
fn popover_y(
    tray_top: f64,
    tray_bottom: f64,
    popover_h: f64,
    wa_top: f64,
    wa_bottom: f64,
    gap: f64,
) -> f64 {
    let below = tray_bottom + gap;
    let above = tray_top - popover_h - gap;
    let y = if below + popover_h <= wa_bottom {
        below
    } else {
        above
    };
    y.min(wa_bottom - popover_h).max(wa_top)
}

/// Pure horizontal-placement decision for [`show_under`]. Centres the popover
/// on the tray icon, then clamps its left X so the whole window stays inside
/// the work area even when the tray hugs a screen edge. Physical pixels.
fn popover_x(tray_center_x: f64, popover_w: f64, wa_left: f64, wa_right: f64) -> f64 {
    (tray_center_x - popover_w / 2.0)
        .min(wa_right - popover_w)
        .max(wa_left)
}

/// Show the popover in a safe fallback position (top-centre of the primary
/// monitor). Used when we don't have a tray rect (frontend-invoked toggles).
pub fn show_centered<R: Runtime>(popover: &WebviewWindow<R>) -> tauri::Result<()> {
    if let Ok(Some(monitor)) = popover.current_monitor() {
        let size = popover.outer_size()?;
        let m_size = monitor.size();
        let x = (m_size.width as i32 / 2) - (size.width as i32 / 2);
        let y = 40; // a few pixels under the menu bar
        let _ = popover.set_position(PhysicalPosition::new(x, y));
    } else {
        // Last resort: just put it at logical (40, 40).
        let _ = popover.set_position(LogicalPosition::new(40.0, 40.0));
    }
    popover.show()?;
    popover.set_focus()?;
    install_outside_click_monitor(popover);
    let _ = popover.emit("popover:opened", ());
    Ok(())
}

/// Hide the popover window.
pub fn hide<R: Runtime>(popover: &WebviewWindow<R>) -> tauri::Result<()> {
    popover.hide()?;
    uninstall_outside_click_monitor();
    Ok(())
}

/// Wrap macOS global monitor install — no-op on jiných platformách,
/// aby zbytek modulu nemusel větvit přes `#[cfg]`.
#[inline]
fn install_outside_click_monitor<R: Runtime>(popover: &WebviewWindow<R>) {
    #[cfg(target_os = "macos")]
    {
        global_mouse_monitor::install(popover.app_handle());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = popover;
    }
}

#[inline]
fn uninstall_outside_click_monitor() {
    #[cfg(target_os = "macos")]
    {
        global_mouse_monitor::uninstall();
    }
}

/// Hide the popover by label (helper for command callers).
pub fn hide_by_app<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let popover = get_popover(app)?;
    hide(&popover)
}

/// Bring the main window forward (showing it if hidden). Used by the tray menu.
pub fn open_main<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let win = focus_main_window(app)?;
    let _ = win.emit("main-window:navigate", "main");
    Ok(())
}

/// Bring the main window forward and ask the frontend to navigate to setup.
pub fn open_settings<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let win = focus_main_window(app)?;
    let _ = win.emit("main-window:navigate", "setup");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_hides_on_focus_loss_when_visible() {
        let ev = WindowEvent::Focused(false);
        assert!(should_auto_hide(&ev, true));
    }

    #[test]
    fn does_not_auto_hide_when_already_hidden() {
        let ev = WindowEvent::Focused(false);
        assert!(!should_auto_hide(&ev, false));
    }

    #[test]
    fn does_not_auto_hide_on_focus_gain() {
        let ev = WindowEvent::Focused(true);
        assert!(!should_auto_hide(&ev, true));
    }

    #[test]
    fn does_not_auto_hide_on_destroyed() {
        let ev = WindowEvent::Destroyed;
        assert!(!should_auto_hide(&ev, true));
    }

    // --- Popover placement -------------------------------------------------
    // Work area modelled as a 1000px-tall screen (top=0, bottom=1000);
    // popover 520px tall, 380px wide; 6px gap.

    #[test]
    fn anchors_below_when_tray_is_at_the_top() {
        // macOS: menu bar at the top, tray icon near y=0 → plenty of room below.
        let y = popover_y(0.0, 24.0, 520.0, 0.0, 1000.0, 6.0);
        assert_eq!(y, 30.0); // tray_bottom (24) + gap (6)
    }

    #[test]
    fn flips_above_when_tray_is_at_the_bottom() {
        // Windows: taskbar at the bottom, tray near y=960 → below would spill
        // past 1000, so flip above the icon.
        let y = popover_y(960.0, 1000.0, 520.0, 0.0, 1000.0, 6.0);
        assert_eq!(y, 434.0); // tray_top (960) - popover_h (520) - gap (6)
    }

    #[test]
    fn clamps_into_work_area_when_neither_orientation_fits() {
        // Tiny work area shorter than the popover — pin to the top of it.
        let y = popover_y(300.0, 320.0, 520.0, 100.0, 400.0, 6.0);
        assert_eq!(y, 100.0); // max(wa_bottom - popover_h, wa_top) = max(-120, 100)
    }

    #[test]
    fn pins_below_the_top_taskbar_without_overlapping_it() {
        // Taskbar at the TOP (48px): work area starts at y=48, tray sits in the
        // bar (y≈8..40). Below fits, but the final clamp pins it to wa_top so
        // the popover doesn't ride 2px up into the taskbar.
        let y = popover_y(8.0, 40.0, 520.0, 48.0, 1080.0, 6.0);
        assert_eq!(y, 48.0); // clamped up to wa_top (was 46 = tray_bottom + gap)
    }

    #[test]
    fn vertical_taskbar_tray_at_bottom_flips_above_and_sits_in_work_area() {
        // Left/right taskbar: work area is full-height (0..1080), tray is at the
        // bottom of the vertical bar (y≈1040..1072). Below would spill, so flip
        // above the icon; the popover (514..1034) stays inside the work area.
        let y = popover_y(1040.0, 1072.0, 520.0, 0.0, 1080.0, 6.0);
        assert_eq!(y, 514.0); // tray_top - popover_h - gap = 1040 - 520 - 6
    }

    #[test]
    fn left_taskbar_clamps_x_to_the_right_of_the_bar() {
        // Left taskbar 48px wide → work area starts at x=48; tray near x≈24.
        // Centre would go negative; clamp pins it just right of the taskbar.
        let x = popover_x(24.0, 380.0, 48.0, 1920.0);
        assert_eq!(x, 48.0); // wa_left
    }

    #[test]
    fn right_taskbar_clamps_x_to_the_left_of_the_bar() {
        // Right taskbar 48px wide → work area ends at x=1872; tray near x≈1896.
        let x = popover_x(1896.0, 380.0, 0.0, 1872.0);
        assert_eq!(x, 1492.0); // wa_right - popover_w = 1872 - 380
    }

    #[test]
    fn centres_x_on_the_tray_icon_when_there_is_room() {
        // Tray centred at x=500 → left edge at 500 - 190.
        let x = popover_x(500.0, 380.0, 0.0, 1000.0);
        assert_eq!(x, 310.0);
    }

    #[test]
    fn clamps_x_off_the_right_edge() {
        // Windows tray bottom-right: centre near x=990 would overflow the
        // right edge — clamp so the popover stays fully on-screen.
        let x = popover_x(990.0, 380.0, 0.0, 1000.0);
        assert_eq!(x, 620.0); // wa_right (1000) - popover_w (380)
    }

    #[test]
    fn clamps_x_off_the_left_edge() {
        let x = popover_x(10.0, 380.0, 0.0, 1000.0);
        assert_eq!(x, 0.0); // wa_left
    }
}
