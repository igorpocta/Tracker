//! User-activity tracking commands (Phase 18A — Item 32).
//!
//! The frontend wires global `mousemove` / `keydown` listeners and calls
//! `record_user_activity` once every 30 seconds (debounced). The backend
//! aggregates these into `active_seconds` for the current local date; when
//! enough wall time passes between events to cross the configured threshold,
//! the gap is recorded as `inactive_seconds`.
//!
//! This is **not** billing input — it's purely informational (the Goals view
//! surfaces an active/inactive ratio).

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::cache::{self, activity::DailyActivityRow};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityIngestResult {
    pub active_added: i64,
    pub inactive_added: i64,
}

#[tauri::command]
pub async fn record_user_activity(
    state: tauri::State<'_, AppState>,
    timestamp_ms: i64,
) -> Result<ActivityIngestResult, String> {
    let threshold = cache::activity::get_threshold_min(&state.db).map_err(|e| e.to_string())?;
    let (active, inactive) = state.activity_recorder.ingest(timestamp_ms, threshold);
    let today = Local::now().date_naive();
    if active > 0 {
        cache::activity::record_active_chunk(&state.db, today, active)
            .map_err(|e| e.to_string())?;
    }
    if inactive > 0 {
        cache::activity::record_inactive_chunk(&state.db, today, inactive)
            .map_err(|e| e.to_string())?;
    }
    Ok(ActivityIngestResult {
        active_added: active,
        inactive_added: inactive,
    })
}

#[tauri::command]
pub async fn get_daily_activity(
    state: tauri::State<'_, AppState>,
    date: String,
) -> Result<DailyActivityRow, String> {
    let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| format!("invalid date {date:?}: {e}"))?;
    cache::activity::get_by_date(&state.db, d).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_activity_threshold_min(
    state: tauri::State<'_, AppState>,
) -> Result<i32, String> {
    cache::activity::get_threshold_min(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_activity_threshold_min(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    min: i32,
) -> Result<(), String> {
    if !(1..=120).contains(&min) {
        return Err(format!("threshold must be 1..=120, got {min}"));
    }
    cache::activity::set_threshold_min(&state.db, min).map_err(|e| e.to_string())?;
    let _ = app.emit("prefs-changed", "activity_threshold_min");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::cache::activity::ActivityRecorder;

    #[test]
    fn first_event_records_nothing() {
        let r = ActivityRecorder::new();
        let (a, i) = r.ingest(1_000_000, 5);
        assert_eq!(a, 0);
        assert_eq!(i, 0);
    }

    #[test]
    fn short_gap_is_all_active() {
        let r = ActivityRecorder::new();
        r.ingest(1_000_000, 5);
        let (a, i) = r.ingest(1_000_000 + 30_000, 5);
        assert_eq!(a, 30);
        assert_eq!(i, 0);
    }

    #[test]
    fn long_gap_splits_into_active_threshold_then_inactive() {
        let r = ActivityRecorder::new();
        r.ingest(1_000_000, 5);
        let (a, i) = r.ingest(1_000_000 + 600_000, 5);
        assert_eq!(a, 300);
        assert_eq!(i, 300);
    }
}
