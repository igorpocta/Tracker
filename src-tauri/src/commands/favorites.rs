//! Favorite (starred) issues — Phase 18B Item 26.
//!
//! Lets the user mark a Jira issue as a favorite so it appears at the top of
//! Recent / Quick start lists. Favorites are local to this install.

use tauri::Emitter;

use crate::cache::{self, issues::IssueRow};
use crate::state::AppState;
use crate::validation::validate_issue_key;

/// Pure logic for `list_favorites`. Joins favorites against the cached
/// `issues_v2` table by the exact `(connection_id, issue_key)` pair so two
/// tenants sharing a key never collide.
///
/// Favorites whose `connection_id` the migration could not disambiguate
/// (legacy NULL) are **skipped** — surfacing them as startable risks routing
/// to the wrong tenant, so we'd rather not show them than guess.
pub fn list_favorites_inner(db: &cache::Db) -> Result<Vec<IssueRow>, String> {
    let favs = cache::favorites::list(db).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(favs.len());
    for f in favs {
        let Some(cid) = f.connection_id else {
            // Ambiguous legacy favorite — do not surface.
            continue;
        };
        match cache::issues::get_by_conn_key(db, cid, &f.issue_key).map_err(|e| e.to_string())? {
            Some(row) => out.push(row),
            // Favorite for a real connection, but the issue isn't cached yet
            // (e.g. before first sync). Still startable — carry the key +
            // connection so the UI can route correctly.
            None => out.push(IssueRow {
                connection_id: cid,
                issue_key: f.issue_key,
                ..Default::default()
            }),
        }
    }
    Ok(out)
}

pub fn add_favorite_inner(
    db: &cache::Db,
    issue_key: &str,
    connection_id: Option<i64>,
) -> Result<(), String> {
    validate_issue_key(issue_key)?;
    let key = issue_key.trim();
    cache::favorites::add(db, key, connection_id).map_err(|e| e.to_string())
}

pub fn remove_favorite_inner(
    db: &cache::Db,
    issue_key: &str,
    connection_id: Option<i64>,
) -> Result<(), String> {
    validate_issue_key(issue_key)?;
    let key = issue_key.trim();
    cache::favorites::remove(db, key, connection_id).map_err(|e| e.to_string())
}

pub fn is_favorite_inner(
    db: &cache::Db,
    issue_key: &str,
    connection_id: Option<i64>,
) -> Result<bool, String> {
    cache::favorites::is_favorite(db, issue_key.trim(), connection_id).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
// Tauri commands.
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn list_favorites(state: tauri::State<'_, AppState>) -> Result<Vec<IssueRow>, String> {
    list_favorites_inner(&state.db)
}

#[tauri::command]
pub async fn add_favorite(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
    connection_id: Option<i64>,
) -> Result<(), String> {
    add_favorite_inner(&state.db, &issue_key, connection_id)?;
    let _ = app.emit("favorites-changed", &issue_key);
    Ok(())
}

#[tauri::command]
pub async fn remove_favorite(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    issue_key: String,
    connection_id: Option<i64>,
) -> Result<(), String> {
    remove_favorite_inner(&state.db, &issue_key, connection_id)?;
    let _ = app.emit("favorites-changed", &issue_key);
    Ok(())
}

#[tauri::command]
pub async fn is_favorite(
    state: tauri::State<'_, AppState>,
    issue_key: String,
    connection_id: Option<i64>,
) -> Result<bool, String> {
    is_favorite_inner(&state.db, &issue_key, connection_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Db;
    use tempfile::tempdir;

    fn open_db() -> Db {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        Db::open(&path).unwrap()
    }

    fn seed_conn(db: &Db) -> i64 {
        cache::connections::insert(
            db,
            cache::connections::NewConnection {
                provider: "jira",
                name: "Tenant A",
                enabled: true,
                config_json: "{}",
            },
        )
        .unwrap()
    }

    #[test]
    fn list_surfaces_uncached_key_with_connection() {
        let db = open_db();
        let cid = seed_conn(&db);
        add_favorite_inner(&db, "ACME-1", Some(cid)).unwrap();
        let rows = list_favorites_inner(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].issue_key, "ACME-1");
        assert_eq!(rows[0].connection_id, cid);
        // name is empty because the issue isn't cached.
        assert_eq!(rows[0].name, "");
    }

    #[test]
    fn list_skips_ambiguous_legacy_favorite() {
        // AK4: a favorite with no resolvable connection is NOT surfaced as
        // startable (rather than risk the wrong tenant).
        let db = open_db();
        add_favorite_inner(&db, "ACME-1", None).unwrap();
        assert!(list_favorites_inner(&db).unwrap().is_empty());
    }

    #[test]
    fn empty_key_is_rejected() {
        let db = open_db();
        assert!(add_favorite_inner(&db, "", None).is_err());
        assert!(add_favorite_inner(&db, "  ", None).is_err());
    }

    #[test]
    fn invalid_key_format_is_rejected() {
        let db = open_db();
        assert!(add_favorite_inner(&db, "acme-1", None).is_err());
        assert!(add_favorite_inner(&db, "ACME-01", None).is_err());
        assert!(add_favorite_inner(&db, "ACME 1", None).is_err());
    }

    #[test]
    fn is_favorite_round_trips() {
        let db = open_db();
        let cid = seed_conn(&db);
        assert!(!is_favorite_inner(&db, "ACME-2", Some(cid)).unwrap());
        add_favorite_inner(&db, "ACME-2", Some(cid)).unwrap();
        assert!(is_favorite_inner(&db, "ACME-2", Some(cid)).unwrap());
        remove_favorite_inner(&db, "ACME-2", Some(cid)).unwrap();
        assert!(!is_favorite_inner(&db, "ACME-2", Some(cid)).unwrap());
    }
}
