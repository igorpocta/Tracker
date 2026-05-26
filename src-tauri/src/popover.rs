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
    AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Rect, Runtime,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
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

    WebviewWindowBuilder::from_config(app, &cfg)?.build()
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

/// Show the popover positioned underneath `tray_rect`, centred horizontally.
pub fn show_under<R: Runtime>(popover: &WebviewWindow<R>, tray_rect: Rect) -> tauri::Result<()> {
    let size = popover.outer_size()?;
    let scale = popover.scale_factor().unwrap_or(1.0);

    // The tray rect's units (physical vs. logical) depend on the runtime, so
    // normalise everything to physical pixels before doing math.
    let tray_pos = tray_rect.position.to_physical::<f64>(scale);
    let tray_size = tray_rect.size.to_physical::<f64>(scale);

    let popover_w = size.width as f64;

    // Centre horizontally on the tray icon; place a small gap below it.
    let target_x_phys = tray_pos.x + (tray_size.width / 2.0) - (popover_w / 2.0);
    let target_y_phys = tray_pos.y + tray_size.height + POPOVER_GAP_PX * scale;

    popover.set_position(PhysicalPosition::new(
        target_x_phys.round() as i32,
        target_y_phys.round() as i32,
    ))?;

    popover.show()?;
    popover.set_focus()?;
    install_outside_click_monitor(popover);
    let _ = popover.emit("popover:opened", ());
    Ok(())
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
}
