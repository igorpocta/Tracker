CREATE TABLE IF NOT EXISTS issues (
    issue_key               TEXT PRIMARY KEY,
    issue_id                TEXT,
    summary                 TEXT NOT NULL,
    status_category         TEXT,
    priority_order          INTEGER,
    assignee_email          TEXT,
    assignee_account_id     TEXT,
    parent_key              TEXT,
    parent_summary          TEXT,
    issue_type              TEXT,
    time_spent              INTEGER,
    aggregate_time_spent    INTEGER,
    time_original_estimate  INTEGER,
    time_estimate           INTEGER,
    epic_key                TEXT,
    epic_summary            TEXT,
    updated_at              INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_issues_updated_at ON issues(updated_at DESC);

CREATE TABLE IF NOT EXISTS active_timer (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    issue_key   TEXT NOT NULL,
    started_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS recent_worklogs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_key   TEXT NOT NULL,
    issue_id    TEXT,
    summary     TEXT,
    duration_s  INTEGER NOT NULL,
    started_at  INTEGER NOT NULL,
    logged_at   INTEGER NOT NULL,
    comment     TEXT,
    jira_worklog_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_recent_logged_at ON recent_worklogs(logged_at DESC);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
