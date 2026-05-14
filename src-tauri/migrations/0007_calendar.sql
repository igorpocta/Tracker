-- Phase 18A — Non-working days + custom working week.
--
-- Per-date overrides for "I don't work on this day" used by goal predictions,
-- reporting shading, and the daily goal helper. The default working week is
-- stored in `app_settings` under the key `working_week_mask` (bitmask of
-- weekdays, Mon=1 .. Sun=64; default 31 = Mon–Fri).

CREATE TABLE IF NOT EXISTS non_working_days (
    date            TEXT PRIMARY KEY,
    reason          TEXT NOT NULL,
    label           TEXT,
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_non_working_days_date ON non_working_days(date);
