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
    "blue",
    "indigo",
    "violet",
    "pink",
    "red",
    "orange",
    "yellow",
    "green",
    "teal",
    "graphite",
    // Mono palettes
    "aurora",
    "trcker",
    "love",
    "halloween",
    // Phase 18B — Item 16: new MONO palettes
    "mocha",
    "electric",
    "forest",
    "plum",
    "rust",
    // Dual palettes
    "czech",
    "aurora-boreal",
    "sakura-night",
    "cyber-lime",
    "nordic-fjord",
    // Dual palettes (2026 additions)
    "tokyo-night",
    "sunset-drive",
    "deep-ocean",
    "royal-velvet",
    "forest-fire",
];
/// Default palette mode.
pub const DEFAULT_PALETTE_MODE: &str = "mono";
/// Allowed palette mode values.
pub const ALLOWED_PALETTE_MODES: &[&str] = &["mono", "dual"];
/// Default currency code.
pub const DEFAULT_CURRENCY: &str = "CZK";
/// Allowed ISO-4217 currency codes.
pub const ALLOWED_CURRENCIES: &[&str] = &["CZK", "EUR", "USD", "GBP", "PLN", "CHF"];
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
/// Auto-sync interval, in seconds. `0` means "manual only" (the background
/// loop skips fetching). The DB stores the integer as a string.
pub const KEY_AUTO_SYNC_INTERVAL: &str = "auto_sync_interval_seconds";
/// Default auto-sync interval: 1 hour.
pub const DEFAULT_AUTO_SYNC_INTERVAL_SECONDS: i64 = 3_600;
/// Allowed auto-sync intervals: manual, 15m, 1h, 4h, daily.
pub const ALLOWED_AUTO_SYNC_INTERVALS: &[i64] =
    &[0, 15 * 60, 60 * 60, 4 * 60 * 60, 24 * 60 * 60];
/// Phase 18B — Item 12: ISO date (YYYY-MM-DD) of the last time we fired the
/// "daily goal reached" notification. Used to dedupe.
pub const KEY_TODAY_GOAL_NOTIFIED_AT: &str = "today_goal_notified_at";

const KEY_POMODORO_ENABLED: &str = "pomodoro_enabled";
const KEY_POMODORO_WORK_MIN: &str = "pomodoro_work_min";
const KEY_POMODORO_BREAK_MIN: &str = "pomodoro_break_min";
const DEFAULT_POMODORO_WORK_MIN: i64 = 25;
const DEFAULT_POMODORO_BREAK_MIN: i64 = 5;
const MIN_POMODORO_WORK_MIN: i64 = 5;
const MAX_POMODORO_WORK_MIN: i64 = 180;
const MIN_POMODORO_BREAK_MIN: i64 = 1;
const MAX_POMODORO_BREAK_MIN: i64 = 60;

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

/// Minimum daily goal: 30 minutes (1800 s).
pub const MIN_DAILY_GOAL_SECONDS: i64 = 30 * 60;
/// Maximum daily goal: 24 hours (86 400 s). Paranoia upper bound.
pub const MAX_DAILY_GOAL_SECONDS: i64 = 24 * 60 * 60;

pub fn set_daily_goal_inner(db: &Db, seconds: i64) -> Result<(), String> {
    if !(MIN_DAILY_GOAL_SECONDS..=MAX_DAILY_GOAL_SECONDS).contains(&seconds) {
        return Err(format!(
            "Denní cíl musí být mezi {} a {} hodinami (zadáno {seconds} s)",
            MIN_DAILY_GOAL_SECONDS / 3600,
            MAX_DAILY_GOAL_SECONDS / 3600
        ));
    }
    cache::settings::set(db, KEY_DAILY_GOAL, &seconds.to_string()).map_err(|e| e.to_string())
}

pub fn get_auto_sync_interval_inner(db: &Db) -> Result<i64, String> {
    match cache::settings::get(db, KEY_AUTO_SYNC_INTERVAL).map_err(|e| e.to_string())? {
        Some(v) => v
            .parse::<i64>()
            .map_err(|_| format!("invalid {KEY_AUTO_SYNC_INTERVAL}: {v}")),
        None => Ok(DEFAULT_AUTO_SYNC_INTERVAL_SECONDS),
    }
}

pub fn set_auto_sync_interval_inner(db: &Db, seconds: i64) -> Result<(), String> {
    if !ALLOWED_AUTO_SYNC_INTERVALS.contains(&seconds) {
        return Err(format!(
            "Neplatný auto-sync interval {seconds}; očekáváno {ALLOWED_AUTO_SYNC_INTERVALS:?}"
        ));
    }
    cache::settings::set(db, KEY_AUTO_SYNC_INTERVAL, &seconds.to_string())
        .map_err(|e| e.to_string())
}

pub const ALLOWED_WIDGET_FORMATS: &[&str] = &["HH:MM:SS", "Hh Mm", "0.0h"];

pub fn set_widget_format_inner(db: &Db, format: &str) -> Result<(), String> {
    if !ALLOWED_WIDGET_FORMATS.contains(&format) {
        return Err(format!(
            "Neplatný formát widgetu {format:?}; očekáváno {ALLOWED_WIDGET_FORMATS:?}"
        ));
    }
    cache::settings::set(db, KEY_WIDGET_FORMAT, format).map_err(|e| e.to_string())
}

/// App icon je krátký identifier (cca [a-z0-9_-], pro výběr ze setu PNG
/// resources). Omezujeme délku + povolené znaky, ať se nezpůsobí path
/// traversal kdyby kdokoliv někdy hodnoty interpretoval jako filename.
pub fn set_app_icon_inner(db: &Db, icon: &str) -> Result<(), String> {
    if icon.is_empty() || icon.len() > 32 {
        return Err("Identifikátor ikony musí mít 1–32 znaků".into());
    }
    if !icon
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Identifikátor ikony obsahuje neplatné znaky".into());
    }
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

/// Upper bound on the hourly rate. Past this we assume user error (typo,
/// pasting in a yearly figure, accidentally hitting `e` in the input, etc.).
/// Pokud někdy budeme měnu s hyperinflací, zvedne se per-currency
/// (tj. v ConvertToCZK mode).
pub const MAX_HOURLY_RATE: f64 = 99_999.0;

pub fn set_hourly_rate_inner(db: &Db, rate: f64) -> Result<(), String> {
    if !rate.is_finite() {
        return Err("Hodinová sazba musí být platné číslo".into());
    }
    if rate < 0.0 {
        return Err("Hodinová sazba nesmí být záporná".into());
    }
    if rate > MAX_HOURLY_RATE {
        return Err(format!(
            "Hodinová sazba je příliš vysoká (max {MAX_HOURLY_RATE})"
        ));
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
            "Neplatný motiv {theme:?}; očekáváno {ALLOWED_THEMES:?}"
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
            "Neplatná velikost písma {size:?}; očekáváno {ALLOWED_FONT_SIZES:?}"
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
            "Neplatná hustota rozhraní {density:?}; očekáváno {ALLOWED_DENSITIES:?}"
        ));
    }
    cache::settings::set(db, KEY_DENSITY, density).map_err(|e| e.to_string())
}

// ----- Pomodoro -----

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PomodoroConfig {
    pub enabled: bool,
    pub work_min: i64,
    pub break_min: i64,
}

pub fn get_pomodoro_inner(db: &Db) -> Result<PomodoroConfig, String> {
    let enabled = matches!(
        cache::settings::get(db, KEY_POMODORO_ENABLED)
            .map_err(|e| e.to_string())?
            .as_deref(),
        Some("true")
    );
    let work_min = cache::settings::get(db, KEY_POMODORO_WORK_MIN)
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_POMODORO_WORK_MIN);
    let break_min = cache::settings::get(db, KEY_POMODORO_BREAK_MIN)
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_POMODORO_BREAK_MIN);
    Ok(PomodoroConfig {
        enabled,
        work_min,
        break_min,
    })
}

pub fn set_pomodoro_inner(db: &Db, cfg: &PomodoroConfig) -> Result<(), String> {
    if !(MIN_POMODORO_WORK_MIN..=MAX_POMODORO_WORK_MIN).contains(&cfg.work_min) {
        return Err(format!(
            "Pomodoro work musí být {}–{} min",
            MIN_POMODORO_WORK_MIN, MAX_POMODORO_WORK_MIN
        ));
    }
    if !(MIN_POMODORO_BREAK_MIN..=MAX_POMODORO_BREAK_MIN).contains(&cfg.break_min) {
        return Err(format!(
            "Pomodoro pauza musí být {}–{} min",
            MIN_POMODORO_BREAK_MIN, MAX_POMODORO_BREAK_MIN
        ));
    }
    cache::settings::set(
        db,
        KEY_POMODORO_ENABLED,
        if cfg.enabled { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())?;
    cache::settings::set(db, KEY_POMODORO_WORK_MIN, &cfg.work_min.to_string())
        .map_err(|e| e.to_string())?;
    cache::settings::set(db, KEY_POMODORO_BREAK_MIN, &cfg.break_min.to_string())
        .map_err(|e| e.to_string())?;
    Ok(())
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
            "Neplatná barva zvýraznění {accent:?}; očekáváno {ALLOWED_ACCENTS:?}"
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
            "Neplatná měna {currency:?}; očekáváno {ALLOWED_CURRENCIES:?}"
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
            "Neplatný režim palety {mode:?}; očekáváno {ALLOWED_PALETTE_MODES:?}"
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
pub async fn get_auto_sync_interval_seconds(
    state: tauri::State<'_, AppState>,
) -> Result<i64, String> {
    get_auto_sync_interval_inner(&state.db)
}

#[tauri::command]
pub async fn set_auto_sync_interval_seconds(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    seconds: i64,
) -> Result<(), String> {
    set_auto_sync_interval_inner(&state.db, seconds)?;
    let _ = app.emit("prefs-changed", KEY_AUTO_SYNC_INTERVAL);
    Ok(())
}

#[tauri::command]
pub async fn get_pomodoro(state: tauri::State<'_, AppState>) -> Result<PomodoroConfig, String> {
    get_pomodoro_inner(&state.db)
}

#[tauri::command]
pub async fn set_pomodoro(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    config: PomodoroConfig,
) -> Result<(), String> {
    set_pomodoro_inner(&state.db, &config)?;
    let _ = app.emit("prefs-changed", "pomodoro");
    Ok(())
}

#[tauri::command]
pub async fn list_project_colors(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<cache::project_colors::ProjectColor>, String> {
    cache::project_colors::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_project_color(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    project_key: String,
    color: Option<String>,
) -> Result<(), String> {
    let key = project_key.trim();
    if key.is_empty() {
        return Err("project_key nesmí být prázdný".into());
    }
    match color.as_deref().map(str::trim) {
        Some(c) if !c.is_empty() => {
            if !is_valid_hex_color(c) {
                return Err(format!("Neplatná barva {c:?}; očekáváno #RRGGBB"));
            }
            cache::project_colors::set(&state.db, key, c).map_err(|e| e.to_string())?;
        }
        _ => {
            // Prázdná barva → odstranit override.
            cache::project_colors::remove(&state.db, key).map_err(|e| e.to_string())?;
        }
    }
    let _ = app.emit("prefs-changed", "project_colors");
    Ok(())
}

fn is_valid_hex_color(s: &str) -> bool {
    if !s.starts_with('#') || (s.len() != 4 && s.len() != 7) {
        return false;
    }
    s[1..].chars().all(|c| c.is_ascii_hexdigit())
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
pub async fn get_day_timeline_visible(state: tauri::State<'_, AppState>) -> Result<bool, String> {
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
pub async fn get_earnings_visible(state: tauri::State<'_, AppState>) -> Result<bool, String> {
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

    // ------- Item 23: validation rejection tests -------

    #[test]
    fn set_hourly_rate_rejects_non_finite() {
        let db = open_db();
        assert!(set_hourly_rate_inner(&db, f64::INFINITY).is_err());
        assert!(set_hourly_rate_inner(&db, f64::NEG_INFINITY).is_err());
        assert!(set_hourly_rate_inner(&db, f64::NAN).is_err());
    }

    #[test]
    fn set_hourly_rate_rejects_negative() {
        let db = open_db();
        assert!(set_hourly_rate_inner(&db, -1.0).is_err());
        assert!(set_hourly_rate_inner(&db, -0.0001).is_err());
    }

    #[test]
    fn set_hourly_rate_rejects_too_large() {
        let db = open_db();
        assert!(set_hourly_rate_inner(&db, MAX_HOURLY_RATE + 0.01).is_err());
        assert!(set_hourly_rate_inner(&db, 1e10).is_err());
    }

    #[test]
    fn set_hourly_rate_accepts_valid_range() {
        let db = open_db();
        assert!(set_hourly_rate_inner(&db, 0.0).is_ok());
        assert!(set_hourly_rate_inner(&db, 1500.0).is_ok());
        assert!(set_hourly_rate_inner(&db, MAX_HOURLY_RATE).is_ok());
    }

    #[test]
    fn set_daily_goal_rejects_out_of_range() {
        let db = open_db();
        // Below min (30 min)
        assert!(set_daily_goal_inner(&db, 0).is_err());
        assert!(set_daily_goal_inner(&db, 60).is_err());
        // Above max (24 h)
        assert!(set_daily_goal_inner(&db, 25 * 3600).is_err());
        // Negative
        assert!(set_daily_goal_inner(&db, -1).is_err());
    }

    #[test]
    fn set_daily_goal_accepts_valid_range() {
        let db = open_db();
        assert!(set_daily_goal_inner(&db, MIN_DAILY_GOAL_SECONDS).is_ok());
        assert!(set_daily_goal_inner(&db, 8 * 3600).is_ok());
        assert!(set_daily_goal_inner(&db, MAX_DAILY_GOAL_SECONDS).is_ok());
    }

    #[test]
    fn set_widget_format_validates_whitelist() {
        let db = open_db();
        for f in ALLOWED_WIDGET_FORMATS {
            assert!(set_widget_format_inner(&db, f).is_ok(), "{f}");
        }
        assert!(set_widget_format_inner(&db, "").is_err());
        assert!(set_widget_format_inner(&db, "<script>").is_err());
        assert!(set_widget_format_inner(&db, "hh:mm").is_err()); // case-sensitive
    }

    #[test]
    fn auto_sync_interval_defaults_to_one_hour() {
        let db = open_db();
        assert_eq!(
            get_auto_sync_interval_inner(&db).unwrap(),
            DEFAULT_AUTO_SYNC_INTERVAL_SECONDS,
        );
    }

    #[test]
    fn set_auto_sync_interval_accepts_whitelist() {
        let db = open_db();
        for &s in ALLOWED_AUTO_SYNC_INTERVALS {
            assert!(set_auto_sync_interval_inner(&db, s).is_ok(), "{s}");
            assert_eq!(get_auto_sync_interval_inner(&db).unwrap(), s);
        }
    }

    #[test]
    fn set_auto_sync_interval_rejects_arbitrary_seconds() {
        let db = open_db();
        // Off-whitelist values must be rejected so the UI dropdown stays the
        // sole source of truth for allowed cadences.
        assert!(set_auto_sync_interval_inner(&db, 1).is_err());
        assert!(set_auto_sync_interval_inner(&db, 30).is_err());
        assert!(set_auto_sync_interval_inner(&db, 7200).is_err());
        assert!(set_auto_sync_interval_inner(&db, -1).is_err());
    }

    #[test]
    fn set_app_icon_validates_charset_and_length() {
        let db = open_db();
        assert!(set_app_icon_inner(&db, "default").is_ok());
        assert!(set_app_icon_inner(&db, "icon-pink_dark").is_ok());
        assert!(set_app_icon_inner(&db, "").is_err());
        assert!(set_app_icon_inner(&db, "../etc/passwd").is_err());
        assert!(set_app_icon_inner(&db, "icon with spaces").is_err());
        assert!(set_app_icon_inner(&db, &"x".repeat(33)).is_err());
    }

    #[test]
    fn set_currency_rejects_invalid() {
        let db = open_db();
        assert!(set_currency_inner(&db, "XYZ").is_err());
        assert!(set_currency_inner(&db, "").is_err());
        assert!(set_currency_inner(&db, "czk").is_err()); // case sensitive
    }

    #[test]
    fn set_currency_accepts_allowed() {
        let db = open_db();
        for c in ALLOWED_CURRENCIES {
            assert!(set_currency_inner(&db, c).is_ok(), "{c} should be ok");
        }
    }

    #[test]
    fn set_theme_rejects_invalid() {
        let db = open_db();
        assert!(set_theme_inner(&db, "midnight").is_err());
        assert!(set_theme_inner(&db, "").is_err());
    }

    #[test]
    fn set_accent_rejects_unknown_palette() {
        let db = open_db();
        assert!(set_accent_color_inner(&db, "puce").is_err());
        assert!(set_accent_color_inner(&db, "").is_err());
    }
}
