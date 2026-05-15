-- Phase 19 — sync_runs audit table.
--
-- Drží jeden řádek per dokončený sync (jedna connection, jeden mode).
-- Otevírá UI „Historie synchronizací" v Administraci a slouží jako trust
-- signal: kdy poslední sync proběhl, jak dlouho trval, kolik toho upsertl,
-- případně co padlo. Nezasahuje do hot-path syncu — `sync_one_connection`
-- volá `cache::sync_log::record` jednou na konci.

CREATE TABLE sync_runs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id   INTEGER REFERENCES connections(id) ON DELETE SET NULL,
    connection_name TEXT,
    provider        TEXT,
    mode            TEXT NOT NULL,            -- 'full' | 'incremental' | …
    started_at      INTEGER NOT NULL,         -- unix sec UTC
    finished_at     INTEGER NOT NULL,
    issues_count    INTEGER NOT NULL DEFAULT 0,
    worklogs_count  INTEGER NOT NULL DEFAULT 0,
    -- Které fáze padly: NULL = bez chyby. Pokud je nastaveno, drží stručný
    -- text z poslední `last_sync_error` zápisu.
    error_phase     TEXT,
    error_message   TEXT
);

CREATE INDEX idx_sync_runs_finished   ON sync_runs(finished_at DESC);
CREATE INDEX idx_sync_runs_connection ON sync_runs(connection_id);
