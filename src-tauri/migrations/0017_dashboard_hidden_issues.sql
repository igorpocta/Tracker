-- 0017 — per-issue "hide from Jira dashboard" list.
--
-- The dashboard aggregates live Jira results; hiding an issue is a local
-- preference, not a Jira mutation. Identity is (connection_id, issue_key) so
-- the same key in two tenants is hidden independently. ON DELETE CASCADE: drop
-- a connection → its hidden entries go too.

CREATE TABLE IF NOT EXISTS dashboard_hidden_issues (
    connection_id   INTEGER NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    issue_key       TEXT NOT NULL,
    hidden_at       INTEGER NOT NULL,
    PRIMARY KEY (connection_id, issue_key)
);
