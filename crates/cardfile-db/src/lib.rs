//! SQLite storage for Cardfile, built on Diesel.
//!
//! [`Database`] wraps one connection behind a mutex. All methods are blocking and
//! must be called from a background thread, never the UI thread.

mod activity;
mod api_keys;
mod blobs;
mod cards;
mod error;
mod rows;
mod schema;
mod search;
mod settings;
mod shares;
mod tags;
mod users;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use parking_lot::Mutex;

pub use cards::ContentUpdate;
pub use error::{DbError, DbResult};
pub use search::fts_query;
pub use users::{CredentialUpdate, Rekey};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Handle to the Cardfile database. Cheap to clone.
#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<SqliteConnection>>,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Database")
            .field("path", &self.path)
            .finish()
    }
}

impl Database {
    /// Open (or create) the database at `path` and apply pending migrations.
    ///
    /// When migrations are pending on an existing file, a copy is saved to
    /// `<backup_dir>/cardfile-<timestamp>.db` first.
    pub fn open(path: &Path, backup_dir: Option<&Path>) -> DbResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existed = path.exists();
        let url = path.to_string_lossy().into_owned();
        let mut conn = SqliteConnection::establish(&url)?;
        configure(&mut conn)?;
        let pending = conn
            .has_pending_migration(MIGRATIONS)
            .map_err(DbError::Migration)?;
        if pending
            && existed
            && let Some(dir) = backup_dir
        {
            backup_to(&mut conn, dir)?;
        }
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(DbError::Migration)?;
        tracing::info!(path = %path.display(), "database opened");
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Some(path.to_owned()),
        })
    }

    /// A fresh in-memory database, for tests.
    pub fn open_in_memory() -> DbResult<Self> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        configure(&mut conn)?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(DbError::Migration)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            path: None,
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Run `f` with exclusive access to the connection.
    fn with<T>(&self, f: impl FnOnce(&mut SqliteConnection) -> DbResult<T>) -> DbResult<T> {
        let mut conn = self.conn.lock();
        f(&mut conn)
    }

    /// Run `f` inside a transaction.
    fn tx<T>(&self, f: impl FnOnce(&mut SqliteConnection) -> DbResult<T>) -> DbResult<T> {
        self.with(|conn| conn.transaction(f))
    }

    /// Write a consistent copy of the database into `dir` and return its path.
    pub fn backup(&self, dir: &Path) -> DbResult<PathBuf> {
        self.with(|conn| backup_to(conn, dir))
    }
}

/// Per-connection settings. `secure_delete` zeroes deleted content instead of only
/// marking it free, so plaintext replaced by encryption (or purged) cannot be read
/// back from free pages of the file.
fn configure(conn: &mut SqliteConnection) -> DbResult<()> {
    conn.batch_execute(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn backup_to(conn: &mut SqliteConnection, dir: &Path) -> DbResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let stamp = cardfile_core::now_millis();
    let target = dir.join(format!("cardfile-{stamp}.db"));
    let quoted = target.to_string_lossy().replace('\'', "''");
    conn.batch_execute(&format!("VACUUM INTO '{quoted}';"))?;
    tracing::info!(path = %target.display(), "database backup written");
    Ok(target)
}

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;
    use cardfile_core::User;

    pub fn db() -> Database {
        Database::open_in_memory().expect("in-memory db")
    }

    pub fn db_with_user() -> (Database, User) {
        let db = db();
        let user = db
            .create_user("alice", "Alice", None, true)
            .expect("create user");
        (db, user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_file_and_backs_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join("cardfile.db");
        let db = Database::open(&path, Some(&dir.path().join("backups"))).unwrap();
        db.create_user("bob", "Bob", None, true).unwrap();
        let backup = db.backup(&dir.path().join("backups")).unwrap();
        assert!(backup.exists());
        drop(db);
        // Re-open: no pending migrations, data persists.
        let db = Database::open(&path, None).unwrap();
        assert_eq!(db.list_users().unwrap().len(), 1);
    }
}
