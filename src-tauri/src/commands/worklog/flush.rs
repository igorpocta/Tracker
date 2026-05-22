//! Shared backlog flush — push every "still local" worklog that has an
//! assigned issue but no remote id yet.
//!
//! Used by:
//!   * startup recovery (lib.rs)
//!   * manual refresh_all (commands::worklog::sync)
//!   * periodic auto-sync (lib.rs)
//!
//! Bounded by `MAX_FLUSH_PER_RUN` so a long-offline session can't kick off
//! hundreds of parallel HTTP requests. Per-row errors are logged but never
//! abort the flush — one stuck row must not block the rest.

use tauri::{AppHandle, Runtime};

use crate::cache::{self, worklogs::WorklogRow};
use crate::state::AppState;

/// Cap per call so a backlog of unsynced rows doesn't fan-out to hundreds
/// of parallel POSTs. Picked to match the historical startup-flush bound.
pub const MAX_FLUSH_PER_RUN: u32 = 50;

/// Push every unsynced-with-issue worklog (up to [`MAX_FLUSH_PER_RUN`])
/// through the upstream POST helper. Returns the number of rows that
/// successfully synced.
///
/// Filtering responsibility lives in [`cache::worklogs::unsynced_with_issue`],
/// which already excludes tombstoned and pending-delete rows.
///
/// Idempotency: each row is keyed on its local_id; once the helper sets
/// `remote_id`, a follow-up flush call sees `is_synced = true` and skips
/// the row entirely. So even if two flush invocations race, neither
/// duplicates the remote worklog.
pub async fn flush_unsynced_worklogs<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> usize {
    let candidates: Vec<WorklogRow> =
        match cache::worklogs::unsynced_with_issue(&state.db, MAX_FLUSH_PER_RUN) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("flush: cache scan failed: {e}");
                return 0;
            }
        };
    if candidates.is_empty() {
        return 0;
    }
    tracing::info!("flush: pushing {} unsynced worklog(s)", candidates.len());
    let mut ok = 0usize;
    for row in candidates {
        let Some(local_id) = row.id else { continue };
        match super::crud::push_local_worklog_inner(app, state, local_id).await {
            Ok(_) => ok += 1,
            Err(e) => {
                tracing::warn!("flush: push for local_id={local_id} failed: {e}");
            }
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{worklogs::WorklogRow, Db};

    fn temp_db() -> Db {
        let tmp = tempfile::tempdir().unwrap();
        Db::open(&tmp.path().join("t.db")).unwrap()
    }

    #[test]
    fn unsynced_with_issue_excludes_tombstoned_and_pending_delete() {
        let db = temp_db();
        // Need a real connection row — worklogs.connection_id has FK on
        // connections(id) ON DELETE SET NULL.
        let conn_id = cache::connections::insert(
            &db,
            cache::connections::NewConnection {
                provider: "jira",
                name: "Test",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        // Row A: ready to flush.
        let a = WorklogRow {
            id: None,
            connection_id: Some(conn_id),
            issue_key: Some("DEV-1".into()),
            description: None,
            started_at: 0,
            ended_at: 60,
            logged_at: 0,
            updated_at: 0,
            is_synced: false,
            synced_at: None,
            remote_id: None,
            pending_delete_at: None,
            tombstoned_at: None,
            summary: None,
        };
        let id_a = cache::worklogs::record(&db, &a).unwrap();
        // Row B: pending-delete — must NOT come back.
        let mut b = a.clone();
        b.issue_key = Some("DEV-2".into());
        let id_b = cache::worklogs::record(&db, &b).unwrap();
        cache::worklogs::mark_pending_delete(&db, id_b, 100).unwrap();
        // Row C: tombstoned — must NOT come back.
        let mut c = a.clone();
        c.issue_key = Some("DEV-3".into());
        let id_c = cache::worklogs::record(&db, &c).unwrap();
        cache::worklogs::mark_tombstoned(&db, id_c, 100).unwrap();

        let rows = cache::worklogs::unsynced_with_issue(&db, 50).unwrap();
        let ids: Vec<i64> = rows.iter().filter_map(|r| r.id).collect();
        assert_eq!(ids, vec![id_a], "only the clean row must be flushable");
    }
}
