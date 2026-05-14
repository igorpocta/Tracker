-- Phase 11A: extend recent_worklogs to be the "all worklogs" cache, holding
-- both locally-created entries and entries fetched from Jira (e.g. worklogs
-- the user added directly via the Jira web UI).

ALTER TABLE recent_worklogs ADD COLUMN author_account_id TEXT;
ALTER TABLE recent_worklogs ADD COLUMN source TEXT NOT NULL DEFAULT 'local'; -- 'local' | 'jira'
ALTER TABLE recent_worklogs ADD COLUMN updated_at INTEGER;

CREATE UNIQUE INDEX IF NOT EXISTS idx_worklogs_jira_id
    ON recent_worklogs(jira_worklog_id)
    WHERE jira_worklog_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_worklogs_started_at
    ON recent_worklogs(started_at DESC);

CREATE INDEX IF NOT EXISTS idx_worklogs_author
    ON recent_worklogs(author_account_id);
