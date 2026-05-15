//! Long-lived background task that keeps the tray title + tooltip in sync
//! with the active timer.
//!
//! The tray ICON is intentionally cleared on startup — only the menu-bar
//! TITLE text is updated. That text carries the recording state ("🔴 KEY
//! 01:23") and goes monochrome with a sleep emoji when idle ("💤 —:—").
//!
//! Tick rate: 1 Hz. The 🔴 emoji alternates with an equal-width invisible
//! braille blank (`U+2800`) every other tick to produce a blink effect
//! without bouncing the text width.
//!
//! Polling (vs. event-driven) trades a tiny amount of CPU for simplicity: no
//! shared join handle to thread through `AppState`, no abort/respawn dance on
//! every timer change.

use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime};

use crate::cache::timer as timer_cache;
use crate::state::AppState;

const TICK_INTERVAL_MS: u64 = 1000;

/// Spawn the background ticker. Returns immediately; the task lives for the
/// lifetime of the Tokio runtime (which matches the app's lifetime under
/// Tauri's setup model).
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        // Clear the declarative icon from tauri.conf.json once on startup so
        // only the title text is visible in the menu bar from here on.
        let _ = crate::tray::clear_icon(&app);

        let mut last_title: Option<String> = None;
        let mut tick: u64 = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_INTERVAL_MS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            tick = tick.wrapping_add(1);

            // Snapshot the active timer; bail silently on transient DB errors.
            let timer = {
                let state: tauri::State<'_, AppState> = match app.try_state::<AppState>() {
                    Some(s) => s,
                    None => continue,
                };
                match timer_cache::get(&state.db) {
                    Ok(t) => t,
                    Err(_) => continue,
                }
            };

            let (title, tooltip) = match timer {
                Some(t) => {
                    let now_s = Utc::now().timestamp();
                    let elapsed = (now_s - t.started_at).max(0);
                    let key_part = if t.issue_key.is_empty() {
                        "⚠".to_string()
                    } else {
                        t.issue_key.clone()
                    };
                    // 🔴 emoji blinks every other second; replaced by an
                    // invisible braille blank so width stays constant.
                    let pulse = if tick % 2 == 0 { "🔴" } else { "\u{2800}\u{2800}" };
                    let title = format!("{pulse} {key_part} {}", format_hm(elapsed));
                    let tooltip = format!("Tracker — {}", format_hms(elapsed));
                    (title, tooltip)
                }
                None => ("💤 —:—".to_string(), "Tracker — nečinný".to_string()),
            };

            if last_title.as_deref() != Some(title.as_str()) {
                let _ = crate::tray::set_title(&app, Some(title.as_str()));
                last_title = Some(title);
            }
            let _ = crate::tray::set_tooltip(&app, &tooltip);
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
