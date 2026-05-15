//! Smart suggestion engine — „jako včera".
//!
//! Heuristika: pokud uživatel v posledních N pracovních dnech opakovaně
//! trackoval **stejný úkol v blízký čas**, navrhneme ho jako kandidáta na
//! příští start. Tichá pravidla:
//!
//!  - Bere v úvahu jen worklogy z posledních 14 kalendářních dnů.
//!  - Sleduje hodinová okénka (`buckets` 1h široká).
//!  - Suggesce vznikne, když:
//!    * stejný `issue_key` má ≥ 2 výskyty v rámci hodinového okénka,
//!    * okénko obsahuje aktuální lokální čas (± 60 min),
//!    * uživatel nemá běžící timer.
//!
//! Vrací max 3 návrhy seřazené dle počtu výskytů. Bez paniky pokud nic
//! nesedí — UI návrh prostě nezobrazí.

use chrono::{Local, NaiveDate, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

use crate::cache::{self, Db};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub issue_key: String,
    pub summary: Option<String>,
    /// Kolikrát jsme v okénku stejný úkol viděli (v posledních 14 dnech).
    pub occurrences: i64,
    /// Hodina dne, do které okénko spadá (např. 9 znamená 9:00–9:59).
    pub bucket_hour: i64,
}

const LOOKBACK_DAYS: i64 = 14;
const MIN_OCCURRENCES: i64 = 2;
const MAX_SUGGESTIONS: usize = 3;
const HOUR_WINDOW_MIN: i64 = 60;

#[tauri::command]
pub async fn get_suggestions(state: tauri::State<'_, AppState>) -> Result<Vec<Suggestion>, String> {
    let active = cache::timer::get(&state.db).map_err(|e| e.to_string())?;
    if active.is_some() {
        // Když timer běží, návrh by jen mátl. Pošleme prázdný seznam.
        return Ok(Vec::new());
    }
    let now_local = Local::now();
    compute_suggestions(
        &state.db,
        now_local.naive_local().date(),
        now_local.hour() as i64,
    )
    .map_err(|e| e.to_string())
}

fn compute_suggestions(
    db: &Db,
    today: NaiveDate,
    current_hour: i64,
) -> Result<Vec<Suggestion>, cache::DbError> {
    let from = today - chrono::Duration::days(LOOKBACK_DAYS);
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

    // Aggregate: (hour_bucket, issue_key) → count.
    let mut counts: std::collections::HashMap<(i64, String), i64> =
        std::collections::HashMap::new();
    let mut summaries: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for r in rows {
        let issue_key = match r.issue_key.clone() {
            Some(k) if !k.is_empty() => k,
            _ => continue,
        };
        // Vyloučíme dnešní worklogy z téhož hodinového okénka — jinak by
        // suggesce ukázala úkol, který právě teď trackuje user (after the fact).
        let dt = Local.timestamp_opt(r.started_at, 0).single();
        let Some(dt) = dt else { continue };
        if dt.date_naive() == today && (dt.hour() as i64) == current_hour {
            continue;
        }
        let hour = dt.hour() as i64;
        // Pouze okénka v ± HOUR_WINDOW_MIN od aktuálního času.
        if !within_window(hour, current_hour) {
            continue;
        }
        let key = (hour, issue_key.clone());
        *counts.entry(key).or_insert(0) += 1;
        summaries
            .entry(issue_key)
            .or_insert_with(|| r.summary.clone());
    }

    let mut out: Vec<Suggestion> = counts
        .into_iter()
        .filter(|(_, n)| *n >= MIN_OCCURRENCES)
        .map(|((hour, key), n)| Suggestion {
            issue_key: key.clone(),
            summary: summaries.get(&key).cloned().flatten(),
            occurrences: n,
            bucket_hour: hour,
        })
        .collect();
    out.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
    out.truncate(MAX_SUGGESTIONS);
    Ok(out)
}

fn within_window(hour: i64, current: i64) -> bool {
    let diff_min = ((hour - current).rem_euclid(24)).min((current - hour).rem_euclid(24)) * 60;
    diff_min <= HOUR_WINDOW_MIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_window_inclusive_at_one_hour_diff() {
        // Při HOUR_WINDOW_MIN = 60 je diff ± 1 hodina ještě OK.
        assert!(within_window(9, 9));
        assert!(within_window(8, 9));
        assert!(within_window(10, 9));
    }

    #[test]
    fn within_window_excludes_two_hour_diff() {
        assert!(!within_window(7, 9));
        assert!(!within_window(11, 9));
    }

    #[test]
    fn within_window_wraps_around_midnight() {
        // 23 vs 0 = 1 hodina, mělo by být OK.
        assert!(within_window(23, 0));
        assert!(within_window(0, 23));
    }
}
