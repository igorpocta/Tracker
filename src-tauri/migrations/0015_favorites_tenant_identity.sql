-- 0015 — favorites keyed by (connection_id, issue_key), not issue_key alone.
--
-- Před: PK = issue_key → stejný klíč u dvou tenantů se slil do jednoho favoritu
-- (ON CONFLICT(issue_key) přepsal connection_id) a quick-start / dropdown mohl
-- ukázat nebo spustit špatný issue. Nově je identita (connection_id, issue_key).
--
-- Backfill existujících řádků:
--   * má-li řádek connection_id → ponecháme,
--   * jinak, patří-li klíč jednoznačně jednomu tenantovi v issues_v2 →
--     doplníme jeho connection_id,
--   * víceznačné (klíč u víc tenantů) necháme NULL — UI je nezobrazí jako
--     aktivní/spustitelné, ať netrefí špatný tenant.
--
-- favorite_issues nemá žádné příchozí FK, takže rename→recreate→drop je bezpečné
-- i se zapnutým foreign_keys.

ALTER TABLE favorite_issues RENAME TO favorite_issues_old;

CREATE TABLE favorite_issues (
    connection_id   INTEGER REFERENCES connections(id) ON DELETE CASCADE,
    issue_key       TEXT NOT NULL,
    added_at        INTEGER NOT NULL
);

INSERT INTO favorite_issues (connection_id, issue_key, added_at)
SELECT
    CASE
        WHEN f.connection_id IS NOT NULL THEN f.connection_id
        WHEN (SELECT COUNT(*) FROM issues_v2 i WHERE i.issue_key = f.issue_key) = 1
            THEN (SELECT i.connection_id FROM issues_v2 i WHERE i.issue_key = f.issue_key)
        ELSE NULL
    END,
    f.issue_key,
    f.added_at
FROM favorite_issues_old f;

DROP TABLE favorite_issues_old;

-- ON CONFLICT(connection_id, issue_key) cíl pro upsert v add().
CREATE UNIQUE INDEX idx_favorites_conn_key ON favorite_issues(connection_id, issue_key);
CREATE INDEX idx_favorites_added_at ON favorite_issues(added_at DESC);
