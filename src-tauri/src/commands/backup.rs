//! Backup / restore — export DB do JSON a zpět.
//!
//! Backup obsahuje vše, co aplikace potřebuje k reprodukci stavu, **kromě
//! tokenů** (ty žijí v `secret.toml` mimo DB):
//!  - `worklogs` (včetně tombstoned řádků — máme je držet navždy)
//!  - `issues_v2`
//!  - `connections` (config bez tokenu)
//!  - `app_settings`
//!  - `audit_log`
//!  - `favorite_issues`
//!  - `daily_activity`
//!  - `non_working_days`
//!
//! Restore je destruktivní — TRUNCATE před INSERT, ať se po obnově data
//! z dvou původů nesměšují. UI ukazuje confirm dialog.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Emitter;

use crate::cache::Db;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupBundle {
    pub version: u32,
    pub generated_at: i64,
    pub tables: BackupTables,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupTables {
    pub worklogs: Vec<Value>,
    pub issues_v2: Vec<Value>,
    pub connections: Vec<Value>,
    pub app_settings: Vec<Value>,
    pub audit_log: Vec<Value>,
    pub favorite_issues: Vec<Value>,
    pub daily_activity: Vec<Value>,
    pub non_working_days: Vec<Value>,
}

const BACKUP_VERSION: u32 = 1;

/// Export — sebere všechny tabulky do JSON. Tokeny se v exportu neobjeví,
/// protože nejsou v `connections` tabulce.
#[tauri::command]
pub async fn export_backup(state: tauri::State<'_, AppState>) -> Result<BackupBundle, String> {
    export_inner(&state.db).map_err(|e| e.to_string())
}

fn export_inner(db: &Db) -> Result<BackupBundle, rusqlite::Error> {
    let conn = db.pool().get().expect("db pool");
    let tables = BackupTables {
        worklogs: dump_table(
            &conn,
            "SELECT id, connection_id, issue_key, description, started_at, ended_at, logged_at, updated_at, is_synced, synced_at, remote_id, pending_delete_at, tombstoned_at FROM worklogs",
        )?,
        issues_v2: dump_table(
            &conn,
            "SELECT id, connection_id, issue_id, issue_key, name, parent_key, parent_name, status, is_archived, created_at, updated_at, remote_updated_at, last_synced_at FROM issues_v2",
        )?,
        connections: dump_table(
            &conn,
            "SELECT id, provider, name, enabled, created_at, updated_at, config_json FROM connections",
        )?,
        app_settings: dump_table(&conn, "SELECT key, value FROM app_settings")?,
        audit_log: dump_table(
            &conn,
            "SELECT id, occurred_at, op, issue_key, worklog_id, before_json, after_json, success, error, source_audit_id FROM audit_log",
        )?,
        favorite_issues: dump_optional_table(
            &conn,
            "favorite_issues",
            "SELECT issue_key, connection_id, added_at FROM favorite_issues",
        )?,
        daily_activity: dump_optional_table(
            &conn,
            "daily_activity",
            "SELECT date, active_seconds, inactive_seconds FROM daily_activity",
        )?,
        non_working_days: dump_optional_table(
            &conn,
            "non_working_days",
            "SELECT date, reason, label, created_at FROM non_working_days",
        )?,
    };
    Ok(BackupBundle {
        version: BACKUP_VERSION,
        generated_at: chrono::Utc::now().timestamp(),
        tables,
    })
}

/// Is `table` present in this database?
fn table_exists(conn: &rusqlite::Connection, table: &str) -> Result<bool, rusqlite::Error> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n != 0)
}

/// Dump a table that a sufficiently old database might not have yet.
///
/// Absence is checked explicitly rather than inferred from a failed query.
/// The previous `unwrap_or_default()` could not tell "this database predates
/// the table" from "the read failed", so a locked database or an unreadable
/// row produced a backup that looked complete and silently held nothing --
/// which the destructive import would then write over the real data.
fn dump_optional_table(
    conn: &rusqlite::Connection,
    table: &str,
    sql: &str,
) -> Result<Vec<Value>, rusqlite::Error> {
    if !table_exists(conn, table)? {
        return Ok(Vec::new());
    }
    dump_table(conn, sql)
}

/// Read all rows from a SELECT and convert to `Vec<serde_json::Value>` keyed
/// by column name. Funguje pro libovolný typ sloupce, který sqlite vrací
/// jako Null/Integer/Real/Text/Blob.
fn dump_table(conn: &rusqlite::Connection, sql: &str) -> Result<Vec<Value>, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;
    let column_names: Vec<String> = stmt
        .column_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let rows = stmt.query_map([], |row| {
        let mut map = serde_json::Map::new();
        for (i, name) in column_names.iter().enumerate() {
            let v: Value = match row.get_ref(i)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => Value::from(n),
                rusqlite::types::ValueRef::Real(f) => Value::from(f),
                rusqlite::types::ValueRef::Text(t) => {
                    Value::from(String::from_utf8_lossy(t).to_string())
                }
                rusqlite::types::ValueRef::Blob(b) => Value::from(b.to_vec()),
            };
            map.insert(name.clone(), v);
        }
        Ok(Value::Object(map))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
}

/// Import — TRUNCATE všech přepisovaných tabulek + INSERT z bundle.
/// Provedeno v transakci, takže fail vrátí stav před importem.
///
/// Po úspěšném importu rehydratujeme runtime stav AppState a emitujeme
/// stejné `connections-changed` / `config-changed` / `cache-refreshed`
/// eventy jako bežné config / connections commandy. Bez toho by aplikace
/// běžela dál se starými HTTP klienty a `jira_client_cloned()` shimem ze
/// stavu před importem — uživatel by viděl správná data v UI, ale
/// mutations (nový worklog, sync, ...) by mohly trefit už neexistující
/// tenant nebo nový tenant na cestě skrz toho starého.
#[tauri::command]
pub async fn import_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    bundle: BackupBundle,
) -> Result<ImportStats, String> {
    if bundle.version > BACKUP_VERSION {
        return Err(format!(
            "Zálohu z budoucí verze ({}) tato instalace neumí načíst",
            bundle.version
        ));
    }
    let stats = import_inner(&state.db, &bundle)?;

    apply_post_import_state_refresh(&state);

    // Tell the FE everything just changed under it. Existing handlers in
    // `AppShell` already invalidate the relevant React Query keys on each
    // of these.
    let _ = app.emit("connections-changed", ());
    let _ = app.emit("config-changed", ());
    // `cache-refreshed` doubles as "all the worklog / issue lists you've
    // cached need to be re-fetched" — matches what the desktop refresh
    // emits at the end of `refresh_cache`.
    let _ = app.emit("cache-refreshed", stats.worklogs);

    Ok(stats)
}

/// Rebuild runtime state from the post-import DB. Extracted from the
/// Tauri command so integration tests can verify the shim-reset +
/// hydrate sequence without a Tauri runtime / AppHandle.
///
/// 1) Drop the legacy single-Jira shims (`jira_client`, `jira_config`)
///    so a removed-on-restore tenant doesn't keep getting hit.
/// 2) Re-hydrate `state.connections` from the imported rows. Errors are
///    logged but not propagated — the DB is already in the correct
///    state; the worst case is the user has to restart the app to pick
///    up the new clients.
pub fn apply_post_import_state_refresh(state: &AppState) {
    *state.jira_client.write().unwrap_or_else(|e| e.into_inner()) = None;
    *state.jira_config.write().unwrap_or_else(|e| e.into_inner()) = None;
    if let Err(e) = state.hydrate_connections() {
        tracing::warn!("import_backup: hydrate_connections after restore failed: {e}");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStats {
    pub worklogs: usize,
    pub issues_v2: usize,
    pub connections: usize,
    pub audit_log: usize,
    pub app_settings: usize,
}

/// Insert one table's rows, naming the table if anything goes wrong.
///
/// Every table goes through here so a failure is attributable. The three
/// "optional" tables used to swallow their result with `let _`, which meant a
/// failed insert rode along with a committed transaction: the truncate had
/// already happened, so the user's favourites, activity history and
/// non-working days were gone and the import still reported success.
fn insert_table(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    rows: &[Value],
) -> Result<usize, String> {
    insert_rows(tx, table, rows).map_err(|e| format!("obnova tabulky {table} selhala: {e}"))
}

fn import_inner(db: &Db, bundle: &BackupBundle) -> Result<ImportStats, String> {
    let mut conn = db
        .pool()
        .get()
        .map_err(|e| format!("databáze není dostupná: {e}"))?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Truncate v opačném pořadí kvůli FK (audit drží jen string id, FK neřeší,
    // ale pro robustnost).
    for table in [
        "audit_log",
        "favorite_issues",
        "daily_activity",
        "non_working_days",
        "worklogs",
        "issues_v2",
        "connections",
        "app_settings",
    ] {
        tx.execute(&format!("DELETE FROM {table}"), [])
            .map_err(|e| format!("vyprázdnění tabulky {table} selhalo: {e}"))?;
    }

    // Vkládáme rodiče před potomky, ať sedí per-connection FK:
    // `issues_v2.connection_id` je NOT NULL → connections(id), `worklogs` a
    // `favorite_issues` na connections také referují. `connections` proto MUSÍ
    // první; `issues_v2` a `worklogs` jsou mezi sebou nezávislé (žádný FK
    // worklogs→issues_v2). Truncate výše běží v opačném pořadí (potomci první).
    let n_conns = insert_table(&tx, "connections", &bundle.tables.connections)?;
    let n_issues = insert_table(&tx, "issues_v2", &bundle.tables.issues_v2)?;
    let n_worklogs = insert_table(&tx, "worklogs", &bundle.tables.worklogs)?;
    let n_audit = insert_table(&tx, "audit_log", &bundle.tables.audit_log)?;
    let n_settings = insert_table(&tx, "app_settings", &bundle.tables.app_settings)?;
    insert_table(&tx, "favorite_issues", &bundle.tables.favorite_issues)?;
    insert_table(&tx, "daily_activity", &bundle.tables.daily_activity)?;
    insert_table(&tx, "non_working_days", &bundle.tables.non_working_days)?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(ImportStats {
        worklogs: n_worklogs,
        issues_v2: n_issues,
        connections: n_conns,
        audit_log: n_audit,
        app_settings: n_settings,
    })
}

/// Dynamicky postavit INSERT z prvního řádku — sloupce odvodíme z JSON keys.
fn insert_rows(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    rows: &[Value],
) -> Result<usize, rusqlite::Error> {
    if rows.is_empty() {
        return Ok(0);
    }
    let first = rows
        .first()
        .and_then(|v| v.as_object())
        .ok_or_else(|| rusqlite::Error::InvalidQuery)?;
    let columns: Vec<&str> = first.keys().map(|s| s.as_str()).collect();
    let placeholders = (1..=columns.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let cols_csv = columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("INSERT INTO {table} ({cols_csv}) VALUES ({placeholders})");
    let mut stmt = tx.prepare(&sql)?;
    let mut n = 0usize;
    for row in rows {
        let obj = match row.as_object() {
            Some(o) => o,
            None => continue,
        };
        let params: Vec<rusqlite::types::Value> = columns
            .iter()
            .map(|c| json_to_sqlite(obj.get(*c).unwrap_or(&Value::Null)))
            .collect();
        stmt.execute(rusqlite::params_from_iter(params.iter()))?;
        n += 1;
    }
    Ok(n)
}

fn json_to_sqlite(v: &Value) -> rusqlite::types::Value {
    use rusqlite::types::Value as Sv;
    match v {
        Value::Null => Sv::Null,
        Value::Bool(b) => Sv::Integer(if *b { 1 } else { 0 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Sv::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Sv::Real(f)
            } else {
                Sv::Null
            }
        }
        Value::String(s) => Sv::Text(s.clone()),
        Value::Array(_) | Value::Object(_) => Sv::Text(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    #[test]
    fn export_and_import_roundtrips_app_settings() {
        let db = open_db();
        // Seed jednu hodnotu.
        crate::cache::settings::set(&db, "test_key", "hello").unwrap();
        let bundle = export_inner(&db).unwrap();
        assert!(bundle
            .tables
            .app_settings
            .iter()
            .any(|v| v.get("key").and_then(|x| x.as_str()) == Some("test_key")));

        // Clear + import.
        crate::cache::settings::remove(&db, "test_key").unwrap();
        assert!(crate::cache::settings::get(&db, "test_key")
            .unwrap()
            .is_none());
        let _stats = import_inner(&db, &bundle).unwrap();
        assert_eq!(
            crate::cache::settings::get(&db, "test_key")
                .unwrap()
                .as_deref(),
            Some("hello"),
        );
    }

    #[test]
    fn import_rejects_future_version() {
        let db = open_db();
        let bundle = BackupBundle {
            version: BACKUP_VERSION + 1,
            generated_at: 0,
            tables: BackupTables::default(),
        };
        // Sync helper přes import_inner — kontrola version je v Tauri wrapperu,
        // ale tady jen ujistíme, že prázdný bundle projde DELETE bez paniky.
        assert!(import_inner(&db, &bundle).is_ok());
    }

    #[test]
    fn export_omits_a_table_the_database_does_not_have() {
        let db = open_db();
        seed_connection_issue_worklog(&db);
        {
            let conn = db.pool().get().unwrap();
            conn.execute("DROP TABLE favorite_issues", []).unwrap();
        }

        let bundle = export_inner(&db).unwrap();
        assert!(bundle.tables.favorite_issues.is_empty());
        // A missing optional table must not cost us the rest of the backup.
        assert_eq!(bundle.tables.worklogs.len(), 1);
        assert_eq!(bundle.tables.connections.len(), 1);
    }

    #[test]
    fn export_carries_optional_tables_that_do_exist() {
        let db = open_db();
        let conn_id = seed_connection_issue_worklog(&db);
        crate::cache::favorites::add(&db, "ACME-1", Some(conn_id)).unwrap();

        let bundle = export_inner(&db).unwrap();
        assert_eq!(
            bundle.tables.favorite_issues.len(),
            1,
            "a present table must be exported, not silently emptied"
        );
    }

    #[test]
    fn a_failing_table_aborts_the_whole_import() {
        let db = open_db();
        seed_connection_issue_worklog(&db);
        crate::cache::settings::set(&db, "keep_me", "before").unwrap();

        // A row whose column does not exist in the schema. Previously this
        // table's result was discarded, so the truncate stood and the rows
        // never came back.
        let mut bundle = export_inner(&db).unwrap();
        bundle.tables.favorite_issues = vec![json!({ "no_such_column": 1 })];

        let err = import_inner(&db, &bundle).unwrap_err();
        assert!(
            err.contains("favorite_issues"),
            "the failing table must be named: {err}"
        );

        // The transaction rolled back, so nothing was lost.
        assert_eq!(
            crate::cache::settings::get(&db, "keep_me")
                .unwrap()
                .as_deref(),
            Some("before"),
        );
        assert_eq!(crate::cache::worklogs::count(&db).unwrap(), 1);
    }

    #[test]
    fn json_to_sqlite_maps_basic_types() {
        use rusqlite::types::Value as Sv;
        assert!(matches!(json_to_sqlite(&Value::Null), Sv::Null));
        assert!(matches!(json_to_sqlite(&json!(42)), Sv::Integer(42)));
        assert!(matches!(json_to_sqlite(&json!(1.5)), Sv::Real(_)));
        assert!(matches!(json_to_sqlite(&json!("hi")), Sv::Text(_)));
        assert!(matches!(json_to_sqlite(&json!(true)), Sv::Integer(1)));
    }

    /// Seed one connection + one issue + one worklog that references it.
    /// Returns the connection id.
    fn seed_connection_issue_worklog(db: &Db) -> i64 {
        let conn_id = crate::cache::connections::insert(
            db,
            crate::cache::connections::NewConnection {
                provider: "jira",
                name: "Tenant A",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap();
        crate::cache::issues::upsert(
            db,
            &crate::cache::issues::IssueRow {
                connection_id: conn_id,
                issue_id: "10001".into(),
                issue_key: "ACME-1".into(),
                name: "Test issue".into(),
                created_at: 1,
                updated_at: 1,
                ..Default::default()
            },
        )
        .unwrap();
        crate::cache::worklogs::record(
            db,
            &crate::cache::worklogs::WorklogRow {
                connection_id: Some(conn_id),
                issue_key: Some("ACME-1".into()),
                description: Some("work".into()),
                started_at: 1_700_000_000,
                ended_at: 1_700_003_600,
                logged_at: 1_700_000_000,
                is_synced: true,
                remote_id: Some("w1".into()),
                ..Default::default()
            },
        )
        .unwrap();
        conn_id
    }

    #[test]
    fn restore_roundtrips_connection_issue_worklog() {
        // AK1.1 / AK1.3: real data with FK relations must survive export→import
        // into a clean DB. Pre-fix this fails: import inserts worklogs/issues_v2
        // before connections, tripping the connections FK.
        let src = open_db();
        seed_connection_issue_worklog(&src);
        let bundle = export_inner(&src).unwrap();

        let dst = open_db();
        import_inner(&dst, &bundle).expect("import of real FK-linked data must not fail");

        assert_eq!(crate::cache::connections::list(&dst).unwrap().len(), 1);
        assert_eq!(crate::cache::issues::count(&dst).unwrap(), 1);
        let worklogs = crate::cache::worklogs::for_date_range(&dst, 0, i64::MAX).unwrap();
        assert_eq!(worklogs.len(), 1);
        assert_eq!(worklogs[0].connection_id, Some(1));
        assert_eq!(worklogs[0].issue_key.as_deref(), Some("ACME-1"));
    }

    #[test]
    fn restore_preserves_favorites_with_connection() {
        // AK1.2: a favorite bound to a connection survives the round-trip.
        // Pre-fix this fails twice over: the import FK error, and the export
        // SELECT reading a non-existent `created_at` column (real col: added_at).
        let src = open_db();
        let conn_id = seed_connection_issue_worklog(&src);
        crate::cache::favorites::add(&src, "ACME-1", Some(conn_id)).unwrap();
        let bundle = export_inner(&src).unwrap();

        let dst = open_db();
        import_inner(&dst, &bundle).unwrap();

        assert_eq!(crate::cache::favorites::list(&dst).unwrap().len(), 1);
    }

    #[test]
    fn restore_legacy_null_connection_worklog() {
        // AK1.4: a legacy single-Jira worklog with NULL connection_id imports fine.
        let src = open_db();
        crate::cache::worklogs::record(
            &src,
            &crate::cache::worklogs::WorklogRow {
                connection_id: None,
                issue_key: None,
                started_at: 1_700_000_000,
                ended_at: 1_700_000_600,
                logged_at: 1_700_000_000,
                ..Default::default()
            },
        )
        .unwrap();
        let bundle = export_inner(&src).unwrap();

        let dst = open_db();
        import_inner(&dst, &bundle).unwrap();
        assert_eq!(
            crate::cache::worklogs::for_date_range(&dst, 0, i64::MAX)
                .unwrap()
                .len(),
            1
        );
    }
}
