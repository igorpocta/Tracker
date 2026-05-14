//! User preference commands backed by the `app_settings` key/value table.

use tauri::Emitter;

use crate::cache::{self, Db};
use crate::state::AppState;

/// Default daily goal: 8 hours, expressed in seconds.
pub const DEFAULT_DAILY_GOAL_SECONDS: i64 = 8 * 60 * 60;
/// Default widget format: `"hh:mm"` (matches the original Tracker product).
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
/// Default accent (palette) identifier: Aurora teal — the Phase 13 default.
pub const DEFAULT_ACCENT: &str = "aurora";
/// Allowed accent / palette identifiers.
///
/// The Phase 11–12 hue names (`blue`, `indigo`, …) are kept for backwards
/// compatibility with existing installs. Phase 13 introduces the named
/// Mono + Dual palettes from the original Tracker reference.
pub const ALLOWED_ACCENTS: &[&str] = &[
    // Legacy hues
    "blue", "indigo", "violet", "pink", "red", "orange", "yellow", "green",
    "teal", "graphite",
    // Mono palettes
    "aurora", "trcker", "love", "halloween",
    // Dual palettes
    "czech", "aurora-boreal", "sakura-night", "cyber-lime", "nordic-fjord",
];
/// Default palette mode.
pub const DEFAULT_PALETTE_MODE: &str = "mono";
/// Allowed palette mode values.
pub const ALLOWED_PALETTE_MODES: &[&str] = &["mono", "dual"];
/// Default currency code.
pub const DEFAULT_CURRENCY: &str = "CZK";
/// Allowed ISO-4217 currency codes.
pub const ALLOWED_CURRENCIES: &[&str] =
    &["CZK", "EUR", "USD", "GBP", "PLN", "CHF"];
/// Default day timeline visibility — visible.
pub const DEFAULT_DAY_TIMELINE_VISIBLE: bool = true;
/// Phase 18B — Item 22: default visibility of the Reports earnings card.
/// `true` means the value is visible by default; `false` keeps it masked
/// with the eye-toggle to reveal.
pub const DEFAULT_EARNINGS_VISIBLE: bool = true;

const KEY_DAILY_GOAL: &str = "daily_goal_seconds";
const KEY_WIDGET_FORMAT: &str = "widget_format";
const KEY_APP_ICON: &str = "app_icon";
const KEY_HOURLY_RATE: &str = "hourly_rate";
const KEY_THEME: &str = "theme";
const KEY_FONT_SIZE: &str = "font_size";
const KEY_DENSITY: &str = "density";
const KEY_ACCENT: &str = "accent_color";
const KEY_CURRENCY: &str = "currency";
const KEY_PALETTE_MODE: &str = "palette_mode";
const KEY_DAY_TIMELINE_VISIBLE: &str = "day_timeline_visible";
const KEY_EARNINGS_VISIBLE: &str = "earnings_visible";
/// Phase 18B — Item 12: ISO date (YYYY-MM-DD) of the last time we fired the
/// "daily goal reached" notification. Used to dedupe.
pub const KEY_TODAY_GOAL_NOTIFIED_AT: &str = "today_goal_notified_at";

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

// ----- Accent color -----

pub fn get_accent_color_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_ACCENT).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_ACCENTS.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_ACCENT.to_string()),
    }
}

pub fn set_accent_color_inner(db: &Db, accent: &str) -> Result<(), String> {
    if !ALLOWED_ACCENTS.contains(&accent) {
        return Err(format!(
            "invalid accent {accent:?}; expected one of {ALLOWED_ACCENTS:?}"
        ));
    }
    cache::settings::set(db, KEY_ACCENT, accent).map_err(|e| e.to_string())
}

// ----- Currency -----

pub fn get_currency_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_CURRENCY).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_CURRENCIES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_CURRENCY.to_string()),
    }
}

pub fn set_currency_inner(db: &Db, currency: &str) -> Result<(), String> {
    if !ALLOWED_CURRENCIES.contains(&currency) {
        return Err(format!(
            "invalid currency {currency:?}; expected one of {ALLOWED_CURRENCIES:?}"
        ));
    }
    cache::settings::set(db, KEY_CURRENCY, currency).map_err(|e| e.to_string())
}

// ----- Palette mode (Phase 13) -----

pub fn get_palette_mode_inner(db: &Db) -> Result<String, String> {
    match cache::settings::get(db, KEY_PALETTE_MODE).map_err(|e| e.to_string())? {
        Some(v) if ALLOWED_PALETTE_MODES.contains(&v.as_str()) => Ok(v),
        _ => Ok(DEFAULT_PALETTE_MODE.to_string()),
    }
}

pub fn set_palette_mode_inner(db: &Db, mode: &str) -> Result<(), String> {
    if !ALLOWED_PALETTE_MODES.contains(&mode) {
        return Err(format!(
            "invalid palette mode {mode:?}; expected one of {ALLOWED_PALETTE_MODES:?}"
        ));
    }
    cache::settings::set(db, KEY_PALETTE_MODE, mode).map_err(|e| e.to_string())
}

// ----- Day timeline visibility (Phase 14) -----

pub fn get_day_timeline_visible_inner(db: &Db) -> Result<bool, String> {
    match cache::settings::get(db, KEY_DAY_TIMELINE_VISIBLE).map_err(|e| e.to_string())? {
        Some(v) => match v.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Ok(DEFAULT_DAY_TIMELINE_VISIBLE),
        },
        None => Ok(DEFAULT_DAY_TIMELINE_VISIBLE),
    }
}

pub fn set_day_timeline_visible_inner(db: &Db, visible: bool) -> Result<(), String> {
    let v = if visible { "true" } else { "false" };
    cache::settings::set(db, KEY_DAY_TIMELINE_VISIBLE, v).map_err(|e| e.to_string())
}

// ----- Earnings visibility (Phase 18B — Item 22) -----

pub fn get_earnings_visible_inner(db: &Db) -> Result<bool, String> {
    match cache::settings::get(db, KEY_EARNINGS_VISIBLE).map_err(|e| e.to_string())? {
        Some(v) => match v.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Ok(DEFAULT_EARNINGS_VISIBLE),
        },
        None => Ok(DEFAULT_EARNINGS_VISIBLE),
    }
}

pub fn set_earnings_visible_inner(db: &Db, visible: bool) -> Result<(), String> {
    let v = if visible { "true" } else { "false" };
    cache::settings::set(db, KEY_EARNINGS_VISIBLE, v).map_err(|e| e.to_string())
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

// ----- Accent color (Phase 12) -----

#[tauri::command]
pub async fn get_accent_color(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_accent_color_inner(&state.db)
}

#[tauri::command]
pub async fn set_accent_color(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    accent: String,
) -> Result<(), String> {
    set_accent_color_inner(&state.db, &accent)?;
    let _ = app.emit("prefs-changed", "accent_color");
    Ok(())
}

// ----- Currency (Phase 12) -----

#[tauri::command]
pub async fn get_currency(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_currency_inner(&state.db)
}

#[tauri::command]
pub async fn set_currency(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    currency: String,
) -> Result<(), String> {
    set_currency_inner(&state.db, &currency)?;
    let _ = app.emit("prefs-changed", "currency");
    Ok(())
}

// ----- Palette mode (Phase 13) -----

#[tauri::command]
pub async fn get_palette_mode(state: tauri::State<'_, AppState>) -> Result<String, String> {
    get_palette_mode_inner(&state.db)
}

#[tauri::command]
pub async fn set_palette_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    set_palette_mode_inner(&state.db, &mode)?;
    let _ = app.emit("prefs-changed", "palette_mode");
    Ok(())
}

// ----- Day timeline visibility (Phase 14) -----

#[tauri::command]
pub async fn get_day_timeline_visible(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    get_day_timeline_visible_inner(&state.db)
}

#[tauri::command]
pub async fn set_day_timeline_visible(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    visible: bool,
) -> Result<(), String> {
    set_day_timeline_visible_inner(&state.db, visible)?;
    let _ = app.emit("prefs-changed", "day_timeline_visible");
    Ok(())
}

// ----- Earnings visibility (Phase 18B — Item 22) -----

#[tauri::command]
pub async fn get_earnings_visible(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    get_earnings_visible_inner(&state.db)
}

#[tauri::command]
pub async fn set_earnings_visible(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    visible: bool,
) -> Result<(), String> {
    set_earnings_visible_inner(&state.db, visible)?;
    let _ = app.emit("prefs-changed", "earnings_visible");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        // Leak the tempdir so the file outlives this helper; tests are short.
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn day_timeline_visible_defaults_true() {
        let db = open_db();
        assert!(get_day_timeline_visible_inner(&db).unwrap());
    }

    #[test]
    fn day_timeline_visible_round_trips() {
        let db = open_db();
        set_day_timeline_visible_inner(&db, false).unwrap();
        assert!(!get_day_timeline_visible_inner(&db).unwrap());
        set_day_timeline_visible_inner(&db, true).unwrap();
        assert!(get_day_timeline_visible_inner(&db).unwrap());
    }
}
