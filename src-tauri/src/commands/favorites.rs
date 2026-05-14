//! Favorite (starred) issues — Phase 18B Item 26.
//!
//! Lets the user mark a Jira issue as a favorite so it appears at the top of
//! Recent / Quick start lists. Favorites are local to this install.

use tauri::Emitter;

use crate::cache::{self, issues::IssueRow};
use crate::state::AppState;

/// Pure logic for `list_favorites`. Joins favorite keys against the cached
/// `issues` table; rows that lack a cached record are returned with empty
/// summary fields so the UI can still surface the key.
pub fn list_favorites_inner(db: &cache::Db) -> Result<Vec<IssueRow>, String> {
    let keys = cache::favorites::list_keys(db).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        match cache::issues::get_by_key(db, &k).map_err(|e| e.to_string())? {
            Some(row) => out.push(row),
            None => out.push(IssueRow {
                issue_key: k,
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
    let key = issue_key.trim();
    if key.is_empty() {
        return Err("issue_key must not be empty".into());
    }
    cache::favorites::add(db, key, connection_id).map_err(|e| e.to_string())
}

pub fn remove_favorite_inner(db: &cache::Db, issue_key: &str) -> Result<(), String> {
    let key = issue_key.trim();
    if key.is_empty() {
        return Err("issue_key must not be empty".into());
    }
    cache::favorites::remove(db, key).map_err(|e| e.to_string())
}

pub fn is_favorite_inner(db: &cache::Db, issue_key: &str) -> Result<bool, String> {
    cache::favorites::is_favorite(db, issue_key.trim()).map_err(|e| e.to_string())
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
) -> Result<(), String> {
    remove_favorite_inner(&state.db, &issue_key)?;
    let _ = app.emit("favorites-changed", &issue_key);
    Ok(())
}

#[tauri::command]
pub async fn is_favorite(
    state: tauri::State<'_, AppState>,
    issue_key: String,
) -> Result<bool, String> {
    is_favorite_inner(&state.db, &issue_key)
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

    #[test]
    fn list_includes_uncached_keys() {
        let db = open_db();
        add_favorite_inner(&db, "ACME-1", None).unwrap();
        let rows = list_favorites_inner(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].issue_key, "ACME-1");
        // summary is empty because the issue isn't cached.
        assert_eq!(rows[0].summary, "");
    }

    #[test]
    fn empty_key_is_rejected() {
        let db = open_db();
        assert!(add_favorite_inner(&db, "", None).is_err());
        assert!(add_favorite_inner(&db, "  ", None).is_err());
    }

    #[test]
    fn is_favorite_round_trips() {
        let db = open_db();
        assert!(!is_favorite_inner(&db, "ACME-2").unwrap());
        add_favorite_inner(&db, "ACME-2", None).unwrap();
        assert!(is_favorite_inner(&db, "ACME-2").unwrap());
        remove_favorite_inner(&db, "ACME-2").unwrap();
        assert!(!is_favorite_inner(&db, "ACME-2").unwrap());
    }
}
