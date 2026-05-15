//! Streaks — počet po sobě jdoucích **pracovních** dní, kdy uživatel
//! splnil daily goal. Used jako "soft motivator" v hlavním okně.
//!
//! Pravidla:
//! - Aktuální streak = počet pracovních dní končící nejpozději *dnes*
//!   (nebo *včera*, pokud dnes ještě nesplnil) v souvislé sérii.
//! - Den splněn = sum(duration_s nad ne-tombstoned worklogs ve `started_at`
//!   intervalu lokálního dne) >= daily_goal_seconds.
//! - Víkendy/non-working days se přeskakují (nepřeruší streak ani se
//!   nepočítají do něj).
//! - Nejdelší streak vrátíme zvlášť, ať jde zobrazit "rekord".

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use serde::{Deserialize, Serialize};

use crate::cache::{self, Db};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Streaks {
    pub current: i64,
    pub longest: i64,
    /// Splněno už dnes? Pomáhá UI rozhodnout, jestli ukázat "+1" indikátor
    /// pro povzbuzení k dotažení dnešního cíle.
    pub today_met: bool,
}

const LOOKBACK_DAYS: i64 = 365;

#[tauri::command]
pub async fn get_streaks(state: tauri::State<'_, AppState>) -> Result<Streaks, String> {
    let goal_seconds = super::prefs::get_daily_goal_inner(&state.db)?;
    let mask = cache::calendar::get_working_week_mask(&state.db).map_err(|e| e.to_string())?;
    let today = Local::now().date_naive();
    let from = today - Duration::days(LOOKBACK_DAYS);
    let rows = cache::calendar::list_non_working_days(&state.db, from, today)
        .map_err(|e| e.to_string())?;
    let non_working_set: std::collections::HashSet<String> =
        rows.into_iter().map(|r| r.date).collect();

    compute_streaks_inner(&state.db, goal_seconds, mask, &non_working_set)
        .map_err(|e| e.to_string())
}

/// Pure logic — used unit tests + tauri command. Mask je bit-mask:
/// bit 0 = pondělí, … bit 6 = neděle (`30` = po/út/st/čt/pá ⇒ working).
fn compute_streaks_inner(
    db: &Db,
    goal_seconds: i64,
    mask: i32,
    non_working: &std::collections::HashSet<String>,
) -> Result<Streaks, cache::DbError> {
    let today = Local::now().date_naive();
    let from = today - Duration::days(LOOKBACK_DAYS);

    // Posbírat součty per local day pomocí jednoho rangového dotazu, ať
    // se v cyklu níže nevoláme znovu a znovu.
    let from_ts = Local
        .from_local_datetime(&from.and_hms_opt(0, 0, 0).unwrap_or_default())
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(0);
    let to_ts = Local
        .from_local_datetime(&today.and_hms_opt(23, 59, 59).unwrap_or_default())
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(i64::MAX);

    let rows = cache::worklogs::for_date_range(db, from_ts, to_ts)?;
    let mut per_day: std::collections::HashMap<NaiveDate, i64> = std::collections::HashMap::new();
    for r in rows {
        let dt = Local
            .timestamp_opt(r.started_at, 0)
            .single()
            .map(|d| d.date_naive());
        if let Some(d) = dt {
            *per_day.entry(d).or_insert(0) += r.duration_s();
        }
    }

    // Iteruj přes dny v lookback okně, počítej delší / aktuální streak.
    let mut day = today;
    let mut longest = 0i64;
    let mut current = 0i64;
    let mut active_streak: Option<i64> = None;
    let today_met = per_day.get(&today).copied().unwrap_or(0) >= goal_seconds;

    // Going backwards from today: pokud první "pracovní" den (dnes nebo
    // včera, pokud dnes je víkend) splněn → streak začíná dnes, jinak včera.
    while day >= from {
        let day_iso = day.format("%Y-%m-%d").to_string();
        let is_working = is_working_day(day, mask, non_working, &day_iso);
        if !is_working {
            // Non-working day — neruší streak, ale nepřičítá.
            day -= Duration::days(1);
            continue;
        }
        let secs = per_day.get(&day).copied().unwrap_or(0);
        let met = secs >= goal_seconds;
        // Tichý "grace" — pokud dnes ještě nesplněno, nezačínáme streak
        // dnešním selháním. Zatím se nejdřív koukneme na včera.
        if active_streak.is_none() && day == today && !met {
            day -= Duration::days(1);
            continue;
        }
        if met {
            let run = active_streak.unwrap_or(0) + 1;
            active_streak = Some(run);
            if run > longest {
                longest = run;
            }
        } else {
            // Nedokončený pracovní den ⇒ konec aktuálního streaku.
            if current == 0 {
                current = active_streak.unwrap_or(0);
            }
            active_streak = Some(0);
        }
        day -= Duration::days(1);
    }
    // Pokud cyklus skončil bez nesplněného pracovního dne v okně, aktuální
    // streak = poslední běžící run (která narazila na okraj `from`).
    if current == 0 {
        current = active_streak.unwrap_or(0);
    }

    Ok(Streaks {
        current,
        longest,
        today_met,
    })
}

fn is_working_day(
    date: NaiveDate,
    mask: i32,
    non_working: &std::collections::HashSet<String>,
    iso: &str,
) -> bool {
    if non_working.contains(iso) {
        return false;
    }
    let dow_bit = match date.weekday() {
        Weekday::Mon => 1 << 0,
        Weekday::Tue => 1 << 1,
        Weekday::Wed => 1 << 2,
        Weekday::Thu => 1 << 3,
        Weekday::Fri => 1 << 4,
        Weekday::Sat => 1 << 5,
        Weekday::Sun => 1 << 6,
    };
    (mask & dow_bit) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_holidays() -> std::collections::HashSet<String> {
        std::collections::HashSet::new()
    }

    #[test]
    fn working_day_respects_mask() {
        // Mon-Fri = bits 0..4 = 0b00011111 = 31. Skutečné `working_week_mask`
        // v aplikaci default je 30 (po-pá bez ne) — tady jen ověřujeme dow bity.
        let mon = NaiveDate::from_ymd_opt(2026, 5, 11).unwrap(); // Monday
        let sat = NaiveDate::from_ymd_opt(2026, 5, 16).unwrap(); // Saturday
        let mask_mon_to_fri = 0b00011111; // 31
        let none = empty_holidays();
        assert!(is_working_day(mon, mask_mon_to_fri, &none, "2026-05-11"));
        assert!(!is_working_day(sat, mask_mon_to_fri, &none, "2026-05-16"));
    }

    #[test]
    fn holiday_overrides_working_mask() {
        let mon = NaiveDate::from_ymd_opt(2026, 5, 11).unwrap();
        let mut h = empty_holidays();
        h.insert("2026-05-11".into());
        assert!(!is_working_day(mon, 0xff, &h, "2026-05-11"));
    }
}
