//! System tray + popover commands.
//!
//! Phase 4 ships *stubs* that emit events so the frontend can wire up its
//! buttons against the final command names. Real popover positioning + tray
//! menu live in Phase 7.

use tauri::{Emitter, Manager};

#[tauri::command]
pub async fn show_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("popover") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    let _ = app.emit("tray-popover", "show");
    Ok(())
}

#[tauri::command]
pub async fn hide_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("popover") {
        let _ = win.hide();
    }
    let _ = app.emit("tray-popover", "hide");
    Ok(())
}

#[tauri::command]
pub async fn toggle_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("popover") {
        let visible = win.is_visible().unwrap_or(false);
        if visible {
            let _ = win.hide();
            let _ = app.emit("tray-popover", "hide");
        } else {
            let _ = win.show();
            let _ = win.set_focus();
            let _ = app.emit("tray-popover", "show");
        }
    } else {
        let _ = app.emit("tray-popover", "toggle");
    }
    Ok(())
}

#[tauri::command]
pub async fn set_tray_available(
    app: tauri::AppHandle,
    available: bool,
) -> Result<(), String> {
    // Real tray icon visibility is wired in Phase 7; for now we just emit so
    // the frontend can adapt its UI.
    let _ = app.emit("tray-available", available);
    Ok(())
}
