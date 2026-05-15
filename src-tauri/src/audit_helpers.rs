//! Shared helpers used by every provider's `reconstruct` module
//! (`jira/reconstruct.rs`, `freelo/reconstruct.rs`, …).
//!
//! The three primitives moved here were byte-identical between the
//! Jira and Freelo implementations:
//!
//! * [`now_unix`] — current wall-clock as Unix seconds.
//! * [`parse_row`] — deserialise a `WorklogRow` JSON snapshot (provider-
//!   parameterised over the error type).
//! * [`record_linked`] — append a linked audit-log entry referencing the
//!   `source_audit_id`, swallowing any DB error (we never want a failed
//!   audit write to mask the real outcome of the reconstruction call).

use chrono::Utc;

use crate::cache::{
    audit::{record as audit_record, AuditEvent, AuditOp},
    worklogs::WorklogRow,
    Db,
};

/// Wall-clock now as Unix seconds (UTC). Wrapping `Utc::now().timestamp()`
/// pulls the time source into a single spot for future test injection
/// (today nothing stubs it — wiremock tests don't care about the audit
/// timestamp).
pub fn now_unix() -> i64 {
    Utc::now().timestamp()
}

/// Deserialise a `WorklogRow` snapshot (the `before_json` / `after_json`
/// blobs stored in `audit_log`). Generic over the error type so each
/// provider's `ReconstructError` (which already implements
/// `From<serde_json::Error>` via thiserror's `#[from]`) keeps working.
///
/// **Why the `Value` middle step.** `WorklogRow`'s `Serialize` impl
/// emits *both* the canonical key (`description`, `remote_id`) and the
/// legacy frontend alias (`comment`, `jira_worklog_id`) for the same
/// underlying field, plus derived keys (`duration_s`, `source`,
/// `pending_assignment`). When the audit log captures this JSON and
/// `parse_row` later deserialises it via the `#[serde(alias = "comment")]`
/// rule on the `description` field, serde sees two writes to one field
/// and panics with `duplicate field "description"`. We therefore parse
/// into a `Value` first, strip the duplicated aliases when the canonical
/// key is also present, drop the derived-only keys, and only then
/// deserialise into `WorklogRow`. Round-trip stays lossless because the
/// dropped keys are derivable from the kept ones.
pub fn parse_row<E>(json: &str) -> Result<WorklogRow, E>
where
    E: From<serde_json::Error>,
{
    let mut v: serde_json::Value = serde_json::from_str(json)?;
    if let Some(obj) = v.as_object_mut() {
        if obj.contains_key("description") {
            obj.remove("comment");
        }
        if obj.contains_key("remote_id") {
            obj.remove("jira_worklog_id");
        }
        // Derived-only keys never feed back into a `WorklogRow` field.
        obj.remove("source");
        obj.remove("pending_assignment");
        obj.remove("duration_s");
    }
    serde_json::from_value::<WorklogRow>(v).map_err(Into::into)
}

/// Append a follow-up audit entry that links back to the audit row whose
/// reconstruction we just attempted (success or failure). DB errors are
/// intentionally swallowed: failing to write a *meta* audit row should
/// never propagate as the user-visible failure of the reconstruction
/// itself.
#[allow(clippy::too_many_arguments)]
pub fn record_linked(
    db: &Db,
    op: AuditOp,
    issue_key: Option<&str>,
    worklog_id: Option<&str>,
    before: Option<&WorklogRow>,
    after: Option<&WorklogRow>,
    success: bool,
    error: Option<&str>,
    source_audit_id: i64,
) {
    let _ = audit_record(
        db,
        AuditEvent {
            occurred_at: now_unix(),
            op,
            issue_key,
            worklog_id,
            before,
            after,
            success,
            error,
            source_audit_id: Some(source_audit_id),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum TestErr {
        #[error("serde: {0}")]
        Serde(#[from] serde_json::Error),
    }

    #[test]
    fn parse_row_round_trips_minimal_snapshot() {
        // A snapshot with only the canonical (non-alias) keys — matches
        // what every Phase 0 / Phase A reconstruction test writes via the
        // `snapshot_json` helper.
        let json = r#"{
            "id": 42,
            "connection_id": 1,
            "issue_key": "ACME-1",
            "description": "fixing the bug",
            "started_at": 100,
            "ended_at": 460,
            "logged_at": 460,
            "updated_at": 460,
            "is_synced": true,
            "synced_at": 460,
            "remote_id": "j-42",
            "pending_delete_at": null,
            "tombstoned_at": null
        }"#;
        let row: WorklogRow = parse_row::<TestErr>(json).expect("ok");
        assert_eq!(row.id, Some(42));
        assert_eq!(row.connection_id, Some(1));
        assert_eq!(row.issue_key.as_deref(), Some("ACME-1"));
        assert_eq!(row.description.as_deref(), Some("fixing the bug"));
        assert_eq!(row.remote_id.as_deref(), Some("j-42"));
        assert!(row.is_synced);
        assert_eq!(row.duration_s(), 360);
    }

    #[test]
    fn parse_row_propagates_serde_error_as_provider_error() {
        let res: Result<WorklogRow, TestErr> = parse_row("{ not json");
        assert!(matches!(res, Err(TestErr::Serde(_))));
    }

    #[test]
    fn parse_row_round_trips_through_the_canonical_serialize_output() {
        // Regression: `WorklogRow::Serialize` emits both the canonical key
        // (`description`) and the legacy alias (`comment`) for the same
        // underlying field. Round-tripping through `serde_json::to_string`
        // and then back used to fail with a "duplicate field" error
        // because of `#[serde(alias = "comment")]` on `description`. The
        // workaround in Phase 0 was a hand-rolled `snapshot_json` that
        // wrote only the canonical keys; that's fragile because any new
        // caller that serialises with `serde_json::to_string` and stores
        // the result in `audit_log` would re-introduce the bug.
        let row = WorklogRow {
            id: Some(7),
            connection_id: Some(1),
            issue_key: Some("ACME-1".into()),
            description: Some("a comment".into()),
            started_at: 100,
            ended_at: 220,
            logged_at: 220,
            updated_at: 220,
            is_synced: true,
            synced_at: Some(220),
            remote_id: Some("j-7".into()),
            pending_delete_at: None,
            tombstoned_at: None,
            summary: Some("the summary".into()),
        };
        let json = serde_json::to_string(&row).expect("serialise");
        // Sanity: the serialised blob really does contain BOTH the
        // canonical key and the legacy alias — that's what makes the
        // duplicate-field check meaningful.
        assert!(json.contains("\"description\""));
        assert!(json.contains("\"comment\""));
        let parsed: WorklogRow = parse_row::<TestErr>(&json).expect("ok");
        assert_eq!(parsed.id, Some(7));
        assert_eq!(parsed.description.as_deref(), Some("a comment"));
        assert_eq!(parsed.remote_id.as_deref(), Some("j-7"));
        assert!(parsed.is_synced);
    }

    #[test]
    fn now_unix_is_recent_and_monotonic() {
        // Two reads should be close to each other; the second should be
        // ≥ the first. We don't pin an absolute value because that would
        // make this test calendar-fragile.
        let a = now_unix();
        let b = now_unix();
        assert!(b >= a);
        // Sanity: the result is large enough to be "post-2020 Unix
        // timestamp", catching obvious wrong-unit bugs.
        assert!(a > 1_577_000_000);
    }
}
