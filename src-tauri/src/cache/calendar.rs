//! Non-working days + working-week mask (Phase 18A).
//!
//! Working-week mask is stored in `app_settings` under `working_week_mask`
//! (bitmask: Mon=1, Tue=2, Wed=4, Thu=8, Fri=16, Sat=32, Sun=64). Default
//! `31` = Mon–Fri.
//!
//! Non-working days are explicit per-date exceptions stored in
//! `non_working_days` and override the mask for that specific date.

use super::db::{Db, DbError};
use super::settings;
use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

const KEY_WORKING_WEEK_MASK: &str = "working_week_mask";
pub const DEFAULT_WORKING_WEEK_MASK: i32 = 31; // Mon–Fri

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonWorkingDay {
    pub date: String,
    pub reason: String,
    pub label: Option<String>,
    pub created_at: i64,
}

pub fn get_working_week_mask(db: &Db) -> Result<i32, DbError> {
    match settings::get(db, KEY_WORKING_WEEK_MASK)? {
        Some(v) => Ok(v.parse::<i32>().unwrap_or(DEFAULT_WORKING_WEEK_MASK)),
        None => Ok(DEFAULT_WORKING_WEEK_MASK),
    }
}

pub fn set_working_week_mask(db: &Db, mask: i32) -> Result<(), DbError> {
    settings::set(db, KEY_WORKING_WEEK_MASK, &mask.to_string())
}

/// Bit value for a given weekday (Mon=1, Sun=64).
pub fn weekday_bit(w: Weekday) -> i32 {
    match w {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 4,
        Weekday::Thu => 8,
        Weekday::Fri => 16,
        Weekday::Sat => 32,
        Weekday::Sun => 64,
    }
}

/// True iff the working-week mask marks `date`'s weekday as a working day.
/// Does NOT consult the non-working-days exception list — call
/// [`is_working_day`] for the combined check.
pub fn mask_says_working(mask: i32, date: NaiveDate) -> bool {
    (mask & weekday_bit(date.weekday())) != 0
}

pub fn add_non_working_day(
    db: &Db,
    date: NaiveDate,
    reason: &str,
    label: Option<&str>,
) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT OR REPLACE INTO non_working_days (date, reason, label, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![date.to_string(), reason, label, now],
    )?;
    Ok(())
}

pub fn remove_non_working_day(db: &Db, date: NaiveDate) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM non_working_days WHERE date = ?1",
        [date.to_string()],
    )?;
    Ok(())
}

pub fn list_non_working_days(
    db: &Db,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<NonWorkingDay>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT date, reason, label, created_at
         FROM non_working_days
         WHERE date BETWEEN ?1 AND ?2
         ORDER BY date ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![from.to_string(), to.to_string()], |r| {
        Ok(NonWorkingDay {
            date: r.get(0)?,
            reason: r.get(1)?,
            label: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn is_non_working(db: &Db, date: NaiveDate) -> Result<bool, DbError> {
    let conn = db.pool().get()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM non_working_days WHERE date = ?1",
        [date.to_string()],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Combined check: a day is a working day iff the weekday mask says so AND it
/// is not in the non-working-days exception list.
pub fn is_working_day(db: &Db, date: NaiveDate) -> Result<bool, DbError> {
    let mask = get_working_week_mask(db)?;
    if !mask_says_working(mask, date) {
        return Ok(false);
    }
    if is_non_working(db, date)? {
        return Ok(false);
    }
    Ok(true)
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
    fn default_mask_is_monday_through_friday() {
        let db = open_db();
        assert_eq!(get_working_week_mask(&db).unwrap(), 31);
    }

    #[test]
    fn mask_can_be_changed() {
        let db = open_db();
        set_working_week_mask(&db, 14).unwrap(); // Tue–Thu
        assert_eq!(get_working_week_mask(&db).unwrap(), 14);
    }

    #[test]
    fn mask_says_working_for_each_weekday() {
        // 31 = Mon|Tue|Wed|Thu|Fri
        let mon = NaiveDate::from_ymd_opt(2026, 5, 11).unwrap();
        let sat = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();
        assert!(mask_says_working(31, mon));
        assert!(!mask_says_working(31, sat));
    }

    #[test]
    fn non_working_day_roundtrip() {
        let db = open_db();
        let d = NaiveDate::from_ymd_opt(2026, 12, 24).unwrap();
        add_non_working_day(&db, d, "holiday", Some("Štědrý den")).unwrap();
        assert!(is_non_working(&db, d).unwrap());

        let list = list_non_working_days(
            &db,
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
        )
        .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].reason, "holiday");
        assert_eq!(list[0].label.as_deref(), Some("Štědrý den"));

        remove_non_working_day(&db, d).unwrap();
        assert!(!is_non_working(&db, d).unwrap());
    }

    #[test]
    fn is_working_day_combines_mask_and_exceptions() {
        let db = open_db();
        let weekday_mon = NaiveDate::from_ymd_opt(2026, 5, 11).unwrap();
        let weekday_holiday = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap(); // Fri
        let saturday = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap();

        assert!(is_working_day(&db, weekday_mon).unwrap());
        assert!(!is_working_day(&db, saturday).unwrap());

        add_non_working_day(&db, weekday_holiday, "holiday", Some("Vánoce")).unwrap();
        assert!(!is_working_day(&db, weekday_holiday).unwrap());
    }

    #[test]
    fn custom_mask_tue_fri_marks_monday_as_off() {
        let db = open_db();
        set_working_week_mask(&db, 2 | 4 | 8 | 16).unwrap();
        let mon = NaiveDate::from_ymd_opt(2026, 5, 11).unwrap();
        let tue = NaiveDate::from_ymd_opt(2026, 5, 12).unwrap();
        assert!(!is_working_day(&db, mon).unwrap());
        assert!(is_working_day(&db, tue).unwrap());
    }
}
