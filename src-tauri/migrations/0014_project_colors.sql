-- Phase 19 — per-project color overrides.
--
-- Volitelný hex (např. `#3B82F6`) per projekt klíč. „Project key" zde znamená
-- normalizovaný prefix issue klíče (`DEV-792` → `DEV`, `FREELO-12345` →
-- `FREELO-P-…` pokud máme parent project, jinak prefix `FREELO`). UI vrstva
-- mapuje issue na project key přes existující IssueRow.parent_key.
--
-- Když záznam pro klíč neexistuje, padá se na default per-provider barvu
-- (Jira accent, Freelo orange) — color coding je čistě opt-in.

CREATE TABLE project_colors (
    project_key TEXT PRIMARY KEY,
    color       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL
);
