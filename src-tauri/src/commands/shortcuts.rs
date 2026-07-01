//! Global (system-wide) keyboard shortcut for toggling the timer.
//!
//! The shortcut is registered with the OS via `tauri-plugin-global-shortcut`.
//! When pressed it emits a `global-shortcut-triggered` event; the frontend
//! listener performs the actual toggle through the timer store (start an
//! unassigned timer when idle, stop + record the worklog when one is running).
//! Keeping the toggle in the frontend reuses the fully-tested start/stop path
//! (including provider push + all the `timer-*` events that sync the tray and
//! popover) instead of duplicating that logic in the shortcut handler.
//!
//! Design notes:
//!   * The accelerator is user-configurable and stored in `app_settings`. An
//!     empty stored value means "disabled".
//!   * Registration is best-effort: if the combo is already held by another
//!     app or the OS refuses it, `register` fails and we report
//!     `registered: false` so the Settings UI can warn the user (this is the
//!     closest portable signal to "the shortcut is already taken").

use std::str::FromStr;

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::cache::{self, Db};
use crate::state::AppState;

/// Default accelerator: `Cmd/Ctrl + Shift + .` (Period). Deliberately avoids the
/// `Cmd/Ctrl+Shift+T` "reopen closed tab" collision browsers grab. Stored as a
/// Tauri accelerator string; the frontend prettifies it for display.
pub const DEFAULT_GLOBAL_SHORTCUT: &str = "CommandOrControl+Shift+Period";

const KEY_GLOBAL_SHORTCUT: &str = "global_shortcut";

/// Event emitted to all windows when the global shortcut fires. The frontend
/// listens for this and toggles the timer.
pub const EVENT_TRIGGERED: &str = "global-shortcut-triggered";

/// Status returned to the frontend: the currently stored accelerator and
/// whether it is actually registered with the OS right now.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GlobalShortcutStatus {
    pub accelerator: String,
    pub registered: bool,
}

/// Read the stored accelerator, falling back to [`DEFAULT_GLOBAL_SHORTCUT`]
/// when unset. An empty stored string is returned verbatim and means the
/// shortcut is intentionally disabled.
pub fn get_global_shortcut_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_GLOBAL_SHORTCUT).map_err(|e| e.to_string())? {
        Some(v) => Ok(v),
        None => Ok(DEFAULT_GLOBAL_SHORTCUT.to_string()),
    }
}

/// Persist the accelerator. An empty string disables the shortcut; any other
/// value must parse as a valid accelerator, otherwise the call is rejected so
/// we never store a string that can't be registered.
pub fn set_global_shortcut_inner(db: &Db, accelerator: &str) -> Result<(), String> {
    let trimmed = accelerator.trim();
    if !trimmed.is_empty() {
        parse_accelerator(trimmed)?;
    }
    cache::settings::set(db, KEY_GLOBAL_SHORTCUT, trimmed).map_err(|e| e.to_string())
}

/// Parse a Tauri accelerator string (e.g. `"CommandOrControl+Shift+Period"`)
/// into a [`Shortcut`], mapping the parse failure to a friendly Czech message.
pub fn parse_accelerator(accelerator: &str) -> Result<Shortcut, String> {
    Shortcut::from_str(accelerator)
        .map_err(|_| format!("Neplatná klávesová zkratka: {accelerator:?}"))
}

/// (Re)register the global shortcut from the current stored preference,
/// clearing any previously registered combo first. Returns `true` if a
/// shortcut is now live, `false` if it is disabled or registration failed.
pub fn apply_global_shortcut<R: Runtime>(app: &AppHandle<R>) -> bool {
    let accel = {
        let state = app.state::<AppState>();
        get_global_shortcut_inner(&state.db).unwrap_or_default()
    };
    register_accelerator(app, &accel)
}

/// Low-level: unregister everything we previously held, then register
/// `accelerator` (unless empty). The handler emits [`EVENT_TRIGGERED`] on key
/// press. Returns whether a shortcut ended up registered.
fn register_accelerator<R: Runtime>(app: &AppHandle<R>, accelerator: &str) -> bool {
    let gs = app.global_shortcut();
    // Always clear first so changing the combo doesn't leave the old one live.
    let _ = gs.unregister_all();

    let trimmed = accelerator.trim();
    if trimmed.is_empty() {
        return false; // disabled
    }
    let Ok(shortcut) = parse_accelerator(trimmed) else {
        return false;
    };
    gs.on_shortcut(shortcut, move |app, _shortcut, event| {
        // Fire on key-down only; the handler is also invoked on release.
        if event.state() == ShortcutState::Pressed {
            let _ = app.emit(EVENT_TRIGGERED, ());
        }
    })
    .is_ok()
}

#[tauri::command]
pub fn get_global_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GlobalShortcutStatus, String> {
    let accelerator = get_global_shortcut_inner(&state.db)?;
    let registered = match parse_accelerator(accelerator.trim()) {
        Ok(sc) => app.global_shortcut().is_registered(sc),
        Err(_) => false,
    };
    Ok(GlobalShortcutStatus {
        accelerator,
        registered,
    })
}

#[tauri::command]
pub fn set_global_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    accelerator: String,
) -> Result<GlobalShortcutStatus, String> {
    // Persist first (this validates the accelerator), then (re)register.
    set_global_shortcut_inner(&state.db, &accelerator)?;
    let accelerator = accelerator.trim().to_string();
    let registered = register_accelerator(&app, &accelerator);
    Ok(GlobalShortcutStatus {
        accelerator,
        registered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn get_returns_default_when_unset() {
        let db = open_db();
        assert_eq!(
            get_global_shortcut_inner(&db).unwrap(),
            DEFAULT_GLOBAL_SHORTCUT
        );
    }

    #[test]
    fn set_then_get_round_trips() {
        let db = open_db();
        set_global_shortcut_inner(&db, "CommandOrControl+Alt+T").unwrap();
        assert_eq!(
            get_global_shortcut_inner(&db).unwrap(),
            "CommandOrControl+Alt+T"
        );
    }

    #[test]
    fn empty_string_disables_and_is_allowed() {
        let db = open_db();
        set_global_shortcut_inner(&db, "   ").unwrap();
        assert_eq!(get_global_shortcut_inner(&db).unwrap(), "");
    }

    #[test]
    fn set_rejects_an_unparseable_accelerator() {
        let db = open_db();
        let err = set_global_shortcut_inner(&db, "not a shortcut!!").unwrap_err();
        assert!(err.contains("Neplatná"), "unexpected error: {err}");
        // Nothing persisted → still the default.
        assert_eq!(
            get_global_shortcut_inner(&db).unwrap(),
            DEFAULT_GLOBAL_SHORTCUT
        );
    }

    #[test]
    fn default_accelerator_parses() {
        assert!(parse_accelerator(DEFAULT_GLOBAL_SHORTCUT).is_ok());
    }
}
