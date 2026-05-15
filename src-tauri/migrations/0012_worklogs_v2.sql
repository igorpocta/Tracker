-- Phase 19 — Schéma worklogs + issues_v2.
--
-- Účel: oprostit datový model od JIRA-centrického dědictví staré tabulky
-- `recent_worklogs` a vyčistit přepláctanou `issues`. Po této migraci:
--
--   * `worklogs`     drží VŠECHNY časové záznamy (lokální i syncované) napříč
--                    providery; `is_synced` + `remote_id` říkají, jestli a kde
--                    je záznam v cloudu.
--   * `issues_v2`    drží minimální, multi-provider seznam úkolů, na který
--                    se worklogy logicky vážou přes `issue_key`.
--
-- Stará data se nemažou — staré tabulky pouze přejmenovávám na `_legacy`,
-- abychom v případě regrese měli kam sáhnout. Tombstonované worklogy se
-- nově drží navždy (požadavek uživatele): retence v Rust kódu se ruší.
--
-- VAROVÁNÍ: tato migrace NEDĚLÁ ŽÁDNÝ DELETE proti providerům. Lokální data
-- v `_legacy` zůstávají, nové tabulky startují prázdné; další sync je
-- naplní z Jira / Freelo bez ztráty.

-- 1) Přejmenování staré tabulky worklogů a jejích indexů.
ALTER TABLE recent_worklogs RENAME TO recent_worklogs_legacy;

-- SQLite ALTER TABLE RENAME automaticky překreslí FK reference; indexy ale
-- zůstávají vázané na jméno tabulky. Drobíme je, abychom uvolnili jméno
-- pro nové indexy a předešli kolizím.
DROP INDEX IF EXISTS idx_recent_logged_at;
DROP INDEX IF EXISTS idx_worklogs_jira_id;
DROP INDEX IF EXISTS idx_worklogs_started_at;
DROP INDEX IF EXISTS idx_worklogs_author;
DROP INDEX IF EXISTS idx_worklogs_tombstoned;
DROP INDEX IF EXISTS idx_worklogs_pending_delete;
DROP INDEX IF EXISTS idx_worklogs_connection;
DROP INDEX IF EXISTS idx_recent_pending_assignment;

-- 2) Přejmenování staré tabulky issues a jejích indexů.
ALTER TABLE issues RENAME TO issues_legacy;
DROP INDEX IF EXISTS idx_issues_updated_at;
DROP INDEX IF EXISTS idx_issues_connection;

-- 3) Nová tabulka worklogs.
--
-- Sloupce vědomě VYNECHANÉ oproti staré recent_worklogs:
--   - duration_s          → dopočítá se ended_at - started_at
--   - summary             → patří do issues_v2, joinujeme přes issue_key
--   - source ('local'/'jira') → derivováno z is_synced + remote_id
--   - author_account_id   → aplikace je jednouživatelská
--   - pending_assignment  → derivováno z issue_key IS NULL
CREATE TABLE worklogs (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Vazba na integraci a úkol. Obojí může být NULL u čistě lokálního,
    -- ještě nepřiřazeného záznamu (uživatel stopnul timer, ale nevybral úkol).
    connection_id       INTEGER REFERENCES connections(id) ON DELETE SET NULL,
    issue_key           TEXT,

    -- Obsah záznamu.
    description         TEXT,
    started_at          INTEGER NOT NULL,
    ended_at            INTEGER NOT NULL,

    -- Lokální audit/timestampy.
    logged_at           INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,

    -- Sync s providerem.
    is_synced           INTEGER NOT NULL DEFAULT 0,
    synced_at           INTEGER,
    remote_id           TEXT,

    -- Životní cyklus.
    pending_delete_at   INTEGER,
    tombstoned_at       INTEGER,

    CHECK (ended_at >= started_at),
    CHECK (is_synced IN (0, 1))
);

-- Unikátnost upstream id v rámci jedné integrace — předchází duplicitám
-- při paralelních syncech a opětovných importech.
CREATE UNIQUE INDEX idx_worklogs_remote
    ON worklogs(connection_id, remote_id)
    WHERE remote_id IS NOT NULL;

-- Indexy odpovídající nejčastějším dotazům (Time Log range, Reporty).
CREATE INDEX idx_worklogs_started     ON worklogs(started_at DESC);
CREATE INDEX idx_worklogs_connection  ON worklogs(connection_id);
CREATE INDEX idx_worklogs_issue       ON worklogs(issue_key);
CREATE INDEX idx_worklogs_unsynced    ON worklogs(is_synced) WHERE is_synced = 0;
CREATE INDEX idx_worklogs_pending_del ON worklogs(pending_delete_at) WHERE pending_delete_at IS NOT NULL;
CREATE INDEX idx_worklogs_tombstoned  ON worklogs(tombstoned_at) WHERE tombstoned_at IS NOT NULL;

-- 4) Nová tabulka issues_v2.
--
-- Minimum potřebné pro vyhledávač a vazbu z worklogs. Pole jako time_spent,
-- assignee_*, epic_*, priority_order, issue_type apod. ze staré `issues`
-- jsou Jira-specifická a v aplikaci se reálně nepoužívají — neportuju je.
CREATE TABLE issues_v2 (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    connection_id       INTEGER NOT NULL REFERENCES connections(id) ON DELETE CASCADE,

    issue_id            TEXT NOT NULL,
    issue_key           TEXT NOT NULL,
    name                TEXT NOT NULL,

    -- Hierarchie (jen pokud ji provider má — Jira parent/epic, Freelo subtask).
    parent_key          TEXT,
    parent_name         TEXT,

    -- Stav z provideru + flag, jestli úkol skrývat ve vyhledávači.
    status              TEXT,
    is_archived         INTEGER NOT NULL DEFAULT 0,

    created_at          INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL,
    -- 'updated' u Jiry, 'date_edited_at' u Freela — používá se v UI pro
    -- "najdi úkol upravený nedávno" a pro inkrementální sync.
    remote_updated_at   INTEGER,
    last_synced_at      INTEGER,

    CHECK (is_archived IN (0, 1))
);

CREATE UNIQUE INDEX idx_issues2_conn_key ON issues_v2(connection_id, issue_key);
CREATE UNIQUE INDEX idx_issues2_conn_id  ON issues_v2(connection_id, issue_id);
CREATE INDEX idx_issues2_remote_updated  ON issues_v2(remote_updated_at DESC);
CREATE INDEX idx_issues2_archived        ON issues_v2(is_archived);
CREATE INDEX idx_issues2_name            ON issues_v2(name);
