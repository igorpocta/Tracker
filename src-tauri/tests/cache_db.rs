use tempfile::TempDir;
use tracker_lib::cache::audit::{
    get_by_id as audit_get_by_id, list as audit_list, purge_older_than as audit_purge,
    recent as audit_recent, record as audit_record, AuditEvent, AuditOp,
};
use tracker_lib::cache::connections::{insert as insert_conn, NewConnection};
use tracker_lib::cache::issues::{get_by_key, search, upsert, IssueRow};
use tracker_lib::cache::settings::{get as setting_get, set as setting_set};
use tracker_lib::cache::timer::{get as timer_get, start as timer_start, stop as timer_stop};
use tracker_lib::cache::worklogs::{
    clear_pending_delete, for_date_range as worklog_for_date_range, get_by_id,
    get_by_remote_id_any, mark_pending_delete, mark_tombstoned, mark_tombstoned_by_remote_id,
    pending_deletes_older_than, recent as worklog_recent, record as worklog_record,
    total_seconds_for_range, update_fields, upsert_from_remote, WorklogRow,
};
use tracker_lib::cache::Db;

/// `issues_v2` carries an FK to `connections`. Seed one Jira connection so
/// every `IssueRow` we upsert downstream has a parent row to point at. The
/// returned id is what tests should stamp on `IssueRow::connection_id` /
/// `WorklogRow::connection_id`.
fn fresh_db() -> (TempDir, Db, i64) {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let conn_id = insert_conn(
        &db,
        NewConnection {
            provider: "jira",
            name: "test",
            enabled: true,
            config_json: "{}",
        },
    )
    .expect("seed connection");
    (dir, db, conn_id)
}

#[test]
fn open_creates_database_file_and_wal_mode() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("tracker.db");

    let db = Db::open(&path).expect("open");
    let conn = db.pool().get().unwrap();

    let mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");

    assert!(path.exists());
}

#[test]
fn migrations_create_all_core_tables() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(&dir.path().join("t.db")).unwrap();
    let conn = db.pool().get().unwrap();

    // After migration 0012 the v1 tables (`issues`, `recent_worklogs`) were
    // renamed to `*_legacy` and the new provider-neutral tables came online:
    // `issues_v2` + `worklogs`. The rest of the operational set kept its
    // names. We pin both sets so an accidental migration rename in either
    // direction lights up here.
    for table in [
        "issues_v2",
        "worklogs",
        "active_timer",
        "app_settings",
        "schema_migrations",
        "connections",
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "table {table} should exist");
    }
}

#[test]
fn upsert_and_get_issue_roundtrip() {
    let (_d, db, conn_id) = fresh_db();
    let issue = IssueRow {
        connection_id: conn_id,
        issue_id: "ABC-1".into(),
        issue_key: "ABC-1".into(),
        name: "Fix login".into(),
        status: Some("In Progress".into()),
        updated_at: 1_700_000_000,
        ..Default::default()
    };
    upsert(&db, &issue).unwrap();

    let got = get_by_key(&db, "ABC-1").unwrap().unwrap();
    assert_eq!(got.name, "Fix login");
    assert_eq!(got.status.as_deref(), Some("In Progress"));
}

#[test]
fn upsert_overwrites_existing_issue() {
    let (_d, db, conn_id) = fresh_db();
    upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: "ABC-1".into(),
            issue_key: "ABC-1".into(),
            name: "Old summary".into(),
            updated_at: 1,
            ..Default::default()
        },
    )
    .unwrap();
    upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: "ABC-1".into(),
            issue_key: "ABC-1".into(),
            name: "New summary".into(),
            updated_at: 2,
            ..Default::default()
        },
    )
    .unwrap();

    let got = get_by_key(&db, "ABC-1").unwrap().unwrap();
    assert_eq!(got.name, "New summary");
    assert_eq!(got.updated_at, 2);
}

#[test]
fn get_by_key_returns_none_for_missing() {
    let (_d, db, _) = fresh_db();
    assert!(get_by_key(&db, "NOPE-1").unwrap().is_none());
}

#[test]
fn search_matches_summary_substring_case_insensitive() {
    let (_d, db, conn_id) = fresh_db();
    upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: "A-1".into(),
            issue_key: "A-1".into(),
            name: "Login bug".into(),
            updated_at: 1,
            ..Default::default()
        },
    )
    .unwrap();
    upsert(
        &db,
        &IssueRow {
            connection_id: conn_id,
            issue_id: "A-2".into(),
            issue_key: "A-2".into(),
            name: "Database migration".into(),
            updated_at: 2,
            ..Default::default()
        },
    )
    .unwrap();

    let hits = search(&db, "LOGIN", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].issue_key, "A-1");
}

#[test]
fn timer_singleton_overwrites_on_start() {
    let (_d, db, _) = fresh_db();
    timer_start(&db, "A-1", 1_000).unwrap();
    timer_start(&db, "B-2", 2_000).unwrap();
    let t = timer_get(&db).unwrap().unwrap();
    assert_eq!(t.issue_key, "B-2");
    assert_eq!(t.started_at, 2_000);
}

#[test]
fn timer_stop_clears_state() {
    let (_d, db, _) = fresh_db();
    timer_start(&db, "A-1", 1).unwrap();
    timer_stop(&db).unwrap();
    assert!(timer_get(&db).unwrap().is_none());
}

#[test]
fn timer_get_returns_none_when_no_timer() {
    let (_d, db, _) = fresh_db();
    assert!(timer_get(&db).unwrap().is_none());
}

#[test]
fn record_and_list_worklogs_ordered_by_logged_at_desc() {
    let (_d, db, conn_id) = fresh_db();
    let id1 = worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("A-1".into()),
            started_at: 1,
            ended_at: 1 + 600,
            logged_at: 100,
            ..Default::default()
        },
    )
    .unwrap();
    let id2 = worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("A-2".into()),
            started_at: 1,
            ended_at: 1 + 300,
            logged_at: 200,
            ..Default::default()
        },
    )
    .unwrap();

    assert_ne!(id1, id2, "ids must be auto-assigned and unique");

    let rows = worklog_recent(&db, 10).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].issue_key.as_deref(), Some("A-2"));
    assert_eq!(rows[1].issue_key.as_deref(), Some("A-1"));
    assert_eq!(rows[0].id, Some(id2));
    assert_eq!(rows[1].id, Some(id1));
}

#[test]
fn recent_respects_limit() {
    let (_d, db, conn_id) = fresh_db();
    for i in 0..5 {
        worklog_record(
            &db,
            &WorklogRow {
                connection_id: Some(conn_id),
                issue_key: Some(format!("X-{i}")),
                started_at: 0,
                ended_at: 60,
                logged_at: i,
                ..Default::default()
            },
        )
        .unwrap();
    }
    let rows = worklog_recent(&db, 3).unwrap();
    assert_eq!(rows.len(), 3);
}

#[test]
fn settings_get_returns_none_for_missing_key() {
    let (_d, db, _) = fresh_db();
    assert!(setting_get(&db, "no.such.key").unwrap().is_none());
}

#[test]
fn settings_set_and_get_roundtrip() {
    let (_d, db, _) = fresh_db();
    setting_set(&db, "daily_goal_minutes", "480").unwrap();
    assert_eq!(
        setting_get(&db, "daily_goal_minutes").unwrap().as_deref(),
        Some("480")
    );
}

#[test]
fn settings_set_overwrites_existing_value() {
    let (_d, db, _) = fresh_db();
    setting_set(&db, "k", "v1").unwrap();
    setting_set(&db, "k", "v2").unwrap();
    assert_eq!(setting_get(&db, "k").unwrap().as_deref(), Some("v2"));
}

// -----------------------------------------------------------------------------
// Phase 11A: new worklog methods (Jira-sourced rows, date-range queries).
// -----------------------------------------------------------------------------

#[test]
fn migration_runner_replays_idempotently_on_reopen() {
    // Open the DB once to apply every migration, then reopen it. The migration
    // runner must be idempotent (each version recorded in schema_migrations
    // gates a re-run), so the second open should not flap on the v2 tables.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("upgrade.db");
    {
        let db = Db::open(&path).unwrap();
        // Insert a row through the provider-neutral upsert path to make
        // sure data is in place across the reopen. The original test
        // inserted into the now-legacy `recent_worklogs`; we use the new
        // `worklogs` table via `record(...)` instead.
        let conn_id = insert_conn(
            &db,
            NewConnection {
                provider: "jira",
                name: "test",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        worklog_record(
            &db,
            &WorklogRow {
                connection_id: Some(conn_id),
                issue_key: Some("OLD-1".into()),
                started_at: 1,
                ended_at: 1 + 600,
                logged_at: 1,
                updated_at: 1,
                ..Default::default()
            },
        )
        .unwrap();
    }

    // Reopen — every prior migration must already be marked applied; the
    // pre-existing row must still be queryable.
    let db = Db::open(&path).unwrap();
    let conn = db.pool().get().unwrap();
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM worklogs WHERE issue_key = 'OLD-1')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(exists, "row from before reopen should still exist");

    // schema_migrations should record every applied version (1..=14 at the
    // time of writing — keep this in lockstep with `migrations/`).
    let mut stmt = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
        .unwrap();
    let versions: Vec<i32> = stmt
        .query_map([], |r| r.get::<_, i32>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        versions,
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
}

#[test]
fn worklogs_table_has_provider_neutral_columns() {
    // The v1 `recent_worklogs` carried Jira-specific columns
    // (`author_account_id`, `source`, `updated_at_jira`). Migration 0012
    // replaces it with `worklogs` whose schema is provider-agnostic:
    // `connection_id` + `is_synced`/`synced_at`/`remote_id` carry the same
    // information without baking Jira semantics into column names.
    let (_d, db, _) = fresh_db();
    let conn = db.pool().get().unwrap();

    let mut stmt = conn.prepare("PRAGMA table_info(worklogs)").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    for col in [
        "connection_id",
        "issue_key",
        "description",
        "started_at",
        "ended_at",
        "is_synced",
        "synced_at",
        "remote_id",
        "tombstoned_at",
    ] {
        assert!(
            names.iter().any(|n| n == col),
            "column {col} should exist (have: {names:?})"
        );
    }
}

#[test]
fn upsert_from_remote_replaces_by_remote_id() {
    let (_d, db, conn_id) = fresh_db();

    // Insert an initial Jira-sourced row.
    let id1 = upsert_from_remote(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("ACME-1".into()),
            description: Some("initial".into()),
            started_at: 100,
            ended_at: 100 + 600,
            logged_at: 100,
            updated_at: 1_000,
            is_synced: true,
            synced_at: Some(1_000),
            remote_id: Some("J-42".into()),
            ..Default::default()
        },
    )
    .unwrap();

    // Replace via the same remote id with updated data.
    let id2 = upsert_from_remote(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("ACME-1".into()),
            description: Some("updated".into()),
            started_at: 100,
            ended_at: 100 + 900,
            logged_at: 200,
            updated_at: 2_000,
            is_synced: true,
            synced_at: Some(2_000),
            remote_id: Some("J-42".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        id1, id2,
        "second upsert must reuse the original rowid (no duplicate row)"
    );

    let all = worklog_recent(&db, 50).unwrap();
    assert_eq!(all.len(), 1, "should still be exactly one row");
    assert_eq!(all[0].duration_s(), 900);
    assert_eq!(all[0].description.as_deref(), Some("updated"));
    assert_eq!(all[0].synced_at, Some(2_000));
    assert!(all[0].is_synced);
}

#[test]
fn upsert_from_remote_rejects_missing_remote_id() {
    let (_d, db, conn_id) = fresh_db();
    let result = upsert_from_remote(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("A-1".into()),
            started_at: 0,
            ended_at: 60,
            ..Default::default()
        },
    );
    assert!(result.is_err(), "missing remote_id must be rejected");
}

#[test]
fn for_date_range_filters_and_orders() {
    let (_d, db, conn_id) = fresh_db();

    // started_at = 1000 — inside the range.
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("IN-1".into()),
            started_at: 1_000,
            ended_at: 1_000 + 100,
            logged_at: 1_000,
            ..Default::default()
        },
    )
    .unwrap();

    // started_at = 2000 — inside the range.
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("IN-2".into()),
            started_at: 2_000,
            ended_at: 2_000 + 200,
            logged_at: 2_000,
            ..Default::default()
        },
    )
    .unwrap();

    // started_at = 5000 — outside the range.
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("OUT-1".into()),
            started_at: 5_000,
            ended_at: 5_000 + 50,
            logged_at: 5_000,
            ..Default::default()
        },
    )
    .unwrap();

    // The account-id filter is gone from `for_date_range` (it's implicit
    // per-connection now). Asserting the basic in/out behavior captures
    // the surviving contract.
    let all = worklog_for_date_range(&db, 0, 4_000).unwrap();
    let keys: Vec<&str> = all
        .iter()
        .map(|w| w.issue_key.as_deref().unwrap_or(""))
        .collect();
    assert_eq!(keys, vec!["IN-2", "IN-1"]);
}

#[test]
fn total_seconds_for_range_sums_correctly() {
    let (_d, db, conn_id) = fresh_db();
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("A".into()),
            started_at: 1_000,
            ended_at: 1_000 + 100,
            logged_at: 1_000,
            ..Default::default()
        },
    )
    .unwrap();
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("B".into()),
            started_at: 2_000,
            ended_at: 2_000 + 250,
            logged_at: 2_000,
            ..Default::default()
        },
    )
    .unwrap();
    // Out of range.
    worklog_record(
        &db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some("C".into()),
            started_at: 9_999,
            ended_at: 9_999 + 50,
            logged_at: 9_999,
            ..Default::default()
        },
    )
    .unwrap();

    // `total_seconds_for_range` no longer accepts an author filter (per-
    // connection is implicit). The sum is now duration-derived from
    // `ended_at - started_at`.
    let total = total_seconds_for_range(&db, 0, 5_000).unwrap();
    assert_eq!(total, 350);

    // Empty range.
    let none = total_seconds_for_range(&db, 100_000, 200_000).unwrap();
    assert_eq!(none, 0);
}

// -----------------------------------------------------------------------------
// Phase 15: soft-delete + tombstone + audit
// -----------------------------------------------------------------------------

fn seed_remote_row(db: &Db, conn_id: i64, remote_id: &str, issue_key: &str) -> i64 {
    upsert_from_remote(
        db,
        &WorklogRow {
            connection_id: Some(conn_id),
            issue_key: Some(issue_key.into()),
            description: Some("c".into()),
            started_at: 1_700_000_000,
            ended_at: 1_700_000_000 + 1800,
            logged_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            is_synced: true,
            synced_at: Some(1_700_000_000),
            remote_id: Some(remote_id.into()),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn mark_pending_delete_then_clear_roundtrip() {
    let (_d, db, conn_id) = fresh_db();
    let id = seed_remote_row(&db, conn_id, "j-1", "K-1");

    mark_pending_delete(&db, id, 1_700_000_100).unwrap();
    let row = get_by_id(&db, id).unwrap().unwrap();
    assert_eq!(row.pending_delete_at, Some(1_700_000_100));
    assert!(row.tombstoned_at.is_none());

    clear_pending_delete(&db, id).unwrap();
    let row = get_by_id(&db, id).unwrap().unwrap();
    assert!(row.pending_delete_at.is_none());
}

#[test]
fn mark_tombstoned_clears_pending_delete() {
    let (_d, db, conn_id) = fresh_db();
    let id = seed_remote_row(&db, conn_id, "j-1", "K-1");
    mark_pending_delete(&db, id, 1_700_000_100).unwrap();

    mark_tombstoned(&db, id, 1_700_000_200).unwrap();
    let row = get_by_id(&db, id).unwrap().unwrap();
    assert_eq!(row.tombstoned_at, Some(1_700_000_200));
    assert!(row.pending_delete_at.is_none());
}

#[test]
fn mark_tombstoned_by_remote_id_targets_correct_row() {
    let (_d, db, conn_id) = fresh_db();
    let _a = seed_remote_row(&db, conn_id, "j-a", "K-1");
    let b = seed_remote_row(&db, conn_id, "j-b", "K-1");
    mark_tombstoned_by_remote_id(&db, conn_id, "j-b", 1_700_000_300).unwrap();

    let row_a = get_by_remote_id_any(&db, "j-a").unwrap().unwrap();
    let row_b = get_by_id(&db, b).unwrap().unwrap();
    assert!(row_a.tombstoned_at.is_none());
    assert_eq!(row_b.tombstoned_at, Some(1_700_000_300));
}

#[test]
fn pending_deletes_older_than_filters_correctly() {
    let (_d, db, conn_id) = fresh_db();
    let id_old = seed_remote_row(&db, conn_id, "j-old", "K-1");
    let id_new = seed_remote_row(&db, conn_id, "j-new", "K-1");
    mark_pending_delete(&db, id_old, 100).unwrap();
    mark_pending_delete(&db, id_new, 10_000).unwrap();

    let stale = pending_deletes_older_than(&db, 1_000).unwrap();
    let ids: Vec<i64> = stale.iter().map(|r| r.id.unwrap()).collect();
    assert_eq!(ids, vec![id_old]);
    assert!(!ids.contains(&id_new));
}

#[test]
fn update_fields_writes_new_values() {
    let (_d, db, conn_id) = fresh_db();
    let id = seed_remote_row(&db, conn_id, "j-1", "K-1");
    // `update_fields` now takes a slimmer arg list: (id, issue_key,
    // description, started_at, ended_at, synced_at). The old call sig
    // included issue_id/summary/duration_s/updated_at_jira — all dropped
    // or moved to derived/JOIN territory.
    update_fields(
        &db,
        id,
        Some("K-2"),
        Some("Updated"),
        1_800_000_000,
        1_800_000_000 + 3600,
        Some(1_800_000_500),
    )
    .unwrap();

    let row = get_by_id(&db, id).unwrap().unwrap();
    assert_eq!(row.issue_key.as_deref(), Some("K-2"));
    assert_eq!(row.duration_s(), 3600);
    assert_eq!(row.started_at, 1_800_000_000);
    assert_eq!(row.description.as_deref(), Some("Updated"));
    assert_eq!(row.synced_at, Some(1_800_000_500));
    assert!(row.is_synced);
}

#[test]
fn audit_record_and_recent_roundtrip() {
    let (_d, db, conn_id) = fresh_db();
    let row = WorklogRow {
        id: Some(1),
        connection_id: Some(conn_id),
        issue_key: Some("K-1".into()),
        started_at: 1,
        ended_at: 1 + 1800,
        logged_at: 1,
        updated_at: 1,
        is_synced: true,
        remote_id: Some("j-1".into()),
        ..Default::default()
    };

    audit_record(
        &db,
        AuditEvent {
            occurred_at: 100,
            op: AuditOp::Create,
            issue_key: Some("K-1"),
            worklog_id: Some("j-1"),
            before: None,
            after: Some(&row),
            success: true,
            error: None,
            source_audit_id: None,
        },
    )
    .unwrap();
    audit_record(
        &db,
        AuditEvent {
            occurred_at: 200,
            op: AuditOp::Delete,
            issue_key: Some("K-1"),
            worklog_id: Some("j-1"),
            before: Some(&row),
            after: None,
            success: false,
            error: Some("network glitch"),
            source_audit_id: None,
        },
    )
    .unwrap();

    let entries = audit_recent(&db, 10).unwrap();
    assert_eq!(entries.len(), 2);
    // Newest first.
    assert_eq!(entries[0].op, "delete");
    assert!(!entries[0].success);
    assert_eq!(entries[0].error.as_deref(), Some("network glitch"));
    assert!(entries[0].before_json.is_some());
    assert!(entries[0].after_json.is_none());

    assert_eq!(entries[1].op, "create");
    assert!(entries[1].success);
    assert!(entries[1].after_json.is_some());
    assert!(entries[1].before_json.is_none());
}

#[test]
fn worklog_default_recent_hides_tombstoned() {
    let (_d, db, conn_id) = fresh_db();
    let id = seed_remote_row(&db, conn_id, "j-1", "K-1");
    let _ = seed_remote_row(&db, conn_id, "j-2", "K-1");
    mark_tombstoned(&db, id, 1_700_000_500).unwrap();

    let recent_rows = worklog_recent(&db, 50).unwrap();
    let ids: Vec<&str> = recent_rows
        .iter()
        .map(|r| r.remote_id.as_deref().unwrap_or(""))
        .collect();
    assert!(ids.contains(&"j-2"));
    assert!(!ids.contains(&"j-1"));
}

// -----------------------------------------------------------------------------
// Phase 16 — audit log pagination + filter + linkage
// -----------------------------------------------------------------------------

fn seed_audit_op(db: &Db, op: AuditOp, occurred_at: i64, success: bool, key: &str) -> i64 {
    audit_record(
        db,
        AuditEvent {
            occurred_at,
            op,
            issue_key: Some(key),
            worklog_id: Some(&format!("wl-{occurred_at}")),
            before: None,
            after: None,
            success,
            error: if success { None } else { Some("boom") },
            source_audit_id: None,
        },
    )
    .unwrap()
}

#[test]
fn audit_log_pagination_by_before_id_returns_correct_slice() {
    let (_d, db, _) = fresh_db();
    // Seed 5 rows with strictly increasing occurred_at.
    let _id1 = seed_audit_op(&db, AuditOp::Create, 100, true, "K-1");
    let _id2 = seed_audit_op(&db, AuditOp::Update, 200, true, "K-1");
    let _id3 = seed_audit_op(&db, AuditOp::Delete, 300, true, "K-2");
    let _id4 = seed_audit_op(&db, AuditOp::Update, 400, true, "K-2");
    let id5 = seed_audit_op(&db, AuditOp::Delete, 500, true, "K-3");

    // First page (newest first) — limit 2 → ids 5, 4.
    let page1 = audit_list(&db, 2, None, None, false).unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].id, id5);
    assert_eq!(page1[0].occurred_at, 500);

    // Pagination: pass the last id of page1 as `before_id`.
    let last_id = page1.last().unwrap().id;
    let page2 = audit_list(&db, 2, Some(last_id), None, false).unwrap();
    assert_eq!(page2.len(), 2);
    // Should be ids 3, 2.
    assert_eq!(page2[0].occurred_at, 300);
    assert_eq!(page2[1].occurred_at, 200);

    // Final page.
    let last_id = page2.last().unwrap().id;
    let page3 = audit_list(&db, 2, Some(last_id), None, false).unwrap();
    assert_eq!(page3.len(), 1);
    assert_eq!(page3[0].occurred_at, 100);
}

#[test]
fn audit_log_filter_ops_restricts_results() {
    let (_d, db, _) = fresh_db();
    let _ = seed_audit_op(&db, AuditOp::Create, 100, true, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Update, 200, true, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Delete, 300, true, "K-2");
    let _ = seed_audit_op(&db, AuditOp::Update, 400, true, "K-2");
    let _ = seed_audit_op(&db, AuditOp::Delete, 500, true, "K-3");

    let ops = vec!["delete".to_string()];
    let filtered = audit_list(&db, 50, None, Some(&ops), false).unwrap();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.op == "delete"));

    let ops2 = vec!["update".to_string(), "delete".to_string()];
    let filtered2 = audit_list(&db, 50, None, Some(&ops2), false).unwrap();
    assert_eq!(filtered2.len(), 4);

    // Empty ops list ≡ no filter.
    let empty: Vec<String> = Vec::new();
    let no_filter = audit_list(&db, 50, None, Some(&empty), false).unwrap();
    assert_eq!(no_filter.len(), 5);
}

#[test]
fn audit_log_filter_only_failed_excludes_successful_entries() {
    let (_d, db, _) = fresh_db();
    let _ = seed_audit_op(&db, AuditOp::Create, 100, true, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Create, 200, false, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Delete, 300, false, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Update, 400, true, "K-1");

    let failed = audit_list(&db, 50, None, None, true).unwrap();
    assert_eq!(failed.len(), 2);
    assert!(failed.iter().all(|e| !e.success));
}

#[test]
fn audit_log_default_uses_50_limit_by_recent() {
    let (_d, db, _) = fresh_db();
    for i in 0..60 {
        seed_audit_op(&db, AuditOp::Create, 100 + i, true, "K-1");
    }
    // `recent` is the convenience for "first page, no filters".
    let entries = audit_recent(&db, 50).unwrap();
    assert_eq!(entries.len(), 50);
    // Newest first → highest occurred_at on top.
    assert!(entries[0].occurred_at > entries[entries.len() - 1].occurred_at);
}

#[test]
fn audit_get_by_id_returns_full_entry_or_none() {
    let (_d, db, _) = fresh_db();
    let id = seed_audit_op(&db, AuditOp::Delete, 100, true, "K-1");
    let entry = audit_get_by_id(&db, id).unwrap().expect("present");
    assert_eq!(entry.op, "delete");
    assert_eq!(entry.issue_key.as_deref(), Some("K-1"));

    assert!(audit_get_by_id(&db, 9999).unwrap().is_none());
}

#[test]
fn audit_purge_older_than_removes_old_rows_only() {
    let (_d, db, _) = fresh_db();
    let _ = seed_audit_op(&db, AuditOp::Create, 100, true, "K-1");
    let _ = seed_audit_op(&db, AuditOp::Update, 200, true, "K-1");
    let kept = seed_audit_op(&db, AuditOp::Delete, 400, true, "K-1");

    let removed = audit_purge(&db, 300).unwrap();
    assert_eq!(removed, 2);

    let remaining = audit_list(&db, 50, None, None, false).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, kept);
}

#[test]
fn audit_source_audit_id_linkage_persists() {
    let (_d, db, _) = fresh_db();
    let original = seed_audit_op(&db, AuditOp::Delete, 100, true, "K-1");
    let restore_id = audit_record(
        &db,
        AuditEvent {
            occurred_at: 200,
            op: AuditOp::Restore,
            issue_key: Some("K-1"),
            worklog_id: Some("new-id"),
            before: None,
            after: None,
            success: true,
            error: None,
            source_audit_id: Some(original),
        },
    )
    .unwrap();

    let entry = audit_get_by_id(&db, restore_id).unwrap().expect("present");
    assert_eq!(entry.source_audit_id, Some(original));
    assert_eq!(entry.op, "restore");
}
