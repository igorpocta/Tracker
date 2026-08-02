-- Focus mode: rules for blocking (or exclusively allowing) apps and websites
-- while a focus session is running.
--
-- `pattern` semantics depend on `kind`:
--   'app'  → bundle identifier (macOS), executable name (Windows), or the
--            app's display name. Matched case-insensitively against all three
--            so one rule works on both platforms.
--   'site' → "domain", "domain/path-prefix" or "*.domain". Subdomains match
--            implicitly.
--
-- `action` only applies to 'app' rules. 'hide' pushes the app out of the way,
-- 'kill' terminates it. Strict (allow-list) mode always uses 'hide' regardless
-- of what is stored here — see `focus::rules`.
CREATE TABLE IF NOT EXISTS focus_rules (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT    NOT NULL CHECK (kind IN ('app', 'site')),
    mode       TEXT    NOT NULL CHECK (mode IN ('block', 'allow')),
    pattern    TEXT    NOT NULL,
    label      TEXT,
    action     TEXT    NOT NULL DEFAULT 'hide' CHECK (action IN ('hide', 'kill')),
    enabled    INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

-- One rule per (kind, mode, pattern) — re-adding an existing entry updates it
-- instead of piling up duplicates.
CREATE UNIQUE INDEX IF NOT EXISTS idx_focus_rules_unique
    ON focus_rules (kind, mode, pattern);

CREATE INDEX IF NOT EXISTS idx_focus_rules_lookup
    ON focus_rules (kind, enabled);
