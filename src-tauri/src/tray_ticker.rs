//! Long-lived background task that keeps the tray icon + tooltip in sync with
//! the active timer.
//!
//! Started once during app setup. Polls the DB every second:
//! - If a timer is running, swaps to the running icon and updates the tooltip
//!   to `Tracker — HH:MM:SS`.
//! - If no timer is running, reverts to the idle icon and tooltip.
//!
//! Polling (vs. an event-driven approach) trades a negligible amount of CPU
//! for simplicity: no shared join handle to thread through `AppState`, no need
//! to abort/respawn on every timer change.

use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime};

use crate::cache::timer as timer_cache;
use crate::state::AppState;

/// Spawn the background ticker. Returns immediately; the task lives for the
/// lifetime of the Tokio runtime (which matches the app's lifetime under
/// Tauri's setup model).
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut last_running: Option<bool> = None;
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        // Don't fire on the first tick so the very first update reflects real
        // wall time, not the moment we spawned.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            // Snapshot the active timer; bail silently on transient DB errors.
            let timer = {
                let state: tauri::State<'_, AppState> = match app.try_state::<AppState>() {
                    Some(s) => s,
                    None => continue, // AppState not yet managed — early tick.
                };
                match timer_cache::get(&state.db) {
                    Ok(t) => t,
                    Err(_) => continue,
                }
            };

            let (running, tooltip, title) = match timer {
                Some(t) => {
                    let now_s = Utc::now().timestamp();
                    let elapsed = (now_s - t.started_at).max(0);
                    // Phase 18A — Item 34: menu bar title = `{KEY} HH:MM`.
                    // Unassigned timer (empty key) shows ⚠ instead of a code.
                    let key_part = if t.issue_key.is_empty() {
                        "⚠".to_string()
                    } else {
                        t.issue_key.clone()
                    };
                    let title = format!("{} {}", key_part, format_hm(elapsed));
                    (
                        true,
                        format!("Tracker — {}", format_hms(elapsed)),
                        Some(title),
                    )
                }
                None => (false, "Tracker — nečinný".to_string(), None),
            };

            // Icon swap is relatively expensive; only do it when running state
            // actually changes.
            if last_running != Some(running) {
                let _ = crate::tray::set_running_visual(&app, running);
                last_running = Some(running);
            }
            let _ = crate::tray::set_tooltip(&app, &tooltip);
            let _ = crate::tray::set_title(&app, title.as_deref());
        }
    });
}

/// Format a non-negative seconds value as `HH:MM`. Used for the menu-bar
/// title (the seconds digits get truncated on macOS).
pub fn format_hm(seconds: i64) -> String {
    let s = seconds.max(0);
    let hours = s / 3600;
    let minutes = (s % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

/// Format a non-negative seconds value as `HH:MM:SS`. Pulled out so it can be
/// unit-tested without standing up the whole app.
pub fn format_hms(seconds: i64) -> String {
    let s = seconds.max(0);
    let hours = s / 3600;
    let minutes = (s % 3600) / 60;
    let secs = s % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_hms_zero() {
        assert_eq!(format_hms(0), "00:00:00");
    }

    #[test]
    fn format_hms_negative_is_clamped() {
        assert_eq!(format_hms(-42), "00:00:00");
    }

    #[test]
    fn format_hms_basic() {
        assert_eq!(format_hms(61), "00:01:01");
        assert_eq!(format_hms(3600), "01:00:00");
        assert_eq!(format_hms(3661), "01:01:01");
    }

    #[test]
    fn format_hms_large() {
        // 100 hours fits without truncation.
        assert_eq!(format_hms(100 * 3600 + 23 * 60 + 45), "100:23:45");
    }

    #[test]
    fn format_hm_truncates_seconds() {
        assert_eq!(format_hm(0), "00:00");
        assert_eq!(format_hm(59), "00:00");
        assert_eq!(format_hm(60), "00:01");
        assert_eq!(format_hm(3661), "01:01");
        assert_eq!(format_hm(-10), "00:00");
    }
}
