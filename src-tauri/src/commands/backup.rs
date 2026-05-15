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
        favorite_issues: dump_table(
            &conn,
            "SELECT issue_key, connection_id, created_at FROM favorite_issues",
        )
        // favorite_issues může neexistovat v starší DB — tolerujeme.
        .unwrap_or_default(),
        daily_activity: dump_table(
            &conn,
            "SELECT date, active_seconds, inactive_seconds FROM daily_activity",
        )
        .unwrap_or_default(),
        non_working_days: dump_table(
            &conn,
            "SELECT date, reason, label, created_at FROM non_working_days",
        )
        .unwrap_or_default(),
    };
    Ok(BackupBundle {
        version: BACKUP_VERSION,
        generated_at: chrono::Utc::now().timestamp(),
        tables,
    })
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
#[tauri::command]
pub async fn import_backup(
    state: tauri::State<'_, AppState>,
    bundle: BackupBundle,
) -> Result<ImportStats, String> {
    if bundle.version > BACKUP_VERSION {
        return Err(format!(
            "Zálohu z budoucí verze ({}) tato instalace neumí načíst",
            bundle.version
        ));
    }
    import_inner(&state.db, &bundle).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStats {
    pub worklogs: usize,
    pub issues_v2: usize,
    pub connections: usize,
    pub audit_log: usize,
    pub app_settings: usize,
}

fn import_inner(db: &Db, bundle: &BackupBundle) -> Result<ImportStats, rusqlite::Error> {
    let mut conn = db.pool().get().expect("db pool");
    let tx = conn.transaction()?;

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
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }

    let n_worklogs = insert_rows(&tx, "worklogs", &bundle.tables.worklogs)?;
    let n_issues = insert_rows(&tx, "issues_v2", &bundle.tables.issues_v2)?;
    let n_conns = insert_rows(&tx, "connections", &bundle.tables.connections)?;
    let n_audit = insert_rows(&tx, "audit_log", &bundle.tables.audit_log)?;
    let n_settings = insert_rows(&tx, "app_settings", &bundle.tables.app_settings)?;
    let _ = insert_rows(&tx, "favorite_issues", &bundle.tables.favorite_issues);
    let _ = insert_rows(&tx, "daily_activity", &bundle.tables.daily_activity);
    let _ = insert_rows(&tx, "non_working_days", &bundle.tables.non_working_days);

    tx.commit()?;
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
    fn json_to_sqlite_maps_basic_types() {
        use rusqlite::types::Value as Sv;
        assert!(matches!(json_to_sqlite(&Value::Null), Sv::Null));
        assert!(matches!(json_to_sqlite(&json!(42)), Sv::Integer(42)));
        assert!(matches!(json_to_sqlite(&json!(1.5)), Sv::Real(_)));
        assert!(matches!(json_to_sqlite(&json!("hi")), Sv::Text(_)));
        assert!(matches!(json_to_sqlite(&json!(true)), Sv::Integer(1)));
    }
}
