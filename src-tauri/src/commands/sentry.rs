//! Sentry-related commands.
//!
//! Phase 19 — error reporting is OPT-IN. The frontend reads three pieces of
//! state:
//!
//! * `get_sentry_enabled() -> bool` — current value of the
//!   `sentry_enabled` app_settings key (default `false`).
//! * `set_sentry_enabled(value)` — persist the new value. If turning ON,
//!   attempt to init the backend SDK on the spot (no-op if no DSN is
//!   configured). If turning OFF, capture stops on the next process
//!   restart — the frontend additionally flushes its own SDK.
//! * `get_install_id() -> String` — stable anonymous UUID generated once
//!   per install and persisted in app_settings under `install_id`. Used
//!   as Sentry's `user.id` so events from one install group together
//!   without exposing PII.

use tauri::Emitter;
use uuid::Uuid;

use crate::cache::{self, Db};
use crate::state::AppState;

/// app_settings key for the per-install anonymous identifier.
pub const KEY_INSTALL_ID: &str = "install_id";
/// app_settings key for the opt-in toggle.
pub const KEY_SENTRY_ENABLED: &str = "sentry_enabled";

// -----------------------------------------------------------------------------
// Inner (Tauri-free) helpers.
// -----------------------------------------------------------------------------

/// Read the persisted install id, generating + persisting a fresh UUID v4
/// on the very first call.
pub fn get_or_create_install_id_inner(db: &Db) -> Result<String, String> {
    if let Some(v) = cache::settings::get(db, KEY_INSTALL_ID).map_err(|e| e.to_string())? {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let new_id = Uuid::new_v4().to_string();
    cache::settings::set(db, KEY_INSTALL_ID, &new_id).map_err(|e| e.to_string())?;
    Ok(new_id)
}

/// Read the persisted Sentry opt-in flag (default `false`).
pub fn get_sentry_enabled_inner(db: &Db) -> Result<bool, String> {
    match cache::settings::get(db, KEY_SENTRY_ENABLED).map_err(|e| e.to_string())? {
        Some(v) => match v.as_str() {
            "true" | "1" => Ok(true),
            _ => Ok(false),
        },
        None => Ok(false),
    }
}

/// Persist the Sentry opt-in flag.
pub fn set_sentry_enabled_inner(db: &Db, value: bool) -> Result<(), String> {
    let v = if value { "true" } else { "false" };
    cache::settings::set(db, KEY_SENTRY_ENABLED, v).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Tauri commands.
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_install_id(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_or_create_install_id_inner(&state.db)
}

#[tauri::command]
pub async fn get_sentry_enabled(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    get_sentry_enabled_inner(&state.db)
}

/// Persist the new value, then opportunistically (re)initialise the backend
/// SDK. Disabling at runtime is documented as "takes effect on next
/// restart" — we still emit `prefs-changed` so the frontend can call
/// `shutdownSentry()` on its own SDK immediately.
#[tauri::command]
pub async fn set_sentry_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    value: bool,
) -> Result<(), String> {
    set_sentry_enabled_inner(&state.db, value)?;
    if value {
        let install_id = get_or_create_install_id_inner(&state.db)?;
        // Best-effort init — silently no-ops if there's no DSN configured.
        let _ = crate::sentry_init::init_if_enabled(&install_id, true);
    }
    let _ = app.emit("prefs-changed", "sentry_enabled");
    Ok(())
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
    fn install_id_is_stable_across_calls() {
        let db = open_db();
        let a = get_or_create_install_id_inner(&db).unwrap();
        let b = get_or_create_install_id_inner(&db).unwrap();
        assert_eq!(a, b);
        // Looks like a UUID.
        assert_eq!(a.len(), 36);
        assert_eq!(a.chars().filter(|&c| c == '-').count(), 4);
    }

    #[test]
    fn install_id_is_unique_per_db() {
        let a = get_or_create_install_id_inner(&open_db()).unwrap();
        let b = get_or_create_install_id_inner(&open_db()).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn sentry_enabled_defaults_false() {
        let db = open_db();
        assert!(!get_sentry_enabled_inner(&db).unwrap());
    }

    #[test]
    fn sentry_enabled_round_trips() {
        let db = open_db();
        set_sentry_enabled_inner(&db, true).unwrap();
        assert!(get_sentry_enabled_inner(&db).unwrap());
        set_sentry_enabled_inner(&db, false).unwrap();
        assert!(!get_sentry_enabled_inner(&db).unwrap());
    }
}
