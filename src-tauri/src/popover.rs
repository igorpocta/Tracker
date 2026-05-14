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

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Rect, Runtime, WebviewWindow};

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
    let _ = popover.emit("popover:opened", ());
    Ok(())
}

/// Hide the popover window.
pub fn hide<R: Runtime>(popover: &WebviewWindow<R>) -> tauri::Result<()> {
    popover.hide()?;
    Ok(())
}

/// Hide the popover by label (helper for command callers).
pub fn hide_by_app<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let popover = get_popover(app)?;
    hide(&popover)
}

/// Bring the main window forward (showing it if hidden). Used by the tray menu.
pub fn open_main<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        win.show()?;
        win.unminimize().ok();
        win.set_focus()?;
        let _ = win.emit("main-window:navigate", "main");
    }
    Ok(())
}

/// Bring the main window forward and ask the frontend to navigate to setup.
pub fn open_settings<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    open_main(app)?;
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.emit("main-window:navigate", "setup");
    }
    Ok(())
}
