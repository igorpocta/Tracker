//! Long-lived background task that keeps the tray icon + tooltip in sync with
//! the active timer.
//!
//! Tick rate: **4 Hz** (250 ms). When a timer is running, every tick swaps the
//! tray icon to the next frame of the recording pulse (red dot fading in/out
//! over ~1.75 s). The title (`{KEY} HH:MM`) is also updated every tick but
//! changes at most once per minute. When idle, the ticker still runs at 4 Hz
//! but only re-applies the static idle icon + title — the per-frame work is a
//! cheap no-op when state hasn't changed.
//!
//! Polling (vs. event-driven) trades a tiny amount of CPU for simplicity: no
//! shared join handle to thread through `AppState`, no abort/respawn dance on
//! every timer change.

use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Manager, Runtime};

use crate::cache::timer as timer_cache;
use crate::state::AppState;
use crate::tray_pulse::FRAME_COUNT;

/// Pulse animation cycle: 7 frames × 250 ms = 1.75 s per breath.
const TICK_INTERVAL_MS: u64 = 250;

/// Spawn the background ticker. Returns immediately; the task lives for the
/// lifetime of the Tokio runtime (which matches the app's lifetime under
/// Tauri's setup model).
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        // Tracks last applied state so we only redraw on change.
        let mut last_running: Option<bool> = None;
        let mut last_pulse_frame: Option<usize> = None;
        let mut last_title: Option<String> = None;

        let mut pulse_idx: usize = 0;
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_INTERVAL_MS));
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

            match timer {
                Some(t) => {
                    let now_s = Utc::now().timestamp();
                    let elapsed = (now_s - t.started_at).max(0);
                    let key_part = if t.issue_key.is_empty() {
                        "⚠".to_string()
                    } else {
                        t.issue_key.clone()
                    };
                    let title = format!("{key_part} {}", format_hm(elapsed));
                    let tooltip = format!("Tracker — {}", format_hms(elapsed));

                    // Cycle the pulse frame index each tick.
                    pulse_idx = (pulse_idx + 1) % FRAME_COUNT;

                    // Always push the new pulse frame (cheap PNG re-encode +
                    // tray.set_icon). Title only changes once a minute, but we
                    // diff to avoid spurious set_title calls.
                    if last_pulse_frame != Some(pulse_idx) {
                        let _ = crate::tray::set_pulse_frame(&app, pulse_idx);
                        last_pulse_frame = Some(pulse_idx);
                    }
                    if last_title.as_deref() != Some(title.as_str()) {
                        let _ = crate::tray::set_title(&app, Some(title.as_str()));
                        last_title = Some(title);
                    }
                    let _ = crate::tray::set_tooltip(&app, &tooltip);

                    last_running = Some(true);
                }
                None => {
                    // Idle. Static stopwatch icon (template, monochrome) +
                    // a sleeping title with a placeholder time. Only redraw
                    // when transitioning from running → idle so the menu-bar
                    // entry doesn't churn while nothing's happening.
                    if last_running != Some(false) {
                        let _ = crate::tray::set_running_visual(&app, false);
                        last_running = Some(false);
                        last_pulse_frame = None;
                        pulse_idx = 0;
                    }
                    let idle_title = "💤 —:—";
                    if last_title.as_deref() != Some(idle_title) {
                        let _ = crate::tray::set_title(&app, Some(idle_title));
                        last_title = Some(idle_title.to_string());
                    }
                    let _ = crate::tray::set_tooltip(&app, "Tracker — nečinný");
                }
            }
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
