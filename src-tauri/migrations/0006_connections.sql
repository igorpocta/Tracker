-- Phase 18A — Multi-provider / multi-connection model.
--
-- Replaces the single-provider config with a list of named "connections", each
-- carrying its own provider kind, label, enabled flag, and provider-specific
-- JSON config blob. The Jira API token for connection `id` is stored in the
-- secret file under key `connection:<id>:token`.

CREATE TABLE IF NOT EXISTS connections (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    provider        TEXT NOT NULL,
    name            TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL,
    config_json     TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_connections_name ON connections(name);

-- Tag existing cache with connection ownership. `NULL` rows belong to the
-- legacy single-connection install; the startup migration backfills them once
-- the first connection is hydrated.
ALTER TABLE issues ADD COLUMN connection_id INTEGER REFERENCES connections(id);
ALTER TABLE recent_worklogs ADD COLUMN connection_id INTEGER REFERENCES connections(id);
CREATE INDEX IF NOT EXISTS idx_issues_connection ON issues(connection_id);
CREATE INDEX IF NOT EXISTS idx_worklogs_connection ON recent_worklogs(connection_id);
