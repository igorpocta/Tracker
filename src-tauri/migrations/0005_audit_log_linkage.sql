-- Phase 16: linkage column for audit_log + new op kinds.
--
-- `source_audit_id` lets a "restore" / "revert" / "retry" entry point back at
-- the original audit row it was triggered from. The UI uses this to detect
-- "this delete has already been restored" so we don't offer the action twice.
--
-- We don't backfill — older rows simply have NULL, which is the correct
-- "no linkage" sentinel anyway.

ALTER TABLE audit_log ADD COLUMN source_audit_id INTEGER REFERENCES audit_log(id);

CREATE INDEX IF NOT EXISTS idx_audit_source
    ON audit_log(source_audit_id)
    WHERE source_audit_id IS NOT NULL;
