//! System tray icon setup + visual helpers.
//!
//! The tray surface owns:
//! - The icon registered with id `main-tray` (matches `tauri.conf.json`).
//! - A context menu with Open Tracker / Settings / Quit.
//! - Left-click → toggle the popover window beneath the tray icon.
//! - Visual state: tooltip + icon swap between idle and running variants.
//!
//! Bytes for both icon variants are compiled into the binary so we never have
//! to chase resource paths at runtime — they live next to this file in
//! `src-tauri/icons/`.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Runtime,
};

/// Stable id used both in `tauri.conf.json` and when we call
/// [`tauri::AppHandle::tray_by_id`].
pub const TRAY_ID: &str = "main-tray";

const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray.png");
const TRAY_ICON_RUNNING: &[u8] = include_bytes!("../icons/tray-running.png");

/// Install the system tray (icon + menu + click handler).
///
/// Should be called exactly once during `tauri::Builder::setup`.
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // --- Menu items --------------------------------------------------------
    let open_main = MenuItem::with_id(app, "open_main", "Open Tracker", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(
        app,
        "open_settings",
        "Settings…",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Tracker", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(app, &[&open_main, &open_settings, &separator, &quit])?;

    // --- Builder -----------------------------------------------------------
    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Tracker — idle")
        .icon(Image::from_bytes(TRAY_ICON_IDLE)?)
        .icon_as_template(true)
        // We control the popover via left-click; the menu fires on right-click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open_main" => {
                let _ = crate::popover::open_main(app);
            }
            "open_settings" => {
                let _ = crate::popover::open_settings(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let _ = crate::popover::toggle(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    Ok(())
}

/// Swap the tray icon between the idle and running variants. macOS treats both
/// as template images so they tint correctly under light/dark menu bars.
pub fn set_running_visual<R: Runtime>(app: &AppHandle<R>, running: bool) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let bytes = if running {
        TRAY_ICON_RUNNING
    } else {
        TRAY_ICON_IDLE
    };
    let img = Image::from_bytes(bytes)?;
    // `set_icon_with_as_template` is atomic on macOS (avoids flicker) and falls
    // back to a plain `set_icon` everywhere else.
    tray.set_icon_with_as_template(Some(img), true)?;
    Ok(())
}

/// Update the tray tooltip. macOS-only (Linux/Windows behave differently but
/// the call is safe to make — it's a no-op on Linux).
pub fn set_tooltip<R: Runtime>(app: &AppHandle<R>, text: &str) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_tooltip(Some(text))?;
    Ok(())
}

/// Show or hide the tray icon entirely. Used by `set_tray_available` so the
/// user can opt out of the tray.
pub fn set_visible<R: Runtime>(app: &AppHandle<R>, visible: bool) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_visible(visible)?;
    Ok(())
}
