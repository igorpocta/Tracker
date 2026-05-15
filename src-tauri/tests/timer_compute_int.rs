//! Unit tests for the pure `compute_stop_outcome` helper extracted in
//! Phase A5 from `stop_timer_inner`. No DB, no Tauri State, no HTTP — just
//! integer arithmetic and UTC date comparison.
//!
//! The behavior pinned here matches the pre-extraction semantics of
//! `record_local_stop` (clamp negative durations to 0,
//! `ended_at = started + duration`) plus the historical `apply_rounding`
//! contract for the `"up"` mode (`commands::rounding::apply_rounding`).

use chrono::{NaiveDate, TimeZone, Utc};
use tracker_lib::commands::timer::{compute_stop_outcome, StopOutcome};

fn unix(date: (i32, u32, u32), time: (u32, u32, u32)) -> i64 {
    Utc.with_ymd_and_hms(date.0, date.1, date.2, time.0, time.1, time.2)
        .unwrap()
        .timestamp()
}

#[test]
fn no_rounding_returns_raw_duration() {
    let start = unix((2026, 5, 15), (9, 0, 0));
    let end = unix((2026, 5, 15), (9, 14, 0)); // 14 min later
    let outcome = compute_stop_outcome(start, end, 0, 0);
    assert_eq!(
        outcome,
        StopOutcome {
            started_at: start,
            ended_at: start + 14 * 60,
            raw_duration_s: 14 * 60,
            rounded_duration_s: 14 * 60,
            rolled_over_to_next_day: false,
        }
    );
}

#[test]
fn fifteen_minute_round_up_pushes_14min_to_15min() {
    let start = unix((2026, 5, 15), (9, 0, 0));
    let end = start + 14 * 60;
    let outcome = compute_stop_outcome(start, end, 15, 0);
    assert_eq!(outcome.raw_duration_s, 14 * 60);
    assert_eq!(outcome.rounded_duration_s, 15 * 60);
    assert_eq!(outcome.ended_at, start + 15 * 60);
    assert!(!outcome.rolled_over_to_next_day);
}

#[test]
fn fifteen_minute_round_up_pushes_16min_to_30min() {
    let start = unix((2026, 5, 15), (9, 0, 0));
    let end = start + 16 * 60;
    let outcome = compute_stop_outcome(start, end, 15, 0);
    assert_eq!(outcome.raw_duration_s, 16 * 60);
    assert_eq!(outcome.rounded_duration_s, 30 * 60);
    assert_eq!(outcome.ended_at, start + 30 * 60);
}

#[test]
fn round_up_preserves_exact_multiples() {
    // Already exactly on the 15-min boundary — must NOT bump up another step.
    let start = unix((2026, 5, 15), (9, 0, 0));
    let outcome = compute_stop_outcome(start, start + 30 * 60, 15, 0);
    assert_eq!(outcome.rounded_duration_s, 30 * 60);
}

#[test]
fn zero_duration_with_rounding_stays_zero() {
    // The legacy `apply_rounding("up", _)` has a `if d == 0 return 0` guard;
    // pinning that same edge case here.
    let start = unix((2026, 5, 15), (9, 0, 0));
    let outcome = compute_stop_outcome(start, start, 15, 0);
    assert_eq!(outcome.raw_duration_s, 0);
    assert_eq!(outcome.rounded_duration_s, 0);
    assert_eq!(outcome.ended_at, start);
    assert!(!outcome.rolled_over_to_next_day);
}

#[test]
fn negative_duration_clamps_to_zero() {
    // Clock skew or a stale timer that's somehow ahead of now: pre-extraction
    // code in `record_local_stop` did `(now_s - started_at_s).max(0)` and the
    // helper preserves that clamp.
    let start = unix((2026, 5, 15), (10, 0, 0));
    let end = unix((2026, 5, 15), (9, 0, 0)); // 1 h before start
    let outcome = compute_stop_outcome(start, end, 0, 0);
    assert_eq!(outcome.raw_duration_s, 0);
    assert_eq!(outcome.rounded_duration_s, 0);
    assert_eq!(
        outcome.ended_at, start,
        "ended_at == started_at when clamped"
    );
}

#[test]
fn day_rollover_flag_set_when_end_crosses_utc_midnight() {
    // Start at 23:50 UTC, run for 30 minutes → ends 00:20 next UTC day.
    let start = unix((2026, 5, 15), (23, 50, 0));
    let end = start + 30 * 60;
    let outcome = compute_stop_outcome(start, end, 0, 0);
    assert!(
        outcome.rolled_over_to_next_day,
        "23:50+30m crosses UTC midnight"
    );
    // Sanity: dates differ.
    let start_date = Utc.timestamp_opt(start, 0).unwrap().date_naive();
    let end_date = Utc.timestamp_opt(outcome.ended_at, 0).unwrap().date_naive();
    assert_eq!(start_date, NaiveDate::from_ymd_opt(2026, 5, 15).unwrap());
    assert_eq!(end_date, NaiveDate::from_ymd_opt(2026, 5, 16).unwrap());
}

#[test]
fn rounding_can_force_a_day_rollover() {
    // Start at 23:55 UTC, raw duration 4 min (no rollover), but 15-min
    // round-up bumps it to 15 min → ends at 00:10 next day.
    let start = unix((2026, 5, 15), (23, 55, 0));
    let end = start + 4 * 60;
    let outcome = compute_stop_outcome(start, end, 15, 0);
    assert_eq!(outcome.raw_duration_s, 4 * 60);
    assert_eq!(outcome.rounded_duration_s, 15 * 60);
    assert!(
        outcome.rolled_over_to_next_day,
        "rounded ended_at crosses UTC midnight even though raw didn't"
    );
}

#[test]
fn same_utc_day_does_not_rollover() {
    let start = unix((2026, 5, 15), (8, 0, 0));
    let end = start + 6 * 3600; // 14:00 same day
    let outcome = compute_stop_outcome(start, end, 0, 0);
    assert!(!outcome.rolled_over_to_next_day);
}

#[test]
fn undo_window_arg_does_not_affect_outcome() {
    // Documented as informational — pin that pinning. Helper should be a
    // pure function of (start, now, rounding_minutes).
    let start = unix((2026, 5, 15), (9, 0, 0));
    let end = start + 600;
    let a = compute_stop_outcome(start, end, 5, 0);
    let b = compute_stop_outcome(start, end, 5, 5);
    let c = compute_stop_outcome(start, end, 5, u32::MAX);
    assert_eq!(a, b);
    assert_eq!(b, c);
}
