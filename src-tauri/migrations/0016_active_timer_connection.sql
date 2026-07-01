-- 0016 — aktivní timer si pamatuje connection_id vybraného tenanta.
--
-- Dřív se connection dohledávala z issue_key až při stopu / auto-transition
-- (get_connection_id_by_key). U dvou *enabled* tenantů se stejným klíčem to
-- mohlo uložit worklog a přejít issue ve špatném tenantovi. Nově start uloží
-- explicitní connection_id (zná-li ho z favorita / výběru) a stop ho použije
-- přednostně; při NULL se zachová původní dohledání z klíče (zpětná kompat.).
--
-- Sloupec je nullable a bez defaultu jiného než NULL → ALTER ADD COLUMN je
-- v SQLite povolen i s REFERENCES. active_timer drží max. jeden ephemerální
-- řádek (id = 1).
ALTER TABLE active_timer
    ADD COLUMN connection_id INTEGER REFERENCES connections(id) ON DELETE SET NULL;
