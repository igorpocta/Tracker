use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;
use thiserror::Error;

pub type DbPool = Pool<SqliteConnectionManager>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("migration failed: {0}")]
    Migration(String),
}

pub struct Db {
    pool: DbPool,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let manager = SqliteConnectionManager::file(path).with_init(|c| {
            c.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;",
            )
        });
        let pool = Pool::builder().max_size(8).build(manager)?;
        let db = Db { pool };
        crate::cache::migrations::run(&db)?;
        Ok(db)
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
