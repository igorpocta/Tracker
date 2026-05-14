//! User preference commands backed by the `app_settings` key/value table.

use tauri::Emitter;

use crate::cache::{self, Db};
use crate::state::AppState;

/// Default daily goal: 8 hours, expressed in seconds.
pub const DEFAULT_DAILY_GOAL_SECONDS: i64 = 8 * 60 * 60;
/// Default widget format: `"hh:mm"` (matches the original Trcker product).
pub const DEFAULT_WIDGET_FORMAT: &str = "hh:mm";
/// Default app icon identifier.
pub const DEFAULT_APP_ICON: &str = "default";
/// Default hourly rate: 0 (disabled — the UI hides the row).
pub const DEFAULT_HOURLY_RATE: f64 = 0.0;
/// Default theme: `"auto"` (follow the OS setting).
pub const DEFAULT_THEME: &str = "auto";
/// Allowed theme values.
pub const ALLOWED_THEMES: &[&str] = &["auto", "light", "dark"];
/// Default font size: `"md"`.
pub const DEFAULT_FONT_SIZE: &str = "md";
/// Allowed font size values.
pub const ALLOWED_FONT_SIZES: &[&str] = &["sm", "md", "lg"];
/// Default density: `"comfortable"`.
pub const DEFAULT_DENSITY: &str = "comfortable";
/// Allowed density values.
pub const ALLOWED_DENSITIES: &[&str] = &["compact", "comfortable"];

const KEY_DAILY_GOAL: &str = "daily_goal_seconds";
const KEY_WIDGET_FORMAT: &str = "widget_format";
const KEY_APP_ICON: &str = "app_icon";
const KEY_HOURLY_RATE: &str = "hourly_rate";
const KEY_THEME: &str = "theme";
const KEY_FONT_SIZE: &str = "font_size";
const KEY_DENSITY: &str = "density";

// -----------------------------------------------------------------------------
// Inner (Tauri-free) helpers.
// -----------------------------------------------------------------------------

pub fn get_daily_goal_inner(db: &Db) -> Result<i64, String> {
    match cache::settings::get(db, KEY_DAILY_GOAL).map_err(|e| e.to_string())? {
        Some(v) => v
            .parse::<i64>()
            .map_err(|_| format!("invalid daily_goal_seconds: {v}")),
        None => Ok(DEFAULT_DAILY_GOAL_SECONDS),
    }
}

pub fn set_daily_goal_inner(db: &Db, seconds: i64) -> Result<(), String> {
    if seconds < 0 {
        return Err("daily_goal_seconds must be non-negative".into());
    }
    cache::settings::set(db, KEY_DAILY_GOAL, &seconds.to_string()).map_err(|e| e.to_string())
}

pub fn set_widget_format_inner(db: &Db, format: &str) -> Result<(), String> {
    cache::settings::set(db, KEY_WIDGET_FORMAT, format).map_err(|e| e.to_string())
}

pub fn set_app_icon_inner(db: &Db, icon: &str) -> Result<(), String> {
    cache::settings::set(db, KEY_APP_ICON, icon).map_err(|e| e.to_string())
}

pub fn get_hourly_rate_inner(db: &Db) -> Result<f64, String> {
    match cache::settings::get(db, KEY_HOURLY_RATE).map_err(|e| e.to_string())? {
        Some(v) => v
            .parse::<f64>()
            .map_err(|_| format!("invalid hourly_rate: {v}")),
        None => Ok(DEFAULT_HOURLY_RATE),
    }
}

pub fn set_hourly_rate_inner(db: &Db, rate: f64) -> Result<(), String> {
    if rate < 0.0 || !rate.is_finite() {
        return Err("hourly_rate must be a non-negative finite number".into());
    }
    cache::settings::set(db, KEY_HOURLY_RATE, &rate.to_string()).map_err(|e| e.to_string())
}

// ----- Theme -----

pub fn get_theme_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_THEME).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_THEMES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_THEME.to_string()),
    }
}

pub fn set_theme_inner(db: &Db, theme: &str) -> Result<(), String> {
    if !ALLOWED_THEMES.contains(&theme) {
        return Err(format!(
            "invalid theme {theme:?}; expected one of {ALLOWED_THEMES:?}"
        ));
    }
    cache::settings::set(db, KEY_THEME, theme).map_err(|e| e.to_string())
}

// ----- Font size -----

pub fn get_font_size_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_FONT_SIZE).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_FONT_SIZES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_FONT_SIZE.to_string()),
    }
}

pub fn set_font_size_inner(db: &Db, size: &str) -> Result<(), String> {
    if !ALLOWED_FONT_SIZES.contains(&size) {
        return Err(format!(
            "invalid font_size {size:?}; expected one of {ALLOWED_FONT_SIZES:?}"
        ));
    }
    cache::settings::set(db, KEY_FONT_SIZE, size).map_err(|e| e.to_string())
}

// ----- Density -----

pub fn get_density_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_DENSITY).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_DENSITIES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_DENSITY.to_string()),
    }
}

pub fn set_density_inner(db: &Db, density: &str) -> Result<(), String> {
    if !ALLOWED_DENSITIES.contains(&density) {
        return Err(format!(
            "invalid density {density:?}; expected one of {ALLOWED_DENSITIES:?}"
        ));
    }
    cache::settings::set(db, KEY_DENSITY, density).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Tauri commands.
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_daily_goal(state: tauri::State<'_, AppState>) -> Result<i64, String> {
    get_daily_goal_inner(&state.db)
}

#[tauri::command]
pub async fn set_daily_goal(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    seconds: i64,
) -> Result<(), String> {
    set_daily_goal_inner(&state.db, seconds)?;
    let _ = app.emit("prefs-changed", "daily_goal_seconds");
    Ok(())
}

#[tauri::command]
pub async fn set_widget_format(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    format: String,
) -> Result<(), String> {
    set_widget_format_inner(&state.db, &format)?;
    let _ = app.emit("prefs-changed", "widget_format");
    Ok(())
}

#[tauri::command]
pub async fn set_app_icon(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    icon: String,
) -> Result<(), String> {
    set_app_icon_inner(&state.db, &icon)?;
    let _ = app.emit("prefs-changed", "app_icon");
    Ok(())
}

#[tauri::command]
pub async fn get_hourly_rate(state: tauri::State<'_, AppState>) -> Result<f64, String> {
    get_hourly_rate_inner(&state.db)
}

#[tauri::command]
pub async fn set_hourly_rate(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    rate: f64,
) -> Result<(), String> {
    set_hourly_rate_inner(&state.db, rate)?;
    let _ = app.emit("prefs-changed", "hourly_rate");
    Ok(())
}

// ----- Appearance prefs (Phase 11A) -----

#[tauri::command]
pub async fn get_theme(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_theme_inner(&state.db)
}

#[tauri::command]
pub async fn set_theme(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    theme: String,
) -> Result<(), String> {
    set_theme_inner(&state.db, &theme)?;
    let _ = app.emit("prefs-changed", "theme");
    Ok(())
}

#[tauri::command]
pub async fn get_font_size(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_font_size_inner(&state.db)
}

#[tauri::command]
pub async fn set_font_size(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    size: String,
) -> Result<(), String> {
    set_font_size_inner(&state.db, &size)?;
    let _ = app.emit("prefs-changed", "font_size");
    Ok(())
}

#[tauri::command]
pub async fn get_density(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_density_inner(&state.db)
}

#[tauri::command]
pub async fn set_density(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    density: String,
) -> Result<(), String> {
    set_density_inner(&state.db, &density)?;
    let _ = app.emit("prefs-changed", "density");
    Ok(())
}
