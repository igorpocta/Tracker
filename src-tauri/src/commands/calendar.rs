//! Calendar / non-working days commands (Phase 18A — Item 2).
//!
//! Backs the per-day "I don't work on this day" feature plus the configurable
//! working week mask. The frontend wires the calendar context-menu "tento den
//! nepracuji" toggle and the working-week chip in Phase 18B.

use chrono::NaiveDate;
use tauri::Emitter;

use crate::cache::{self, calendar::NonWorkingDay};
use crate::state::AppState;

const ALLOWED_REASONS: &[&str] = &["holiday", "personal", "vacation"];

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| format!("invalid date {s:?}: {e}"))
}

#[tauri::command]
pub async fn get_working_week_mask(state: tauri::State<'_, AppState>) -> Result<i32, String> {
    cache::calendar::get_working_week_mask(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_working_week_mask(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mask: i32,
) -> Result<(), String> {
    if !(0..=127).contains(&mask) {
        return Err(format!(
            "Maska pracovních dnů musí být 0 až 127 (zadáno {mask})"
        ));
    }
    cache::calendar::set_working_week_mask(&state.db, mask).map_err(|e| e.to_string())?;
    let _ = app.emit("prefs-changed", "working_week_mask");
    Ok(())
}

#[tauri::command]
pub async fn list_non_working_days(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<Vec<NonWorkingDay>, String> {
    let from = parse_date(&from_date)?;
    let to = parse_date(&to_date)?;
    if to < from {
        return Err("to_date is before from_date".into());
    }
    cache::calendar::list_non_working_days(&state.db, from, to).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_non_working_day(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    date: String,
    reason: String,
    label: Option<String>,
) -> Result<(), String> {
    if !ALLOWED_REASONS.contains(&reason.as_str()) {
        return Err(format!(
            "Neplatný důvod {reason:?}; očekáváno {ALLOWED_REASONS:?}"
        ));
    }
    let d = parse_date(&date)?;
    cache::calendar::add_non_working_day(&state.db, d, &reason, label.as_deref())
        .map_err(|e| e.to_string())?;
    let _ = app.emit("calendar-changed", &date);
    Ok(())
}

#[tauri::command]
pub async fn remove_non_working_day(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    date: String,
) -> Result<(), String> {
    let d = parse_date(&date)?;
    cache::calendar::remove_non_working_day(&state.db, d).map_err(|e| e.to_string())?;
    let _ = app.emit("calendar-changed", &date);
    Ok(())
}

#[tauri::command]
pub async fn is_working_day(
    state: tauri::State<'_, AppState>,
    date: String,
) -> Result<bool, String> {
    let d = parse_date(&date)?;
    cache::calendar::is_working_day(&state.db, d).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_date_accepts_iso() {
        assert!(parse_date("2026-05-14").is_ok());
        assert!(parse_date("2000-01-01").is_ok());
    }

    #[test]
    fn parse_date_rejects_bad_input() {
        assert!(parse_date("").is_err());
        assert!(parse_date("yesterday").is_err());
        assert!(parse_date("2026/05/14").is_err());
        assert!(parse_date("14-05-2026").is_err());
    }

    #[test]
    fn parse_date_rejects_impossible() {
        assert!(parse_date("2026-13-01").is_err());
        assert!(parse_date("2026-02-30").is_err());
    }

    #[test]
    fn working_week_mask_range_check() {
        assert!((0..=127).contains(&0));
        assert!((0..=127).contains(&31)); // Mon-Fri
        assert!((0..=127).contains(&127));
        assert!(!(0..=127).contains(&-1));
        assert!(!(0..=127).contains(&128));
    }

    #[test]
    fn allowed_reasons_set_is_stable() {
        for r in ["holiday", "personal", "vacation"] {
            assert!(ALLOWED_REASONS.contains(&r));
        }
        assert!(!ALLOWED_REASONS.contains(&"weekend"));
    }
}
