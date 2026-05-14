-- Phase 18A — Daily activity / inactivity ratios (no billing impact).
--
-- The frontend posts user-activity events (mouse, keyboard) to the backend;
-- the backend aggregates them per local date into `active_seconds` /
-- `inactive_seconds`. The Goals view surfaces the ratio.

CREATE TABLE IF NOT EXISTS daily_activity (
    date              TEXT PRIMARY KEY,
    active_seconds    INTEGER NOT NULL DEFAULT 0,
    inactive_seconds  INTEGER NOT NULL DEFAULT 0,
    updated_at        INTEGER NOT NULL
);
