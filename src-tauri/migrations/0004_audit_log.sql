-- Phase 15: append-only audit log for every worklog mutation.
--
-- Every Tauri mutation command writes a row before AND/OR after the mutation
-- so we can forensically reconstruct what happened to the user's data. The
-- before_json / after_json snapshots are full WorklogRow serializations.
--
-- Retention is unbounded for now; we may revisit if the table grows large
-- but the volume is bounded by user clicks so it'll stay tiny for years.

CREATE TABLE IF NOT EXISTS audit_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at  INTEGER NOT NULL,           -- Unix seconds
    op           TEXT NOT NULL,              -- 'create' | 'update' | 'delete' | 'move' | 'sync_tombstone' | 'undo'
    issue_key    TEXT,
    worklog_id   TEXT,                       -- Jira worklog id (string) when known
    before_json  TEXT,                       -- pre-op snapshot of the WorklogRow (NULL on create)
    after_json   TEXT,                       -- post-op snapshot of the WorklogRow (NULL on delete)
    success      INTEGER NOT NULL,           -- 0 or 1
    error        TEXT                        -- non-null when success = 0
);

CREATE INDEX IF NOT EXISTS idx_audit_occurred ON audit_log(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_worklog ON audit_log(worklog_id);
