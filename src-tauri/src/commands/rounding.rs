//! Time rounding for worklog creation (Phase 18A — Item 27).
//!
//! Three modes:
//! - `"none"`   — pass-through (default).
//! - `"up"`     — round to the next multiple of `interval_minutes`.
//! - `"down"`   — round to the previous multiple.
//!
//! The interval is one of 1, 5, 15, 60 minutes. The rounded duration is applied
//! in [`create_manual_worklog`], `stop_timer_inner`, and `update_worklog`
//! before the value is sent to Jira.

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self, Db};
use crate::state::AppState;

pub const KEY_ROUNDING_MODE: &str = "rounding_mode";
pub const KEY_ROUNDING_INTERVAL: &str = "rounding_interval_minutes";

pub const DEFAULT_ROUNDING_MODE: &str = "none";
pub const DEFAULT_ROUNDING_INTERVAL: i64 = 1;

pub const ALLOWED_ROUNDING_MODES: &[&str] = &["none", "up", "down"];
pub const ALLOWED_INTERVALS: &[i64] = &[1, 5, 15, 60];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoundingMode {
    None,
    Up,
    Down,
}

impl RoundingMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "up" => RoundingMode::Up,
            "down" => RoundingMode::Down,
            _ => RoundingMode::None,
        }
    }
}

/// Round `duration_seconds` according to `mode` + `interval_minutes`.
///
/// The interval is converted to seconds internally. Negative durations are
/// clamped to 0 first. Mode = "none" or interval <= 0 returns the input.
pub fn apply_rounding(
    duration_seconds: i64,
    mode: &str,
    interval_minutes: i64,
) -> i64 {
    let d = duration_seconds.max(0);
    if interval_minutes <= 0 {
        return d;
    }
    let step = interval_minutes.saturating_mul(60);
    match RoundingMode::from_str(mode) {
        RoundingMode::None => d,
        RoundingMode::Up => {
            if d == 0 {
                return 0;
            }
            ((d + step - 1) / step) * step
        }
        RoundingMode::Down => (d / step) * step,
    }
}

// -----------------------------------------------------------------------------
// Inner (Tauri-free) helpers.
// -----------------------------------------------------------------------------

pub fn get_rounding_mode_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_ROUNDING_MODE).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_ROUNDING_MODES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_ROUNDING_MODE.to_string()),
    }
}

pub fn set_rounding_mode_inner(db: &Db, mode: &str) -> Result<(), String> {
    if !ALLOWED_ROUNDING_MODES.contains(&mode) {
        return Err(format!(
            "invalid rounding mode {mode:?}; expected one of {ALLOWED_ROUNDING_MODES:?}"
        ));
    }
    cache::settings::set(db, KEY_ROUNDING_MODE, mode).map_err(|e| e.to_string())
}

pub fn get_rounding_interval_minutes_inner(db: &Db) -> Result<i64, String> {
    match cache::settings::get(db, KEY_ROUNDING_INTERVAL).map_err(|e| e.to_string())? {
        Some(v) => match v.parse::<i64>() {
            Ok(n) if ALLOWED_INTERVALS.contains(&n) => Ok(n),
            _ => Ok(DEFAULT_ROUNDING_INTERVAL),
        },
        None => Ok(DEFAULT_ROUNDING_INTERVAL),
    }
}

pub fn set_rounding_interval_minutes_inner(db: &Db, minutes: i64) -> Result<(), String> {
    if !ALLOWED_INTERVALS.contains(&minutes) {
        return Err(format!(
            "invalid rounding interval {minutes}; expected one of {ALLOWED_INTERVALS:?}"
        ));
    }
    cache::settings::set(db, KEY_ROUNDING_INTERVAL, &minutes.to_string())
        .map_err(|e| e.to_string())
}

/// Convenience: read both settings and apply rounding in one call.
pub fn apply_active_rounding(db: &Db, duration_seconds: i64) -> i64 {
    let mode = get_rounding_mode_inner(db).unwrap_or_else(|_| DEFAULT_ROUNDING_MODE.to_string());
    let interval =
        get_rounding_interval_minutes_inner(db).unwrap_or(DEFAULT_ROUNDING_INTERVAL);
    apply_rounding(duration_seconds, &mode, interval)
}

// -----------------------------------------------------------------------------
// Tauri commands.
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_rounding_mode(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_rounding_mode_inner(&state.db)
}

#[tauri::command]
pub async fn set_rounding_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    set_rounding_mode_inner(&state.db, &mode)?;
    let _ = app.emit("prefs-changed", "rounding_mode");
    Ok(())
}

#[tauri::command]
pub async fn get_rounding_interval_minutes(
    state: tauri::State<'_, AppState>,
) -> Result<i64, String> {
    get_rounding_interval_minutes_inner(&state.db)
}

#[tauri::command]
pub async fn set_rounding_interval_minutes(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    minutes: i64,
) -> Result<(), String> {
    set_rounding_interval_minutes_inner(&state.db, minutes)?;
    let _ = app.emit("prefs-changed", "rounding_interval_minutes");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_mode_passes_through() {
        assert_eq!(apply_rounding(0, "none", 1), 0);
        assert_eq!(apply_rounding(123, "none", 5), 123);
        assert_eq!(apply_rounding(3600, "none", 60), 3600);
    }

    #[test]
    fn up_mode_rounds_up_to_next_multiple() {
        // 1-minute = 60 sec
        assert_eq!(apply_rounding(0, "up", 1), 0);
        assert_eq!(apply_rounding(1, "up", 1), 60);
        assert_eq!(apply_rounding(59, "up", 1), 60);
        assert_eq!(apply_rounding(60, "up", 1), 60);
        assert_eq!(apply_rounding(61, "up", 1), 120);

        // 15-minute = 900 sec
        assert_eq!(apply_rounding(1, "up", 15), 900);
        assert_eq!(apply_rounding(899, "up", 15), 900);
        assert_eq!(apply_rounding(900, "up", 15), 900);
        assert_eq!(apply_rounding(901, "up", 15), 1800);

        // 60-minute = 3600 sec
        assert_eq!(apply_rounding(3500, "up", 60), 3600);
    }

    #[test]
    fn down_mode_rounds_down_to_previous_multiple() {
        assert_eq!(apply_rounding(59, "down", 1), 0);
        assert_eq!(apply_rounding(60, "down", 1), 60);
        assert_eq!(apply_rounding(61, "down", 1), 60);
        assert_eq!(apply_rounding(900, "down", 15), 900);
        assert_eq!(apply_rounding(1799, "down", 15), 900);
        assert_eq!(apply_rounding(1800, "down", 15), 1800);
    }

    #[test]
    fn negative_durations_clamp_to_zero() {
        assert_eq!(apply_rounding(-10, "up", 1), 0);
        assert_eq!(apply_rounding(-10, "down", 1), 0);
    }

    #[test]
    fn zero_interval_is_passthrough() {
        assert_eq!(apply_rounding(123, "up", 0), 123);
    }

    #[test]
    fn unknown_mode_is_passthrough() {
        assert_eq!(apply_rounding(123, "weird", 5), 123);
    }
}
