-- Phase 18A — Unassigned timer support.
--
-- `recent_worklogs.pending_assignment = 1` marks worklogs that were stopped
-- with no issue selected (`issue_key = ''`). They're held locally until the
-- user assigns an issue via `assign_worklog_issue`, which then POSTs to Jira.

ALTER TABLE recent_worklogs ADD COLUMN pending_assignment INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_recent_pending_assignment
    ON recent_worklogs(pending_assignment) WHERE pending_assignment = 1;
