//! Daily activity rollup (Phase 18A).
//!
//! Pure storage: the per-day row is incremented by
//! [`record_active_chunk`] / [`record_inactive_chunk`]. The frontend posts
//! activity events (mouse, keyboard) at a debounced 30-second cadence; the
//! backend translates a stream of timestamps into "active" and "inactive"
//! chunks based on the configured threshold.

use std::sync::Mutex;

use super::db::{Db, DbError};
use super::settings;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

const KEY_THRESHOLD_MIN: &str = "activity_threshold_min";
pub const DEFAULT_THRESHOLD_MIN: i32 = 5;

/// In-process state shared between user-activity events to compute gaps.
#[derive(Debug, Default)]
pub struct ActivityRecorder {
    last_event_ms: Mutex<Option<i64>>,
}

impl ActivityRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process an incoming activity timestamp. Returns the `(active, inactive)`
    /// chunk in seconds attributed to the day, both `>= 0`. The first call
    /// always returns `(0, 0)` because there's no previous timestamp to
    /// compute a gap against.
    pub fn ingest(&self, now_ms: i64, threshold_min: i32) -> (i64, i64) {
        let mut last = self.last_event_ms.lock().unwrap();
        let prev = *last;
        *last = Some(now_ms);
        match prev {
            None => (0, 0),
            Some(p) => {
                let gap_s = (now_ms - p).max(0) / 1000;
                let threshold_s = (threshold_min as i64).max(1) * 60;
                if gap_s <= threshold_s {
                    (gap_s, 0)
                } else {
                    (threshold_s, gap_s - threshold_s)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyActivityRow {
    pub date: String,
    pub active_seconds: i64,
    pub inactive_seconds: i64,
    pub updated_at: i64,
}

pub fn get_threshold_min(db: &Db) -> Result<i32, DbError> {
    match settings::get(db, KEY_THRESHOLD_MIN)? {
        Some(v) => Ok(v.parse::<i32>().unwrap_or(DEFAULT_THRESHOLD_MIN)),
        None => Ok(DEFAULT_THRESHOLD_MIN),
    }
}

pub fn set_threshold_min(db: &Db, min: i32) -> Result<(), DbError> {
    settings::set(db, KEY_THRESHOLD_MIN, &min.to_string())
}

pub fn get_by_date(db: &Db, date: NaiveDate) -> Result<DailyActivityRow, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT date, active_seconds, inactive_seconds, updated_at
         FROM daily_activity WHERE date = ?1",
    )?;
    match stmt.query_row([date.to_string()], |r| {
        Ok(DailyActivityRow {
            date: r.get(0)?,
            active_seconds: r.get(1)?,
            inactive_seconds: r.get(2)?,
            updated_at: r.get(3)?,
        })
    }) {
        Ok(row) => Ok(row),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(DailyActivityRow {
            date: date.to_string(),
            ..Default::default()
        }),
        Err(e) => Err(e.into()),
    }
}

pub fn record_active_chunk(db: &Db, date: NaiveDate, seconds: i64) -> Result<(), DbError> {
    if seconds <= 0 {
        return Ok(());
    }
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO daily_activity (date, active_seconds, inactive_seconds, updated_at)
         VALUES (?1, ?2, 0, ?3)
         ON CONFLICT(date) DO UPDATE SET
             active_seconds = active_seconds + ?2,
             updated_at = ?3",
        rusqlite::params![date.to_string(), seconds, now],
    )?;
    Ok(())
}

pub fn record_inactive_chunk(db: &Db, date: NaiveDate, seconds: i64) -> Result<(), DbError> {
    if seconds <= 0 {
        return Ok(());
    }
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO daily_activity (date, active_seconds, inactive_seconds, updated_at)
         VALUES (?1, 0, ?2, ?3)
         ON CONFLICT(date) DO UPDATE SET
             inactive_seconds = inactive_seconds + ?2,
             updated_at = ?3",
        rusqlite::params![date.to_string(), seconds, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn default_threshold() {
        let db = open_db();
        assert_eq!(get_threshold_min(&db).unwrap(), DEFAULT_THRESHOLD_MIN);
    }

    #[test]
    fn threshold_roundtrip() {
        let db = open_db();
        set_threshold_min(&db, 10).unwrap();
        assert_eq!(get_threshold_min(&db).unwrap(), 10);
    }

    #[test]
    fn record_active_creates_then_increments() {
        let db = open_db();
        let d = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        record_active_chunk(&db, d, 60).unwrap();
        record_active_chunk(&db, d, 30).unwrap();
        let r = get_by_date(&db, d).unwrap();
        assert_eq!(r.active_seconds, 90);
        assert_eq!(r.inactive_seconds, 0);
    }

    #[test]
    fn record_inactive_creates_then_increments() {
        let db = open_db();
        let d = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        record_inactive_chunk(&db, d, 120).unwrap();
        record_active_chunk(&db, d, 60).unwrap();
        let r = get_by_date(&db, d).unwrap();
        assert_eq!(r.active_seconds, 60);
        assert_eq!(r.inactive_seconds, 120);
    }

    #[test]
    fn zero_or_negative_is_noop() {
        let db = open_db();
        let d = NaiveDate::from_ymd_opt(2026, 5, 14).unwrap();
        record_active_chunk(&db, d, 0).unwrap();
        record_active_chunk(&db, d, -10).unwrap();
        let r = get_by_date(&db, d).unwrap();
        assert_eq!(r.active_seconds, 0);
    }
}
