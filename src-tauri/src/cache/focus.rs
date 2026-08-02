//! Persistence for Focus mode rules (`focus_rules` table, migration 0018).
//!
//! Rules are tiny and read on every engine tick, so everything here is a
//! straight query — no caching layer. The engine keeps its own in-memory
//! snapshot and refreshes it when the rule generation changes.

use super::db::{Db, DbError};

/// One persisted rule. `action` is only meaningful for `kind == "app"`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusRuleRow {
    pub id: i64,
    /// `"app"` or `"site"`.
    pub kind: String,
    /// `"block"` or `"allow"`.
    pub mode: String,
    pub pattern: String,
    pub label: Option<String>,
    /// `"hide"` or `"kill"`.
    pub action: String,
    pub enabled: bool,
    pub created_at: i64,
}

/// Fields accepted when creating a rule. `id` and `created_at` are assigned
/// by the store.
#[derive(Debug, Clone)]
pub struct NewFocusRule<'a> {
    pub kind: &'a str,
    pub mode: &'a str,
    pub pattern: &'a str,
    pub label: Option<&'a str>,
    pub action: &'a str,
    pub enabled: bool,
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<FocusRuleRow> {
    Ok(FocusRuleRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        mode: r.get(2)?,
        pattern: r.get(3)?,
        label: r.get(4)?,
        action: r.get(5)?,
        enabled: r.get::<_, i64>(6)? != 0,
        created_at: r.get(7)?,
    })
}

const SELECT_COLUMNS: &str =
    "id, kind, mode, pattern, label, action, enabled, created_at FROM focus_rules";

/// All rules, newest kind-grouped first so the UI renders a stable order.
pub fn list(db: &Db) -> Result<Vec<FocusRuleRow>, DbError> {
    let conn = db.pool().get()?;
    let sql = format!("SELECT {SELECT_COLUMNS} ORDER BY kind, mode, pattern");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Only the rules the engine should act on — `enabled = 1`.
pub fn list_enabled(db: &Db) -> Result<Vec<FocusRuleRow>, DbError> {
    let conn = db.pool().get()?;
    let sql = format!("SELECT {SELECT_COLUMNS} WHERE enabled = 1 ORDER BY kind, mode, pattern");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Insert a rule, or update the existing row with the same
/// `(kind, mode, pattern)`. Returns the row id either way.
pub fn upsert(db: &Db, rule: NewFocusRule<'_>, now: i64) -> Result<i64, DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT INTO focus_rules (kind, mode, pattern, label, action, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (kind, mode, pattern) DO UPDATE SET
             label = excluded.label,
             action = excluded.action,
             enabled = excluded.enabled",
        rusqlite::params![
            rule.kind,
            rule.mode,
            rule.pattern,
            rule.label,
            rule.action,
            i64::from(rule.enabled),
            now,
        ],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM focus_rules WHERE kind = ?1 AND mode = ?2 AND pattern = ?3",
        rusqlite::params![rule.kind, rule.mode, rule.pattern],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Flip a rule's enabled flag without touching anything else.
pub fn set_enabled(db: &Db, id: i64, enabled: bool) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE focus_rules SET enabled = ?2 WHERE id = ?1",
        rusqlite::params![id, i64::from(enabled)],
    )?;
    Ok(())
}

/// Change the enforcement action of an app rule.
pub fn set_action(db: &Db, id: i64, action: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "UPDATE focus_rules SET action = ?2 WHERE id = ?1",
        rusqlite::params![id, action],
    )?;
    Ok(())
}

pub fn delete(db: &Db, id: i64) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM focus_rules WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("focus.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    fn rule<'a>(kind: &'a str, mode: &'a str, pattern: &'a str) -> NewFocusRule<'a> {
        NewFocusRule {
            kind,
            mode,
            pattern,
            label: None,
            action: "hide",
            enabled: true,
        }
    }

    #[test]
    fn upsert_replaces_instead_of_duplicating() {
        let db = open_db();
        let first = upsert(&db, rule("app", "block", "com.slack.Slack"), 100).unwrap();
        let second = upsert(
            &db,
            NewFocusRule {
                label: Some("Slack"),
                action: "kill",
                ..rule("app", "block", "com.slack.Slack")
            },
            200,
        )
        .unwrap();
        assert_eq!(first, second, "same pattern must reuse the row");

        let rows = list(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_deref(), Some("Slack"));
        assert_eq!(rows[0].action, "kill");
    }

    #[test]
    fn list_enabled_skips_disabled_rows() {
        let db = open_db();
        let id = upsert(&db, rule("site", "block", "reddit.com"), 100).unwrap();
        upsert(&db, rule("site", "block", "x.com"), 100).unwrap();
        set_enabled(&db, id, false).unwrap();

        let enabled = list_enabled(&db).unwrap();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].pattern, "x.com");
        assert_eq!(list(&db).unwrap().len(), 2);
    }

    #[test]
    fn delete_removes_the_row() {
        let db = open_db();
        let id = upsert(&db, rule("app", "allow", "com.apple.Terminal"), 100).unwrap();
        delete(&db, id).unwrap();
        assert!(list(&db).unwrap().is_empty());
    }
}
