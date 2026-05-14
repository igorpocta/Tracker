-- Phase 18B — Item 6: active-timer comment field.
--
-- Lets the user attach an in-progress note to a running timer that gets
-- persisted to Jira as the worklog comment when the timer stops (unless the
-- StopDialog overrides it). The column is nullable to preserve the meaning
-- "user hasn't provided a comment".

ALTER TABLE active_timer ADD COLUMN comment TEXT;
