//! System tray + popover commands.
//!
//! Thin Tauri wrappers around [`crate::popover`] / [`crate::tray`]. These are
//! exposed so the React frontend can drive popover visibility from buttons
//! (e.g. an "Open popover" affordance in the main window) without depending on
//! the tray click handler.

use tauri::Emitter;

use crate::popover;

#[tauri::command]
pub async fn show_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    popover::toggle_centered(&app).map_err(|e| e.to_string())?;
    let _ = app.emit("tray-popover", "show");
    Ok(())
}

#[tauri::command]
pub async fn hide_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    popover::hide_by_app(&app).map_err(|e| e.to_string())?;
    let _ = app.emit("tray-popover", "hide");
    Ok(())
}

#[tauri::command]
pub async fn toggle_tray_popover(app: tauri::AppHandle) -> Result<(), String> {
    popover::toggle_centered(&app).map_err(|e| e.to_string())?;
    let _ = app.emit("tray-popover", "toggle");
    Ok(())
}

/// Show or hide the tray icon entirely. Lets the user opt out of the tray
/// (e.g. on a screen-recording session) without quitting the app.
#[tauri::command]
pub async fn set_tray_available(app: tauri::AppHandle, available: bool) -> Result<(), String> {
    crate::tray::set_visible(&app, available).map_err(|e| e.to_string())?;
    let _ = app.emit("tray-available", available);
    Ok(())
}

/// Přebarví **app ikonu** (dock na macOS, taskbar/window na ostatních
/// platformách) podle vybrané palety. `primary` je hlavní brand barva,
/// `secondary` je volitelná tmavší zastávka gradientu (mono palety
/// předají jen `primary`).
///
/// Tichá best-effort akce — pokud render SVG nebo aplikace na NSApp selže,
/// vrátíme chybu, ale aplikace funguje dál s původní ikonou.
#[tauri::command]
pub async fn set_app_icon_accent(
    app: tauri::AppHandle,
    primary: String,
    secondary: Option<String>,
) -> Result<(), String> {
    crate::app_icon::apply(&app, &primary, secondary.as_deref())
}
