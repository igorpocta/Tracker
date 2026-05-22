//! Time helpers shared by sync, mark-and-sweep and date-range filtering.
//!
//! The "application day" is the user's local calendar day, not the UTC day.
//! Sync filtering, mark-and-sweep windows and tombstone scope must all use
//! the same bounds — having Jira compute one window and Freelo another led
//! to worklogs around local midnight being dropped or tombstoned by mistake.

use chrono::{Local, NaiveDate, TimeZone};

/// Inclusive `[from_unix_s, to_unix_s]` UTC seconds for the LOCAL calendar
/// range `[from_date, to_date]`. `from_unix_s` is `from_date` 00:00:00 in
/// the system local timezone; `to_unix_s` is `to_date` 23:59:59 in the
/// system local timezone.
///
/// Returns `None` if either local datetime is ambiguous (DST fall-back) or
/// non-existent (DST spring-forward). Callers must surface this to the user
/// rather than silently widening or narrowing the window.
pub fn local_day_bounds(from_date: NaiveDate, to_date: NaiveDate) -> Option<(i64, i64)> {
    local_day_bounds_in_tz(&Local, from_date, to_date)
}

/// Test seam — same as [`local_day_bounds`] but parameterised over the
/// timezone so tests can pin a fixed offset.
pub fn local_day_bounds_in_tz<TZ: TimeZone>(
    tz: &TZ,
    from_date: NaiveDate,
    to_date: NaiveDate,
) -> Option<(i64, i64)> {
    let from_dt = from_date.and_hms_opt(0, 0, 0)?;
    let to_dt = to_date.and_hms_opt(23, 59, 59)?;
    let from_ts = tz.from_local_datetime(&from_dt).single()?.timestamp();
    let to_ts = tz.from_local_datetime(&to_dt).single()?.timestamp();
    Some((from_ts, to_ts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    #[test]
    fn cest_day_bounds_match_local_midnight() {
        let cest = FixedOffset::east_opt(2 * 3600).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let (from, to) = local_day_bounds_in_tz(&cest, day, day).unwrap();
        // 2026-05-14 00:00:00 +02:00 == 1778709600
        assert_eq!(from, 1_778_709_600);
        // 2026-05-14 23:59:59 +02:00 == 1778795999
        assert_eq!(to, 1_778_795_999);
        assert_eq!(to - from, 24 * 3600 - 1);
    }

    #[test]
    fn after_local_midnight_is_inside_that_local_day() {
        let cest = FixedOffset::east_opt(2 * 3600).unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let (from, to) = local_day_bounds_in_tz(&cest, day, day).unwrap();
        // 00:30 local CEST on 2026-05-14
        let started_ts = cest
            .with_ymd_and_hms(2026, 5, 14, 0, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        assert!(
            started_ts >= from && started_ts <= to,
            "00:30 local must fall inside the local day window"
        );
    }

    #[test]
    fn just_before_local_midnight_is_inside_previous_day() {
        let cest = FixedOffset::east_opt(2 * 3600).unwrap();
        let prev = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
        let (from, to) = local_day_bounds_in_tz(&cest, prev, prev).unwrap();
        let started_ts = cest
            .with_ymd_and_hms(2026, 5, 13, 23, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        assert!(started_ts >= from && started_ts <= to);
    }

    #[test]
    fn multi_day_range_is_contiguous() {
        let cest = FixedOffset::east_opt(2 * 3600).unwrap();
        let a = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        let b = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
        let (from, to) = local_day_bounds_in_tz(&cest, a, b).unwrap();
        assert_eq!(to - from, 3 * 24 * 3600 - 1);
    }
}
