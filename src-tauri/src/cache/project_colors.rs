//! Per-project color overrides — viz migrace 0014.

use super::db::{Db, DbError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectColor {
    pub project_key: String,
    pub color: String,
    pub updated_at: i64,
}

pub fn list(db: &Db) -> Result<Vec<ProjectColor>, DbError> {
    let conn = db.pool().get()?;
    let mut stmt = conn.prepare(
        "SELECT project_key, color, updated_at FROM project_colors
         ORDER BY project_key",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ProjectColor {
            project_key: r.get(0)?,
            color: r.get(1)?,
            updated_at: r.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn set(db: &Db, project_key: &str, color: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "INSERT INTO project_colors (project_key, color, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(project_key) DO UPDATE SET color = ?2, updated_at = ?3",
        rusqlite::params![project_key, color, now],
    )?;
    Ok(())
}

pub fn remove(db: &Db, project_key: &str) -> Result<(), DbError> {
    let conn = db.pool().get()?;
    conn.execute(
        "DELETE FROM project_colors WHERE project_key = ?1",
        [project_key],
    )?;
    Ok(())
}
