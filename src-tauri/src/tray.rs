//! System tray icon setup + visual helpers.
//!
//! The tray surface owns:
//! - The icon registered with id `main-tray` (declared in `tauri.conf.json`).
//! - A context menu with Open Tracker / Settings / Quit.
//! - Left-click → toggle the popover window beneath the tray icon.
//! - Visual state: tooltip + icon swap between idle and running variants.
//!
//! Bytes for both icon variants are compiled into the binary so we never have
//! to chase resource paths at runtime — they live next to this file in
//! `src-tauri/icons/`.
//!
//! Phase 13 fix: previously this module called [`TrayIconBuilder::with_id`]
//! *in addition* to the declarative `trayIcon` block in `tauri.conf.json`,
//! which left macOS with two tray icons. We now look the existing tray up by
//! id and attach the menu + handlers to it instead, producing exactly one
//! tray icon.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconEvent},
    AppHandle, Runtime,
};

/// Stable id used both in `tauri.conf.json` and when we call
/// [`tauri::AppHandle::tray_by_id`].
pub const TRAY_ID: &str = "main-tray";

const TRAY_ICON_IDLE: &[u8] = include_bytes!("../icons/tray.png");
const TRAY_ICON_RUNNING: &[u8] = include_bytes!("../icons/tray-running.png");

/// Attach menu, icon and click handlers to the tray declared in
/// `tauri.conf.json`. This is called exactly once during
/// `tauri::Builder::setup`.
///
/// We deliberately do NOT call `TrayIconBuilder` here — the tray is created
/// by the framework from the declarative config, and creating a second one
/// would result in two macOS menu-bar icons.
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        tracing::warn!("tray with id {TRAY_ID:?} not registered — check tauri.conf.json");
        return Ok(());
    };

    // --- Menu items --------------------------------------------------------
    let quick_start = MenuItem::with_id(
        app,
        "quick_start_unassigned",
        "▶ Začít stopovat bez úkolu",
        true,
        None::<&str>,
    )?;
    let sep0 = PredefinedMenuItem::separator(app)?;
    let open_main = MenuItem::with_id(app, "open_main", "Otevřít Tracker", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(
        app,
        "open_settings",
        "Nastavení…",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Ukončit Tracker", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &quick_start,
            &sep0,
            &open_main,
            &open_settings,
            &separator,
            &quit,
        ],
    )?;

    tray.set_menu(Some(menu))?;
    tray.set_tooltip(Some("Tracker — nečinný"))?;
    tray.set_icon(Some(Image::from_bytes(TRAY_ICON_IDLE)?))?;
    tray.set_icon_as_template(true)?;
    // Left-click drives the popover; the menu fires on right-click only.
    tray.set_show_menu_on_left_click(false)?;

    tray.on_menu_event(|app, event| match event.id.as_ref() {
        "open_main" => {
            let _ = crate::popover::open_main(app);
        }
        "open_settings" => {
            let _ = crate::popover::open_settings(app);
        }
        "quick_start_unassigned" => {
            spawn_quick_start_unassigned(app.clone());
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    });

    tray.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            rect,
            ..
        } = event
        {
            let _ = crate::popover::toggle(tray.app_handle(), rect);
        }
    });

    Ok(())
}

/// Nastav idle tray ikonu na bílou „Zzz" siluetu (rendered SVG → PNG).
/// `iconAsTemplate = false` ať si macOS bílou nepřebarví; user wants it
/// always white regardless of light/dark menu bar.
pub fn set_idle_zzz<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    match crate::tray_pulse::idle_zzz_png() {
        Some(bytes) => {
            let img = Image::from_bytes(bytes)?;
            tray.set_icon_with_as_template(Some(img), false)?;
        }
        None => {
            // SVG render failed — clear the icon so the title text carries
            // the idle state alone (graceful degradation).
            tray.set_icon(None)?;
        }
    }
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

/// Apply a single frame of the recording pulse (a red dot at varying opacity)
/// as the tray icon. Used by `tray_ticker` when a timer is running — the icon
/// is the visual recording indicator while `set_title` carries the issue key
/// and elapsed time.
///
/// Crucially we set `iconAsTemplate = false` here so macOS preserves our red
/// color instead of tinting the glyph monochrome.
pub fn set_pulse_frame<R: Runtime>(app: &AppHandle<R>, frame_idx: usize) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let bytes = crate::tray_pulse::frame_png(frame_idx)
        .map_err(|e| tauri::Error::Anyhow(anyhow::anyhow!("pulse frame: {e}")))?;
    let img = Image::from_bytes(&bytes)?;
    tray.set_icon_with_as_template(Some(img), false)?;
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

/// Remove the tray icon image entirely, leaving only the title text in the
/// macOS menu bar. Called once at startup so the menu-bar entry is just the
/// "💤 —:—" / "🔴 DEV-123 01:23" string (no stopwatch glyph crowding it).
pub fn clear_icon<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_icon(None)?;
    Ok(())
}

/// Phase 18A — Item 34: set the menu-bar title text on macOS.
///
/// `text` is shown next to the tray icon (e.g. `"ACME-1 01:23"` when a timer
/// is running, empty when idle). On Windows/Linux `set_title` is a no-op or
/// not supported — the call is safe everywhere.
pub fn set_title<R: Runtime>(app: &AppHandle<R>, text: Option<&str>) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    tray.set_title(text)?;
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

/// Tray menu „Začít stopovat bez úkolu" — instant kick-off bez interakce s
/// hlavním oknem. Klient si pak v Time Logu úkol doplní (řádek se objeví
/// jako pending-assignment). Stop a všechny ostatní akce nadále řeší
/// hlavní UI, tady chceme jen jednoduchý rychlý start.
fn spawn_quick_start_unassigned<R: Runtime>(app: AppHandle<R>) {
    use tauri::Manager;
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::state::AppState>();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let res = crate::commands::timer::start_timer_inner(&state.db, "", now_ms, None, None);
        if let Ok(active) = res {
            use tauri::Emitter;
            let _ = app.emit("timer-started", &active);
        }
    });
}
