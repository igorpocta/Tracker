use super::db::{Db, DbError};

pub fn get(db: &Db, key: &str) -> Result<Option<String>, DbError> {
    let conn = db.pool().get()?;
    match conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set(db: &Db, key: &str, value: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// Remove a single setting by key. No-op if key doesn't exist.
pub fn remove(db: &Db, key: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute("DELETE FROM app_settings WHERE key = ?1", [key])?;
    Ok(())
}

/// Return all settings whose key starts with `prefix`. Used for grouped
/// settings like `last_sync_error:<connection_id>`.
pub fn list_with_prefix(db: &Db, prefix: &str) -> Result<Vec<(String, String)>, DbError> {
    let conn = db.pool().get()?;
    let pattern = format!("{prefix}%");
    let mut stmt =
        conn.prepare("SELECT key, value FROM app_settings WHERE key LIKE ?1 ORDER BY key")?;
    let mapped = stmt.query_map([pattern], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
