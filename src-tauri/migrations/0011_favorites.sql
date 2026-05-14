-- Phase 18B — Item 26: favorite (starred) issues.
--
-- Favorites surface at the top of Recent / search dropdowns and let the user
-- start a timer on a frequently-used issue without typing. The list is local
-- (no Jira sync); deleting the underlying issue from Jira does not
-- automatically remove the favorite row — the UI just shows it greyed out.

CREATE TABLE IF NOT EXISTS favorite_issues (
    issue_key       TEXT PRIMARY KEY,
    connection_id   INTEGER REFERENCES connections(id) ON DELETE SET NULL,
    added_at        INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_favorites_added_at
    ON favorite_issues(added_at DESC);
