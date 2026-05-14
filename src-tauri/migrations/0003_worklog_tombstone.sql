-- Phase 15: soft-delete safety net + mark-and-sweep for deleted-in-Jira worklogs.
--
-- `pending_delete_at` is set by the user clicking the trash icon. A background
-- task scheduled by the `delete_worklog` command waits ~5s and, if the column
-- is still non-null, fires the actual Jira DELETE + sets `tombstoned_at`. The
-- frontend optimistically hides the row in the meantime. Clicking "Vrátit"
-- (undo) clears `pending_delete_at`.
--
-- `tombstoned_at` marks rows that have been deleted in Jira (either by us via
-- the soft-delete flow, or by the user directly in the Jira web UI — detected
-- via mark-and-sweep during sync). We keep the row for ~30 days as a forensic
-- audit trail; `purge_old_tombstoned` runs on each sync to hard-delete older
-- rows.

ALTER TABLE recent_worklogs ADD COLUMN pending_delete_at INTEGER;
ALTER TABLE recent_worklogs ADD COLUMN tombstoned_at INTEGER;

CREATE INDEX IF NOT EXISTS idx_worklogs_tombstoned
    ON recent_worklogs(tombstoned_at)
    WHERE tombstoned_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_worklogs_pending_delete
    ON recent_worklogs(pending_delete_at)
    WHERE pending_delete_at IS NOT NULL;
