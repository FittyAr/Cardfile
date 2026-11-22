//! Consistent snapshots and restart-time replacement under an exclusive process lock.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use diesel::connection::SimpleConnection;
use diesel::migration::MigrationSource;
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_migrations::MigrationHarness;

use crate::{Database, DbError, DbResult, MIGRATIONS};

/// Name of the database file.
pub const DATABASE_FILE: &str = "notechest.db";

/// Name of the database file before the project was renamed from Cardfile to
/// NoteChest (2.0.0). Existing collections are adopted under the new name on open.
pub const LEGACY_DATABASE_FILE: &str = "cardfile.db";

/// Name prefix of snapshots written before the rename; still recognised so their
/// automatic copies keep being pruned.
const LEGACY_SNAPSHOT_PREFIX: &str = "cardfile-";

/// Whether `dir` holds a collection: `notechest.db`, or the `cardfile.db` of a
/// version from before the rename (adopted the next time it is opened).
pub fn has_collection(dir: &Path) -> bool {
    dir.join(DATABASE_FILE).is_file() || dir.join(LEGACY_DATABASE_FILE).is_file()
}

/// Rename a collection written by NoteChest's predecessor (`cardfile.db`) to
/// `notechest.db`, so that opening `path` finds the user's data.
///
/// Does nothing unless `path` is named `notechest.db`, does not exist, and sits
/// next to a `cardfile.db`. The old file is first made self-contained (the
/// write-ahead log is folded into it), because SQLite pairs `-wal` and `-shm`
/// files by name and a rename of several files at once cannot be made atomic.
/// A single `rename` then moves it. The old lock is held throughout, so a still
/// running older version makes this fail with [`DbError::AlreadyOpen`] instead of
/// ending up with two processes on one collection.
pub(crate) fn adopt_legacy_file(path: &Path) -> DbResult<()> {
    if path.file_name().and_then(|n| n.to_str()) != Some(DATABASE_FILE) || path.exists() {
        return Ok(());
    }
    let legacy = path.with_file_name(LEGACY_DATABASE_FILE);
    if !legacy.is_file() {
        return Ok(());
    }
    let lock = lock_database(&legacy)?;
    {
        let mut conn = SqliteConnection::establish(&legacy.to_string_lossy())?;
        conn.batch_execute(
            "PRAGMA busy_timeout = 5000;
             PRAGMA wal_checkpoint(TRUNCATE);
             PRAGMA journal_mode = DELETE;",
        )?;
    }
    let mut wal = legacy.as_os_str().to_owned();
    wal.push("-wal");
    if fs::metadata(&wal).is_ok_and(|m| m.len() > 0) {
        return Err(DbError::Corrupt(
            "the old database still has unwritten changes; open it with the previous version first"
                .into(),
        ));
    }
    fs::rename(&legacy, path)?;
    tracing::info!(from = %legacy.display(), to = %path.display(), "adopted the database of the previous name");
    drop(lock);
    // Leftovers of the old name; a new lock file is created for `path`.
    for suffix in ["-wal", "-shm"] {
        let mut leftover = legacy.as_os_str().to_owned();
        leftover.push(suffix);
        let _ = fs::remove_file(leftover);
    }
    let _ = fs::remove_file(legacy.with_extension("lock"));
    Ok(())
}

/// Purpose of a user-created snapshot. Only automatic snapshots are pruned.
#[derive(Clone, Copy, Debug)]
pub enum BackupKind {
    /// Explicitly requested by the administrator.
    Manual,
    /// Scheduled while NoteChest is running.
    Automatic,
    /// Mandatory safety copy before replacing the collection.
    BeforeRestore,
}
impl BackupKind {
    fn name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Automatic => "automatic",
            Self::BeforeRestore => "before-restore",
        }
    }
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

pub(crate) fn lock_database(path: &Path) -> DbResult<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))?;
    // Restarts launch before the old process exits. Give its shutdown hook time
    // to flush, without ever allowing two processes to use the collection.
    for _ in 0..50 {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(std::fs::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(100))
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
    Err(DbError::AlreadyOpen)
}

fn validate(conn: &mut SqliteConnection) -> DbResult<()> {
    let checks = diesel::sql_query("SELECT integrity_check AS value FROM pragma_integrity_check")
        .load::<TextRow>(conn)?;
    if checks.len() != 1 || checks[0].value != "ok" {
        return Err(DbError::Corrupt(
            "backup failed SQLite integrity check".into(),
        ));
    }
    // Check the complete embedded migration history, not just an arbitrary table.
    let mut versions = conn.applied_migrations().map_err(DbError::Migration)?;
    let known = <diesel_migrations::EmbeddedMigrations as MigrationSource<
        diesel::sqlite::Sqlite,
    >>::migrations(&MIGRATIONS)
    .map_err(DbError::Migration)?;
    versions.sort();
    let mut known_versions: Vec<_> = known.iter().map(|m| m.name().version()).collect();
    known_versions.sort();
    if versions.is_empty() || !known_versions.starts_with(&versions) {
        return Err(DbError::Corrupt(
            "not a supported NoteChest database (or created by a newer version)".into(),
        ));
    }
    for query in [
        "SELECT id, username, password_hash, settings, is_admin FROM users LIMIT 0",
        "SELECT id, owner_id, title, content, lock_mode FROM cards LIMIT 0",
        "SELECT id, mime, size, data FROM blobs LIMIT 0",
        "SELECT key, value FROM app_settings LIMIT 0",
        "SELECT id, card_id, created_at FROM card_revisions LIMIT 0",
    ] {
        diesel::sql_query(query).execute(conn)?;
    }
    Ok(())
}

/// Validate a snapshot without changing its journal mode or applying migrations.
pub fn validate_backup(path: &Path) -> DbResult<()> {
    if !path.is_file() {
        return Err(DbError::Corrupt("backup does not exist".into()));
    }
    let mut conn = SqliteConnection::establish(&path.to_string_lossy())?;
    conn.batch_execute("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
    validate(&mut conn)
}

pub(crate) fn snapshot(conn: &mut SqliteConnection, dir: &Path, prefix: &str) -> DbResult<PathBuf> {
    fs::create_dir_all(dir)?;
    let name = format!(
        "notechest-{prefix}-{}-{}",
        notechest_core::now_millis(),
        uuid::Uuid::now_v7()
    );
    let partial = dir.join(format!("{name}.partial"));
    let target = dir.join(format!("{name}.db"));
    let quoted = partial.to_string_lossy().replace('\'', "''");
    let result = (|| {
        conn.batch_execute(&format!("VACUUM INTO '{quoted}';"))?;
        validate_backup(&partial)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&partial)?
            .sync_all()?;
        fs::rename(&partial, &target)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(partial);
    }
    result
}

impl Database {
    /// Save a verified snapshot and prune only this collection's automatic copies.
    pub fn backup_collection(
        &self,
        dir: &Path,
        collection: &str,
        kind: BackupKind,
        keep: usize,
    ) -> DbResult<PathBuf> {
        if uuid::Uuid::parse_str(collection).is_err() {
            return Err(DbError::Corrupt("invalid collection identity".into()));
        }
        self.with(|conn| {
            let target = snapshot(conn, dir, &format!("{collection}-{}", kind.name()))?;
            if matches!(kind, BackupKind::Automatic) {
                // Copies written before the rename carry the old prefix; they count
                // towards the same retention, ordered by their timestamps.
                let prefixes = [
                    format!("notechest-{collection}-automatic-"),
                    format!("{LEGACY_SNAPSHOT_PREFIX}{collection}-automatic-"),
                ];
                let mut files = fs::read_dir(dir)?
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().into_owned();
                        let stamp = prefixes.iter().find_map(|prefix| {
                            let (stamp, id) = name
                                .strip_prefix(prefix.as_str())?
                                .strip_suffix(".db")?
                                .split_once('-')?;
                            uuid::Uuid::parse_str(id).ok()?;
                            stamp.parse::<i64>().ok()
                        })?;
                        Some((stamp, e.path()))
                    })
                    .collect::<Vec<_>>();
                files.sort();
                let remove = files.len().saturating_sub(keep.max(1));
                for (_, old) in files.into_iter().take(remove) {
                    fs::remove_file(old)?;
                }
            }
            Ok(target)
        })
    }

    /// Prepare a migrated replacement and freeze this handle until process exit.
    /// The live database is replaced only on restart, after obtaining its OS lock.
    pub fn prepare_restore(&self, source: &Path, safety_dir: &Path) -> DbResult<()> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| DbError::Corrupt("in-memory database cannot be restored".into()))?;
        self.with(|conn| {
            validate_backup(source)?;
            let parent = path
                .parent()
                .ok_or_else(|| DbError::Corrupt("database has no parent".into()))?;
            let next = parent.join(".restore-next.db");
            let marker = parent.join(".restore-pending");
            if marker.exists() || next.exists() {
                return Err(DbError::RestorePending);
            }
            let safety = snapshot(conn, safety_dir, "before-restore")?;
            // VACUUM INTO also handles a selected database whose WAL is not empty.
            let prepared = (|| {
                let mut selected = SqliteConnection::establish(&source.to_string_lossy())?;
                let quoted = next.to_string_lossy().replace('\'', "''");
                selected.batch_execute(&format!("VACUUM INTO '{quoted}';"))?;
                let mut replacement = SqliteConnection::establish(&next.to_string_lossy())?;
                replacement
                    .run_pending_migrations(MIGRATIONS)
                    .map_err(DbError::Migration)?;
                validate(&mut replacement)?;
                drop(replacement);
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&next)?
                    .sync_all()?;
                let mut marker_file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&marker)?;
                use std::io::Write as _;
                let journal = serde_json::to_vec(&safety.canonicalize()?)
                    .map_err(|e| DbError::Corrupt(e.to_string()))?;
                marker_file.write_all(&journal)?;
                marker_file.sync_all()?;
                self.frozen.store(true, Ordering::SeqCst);
                Ok(())
            })();
            if prepared.is_err() {
                let _ = fs::remove_file(&next);
                let _ = fs::remove_file(&marker);
            }
            prepared
        })
    }
}

pub(crate) fn recover(path: &Path) -> DbResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| DbError::Corrupt("database has no parent".into()))?;
    let marker = parent.join(".restore-pending");
    let next = parent.join(".restore-next.db");
    if !marker.exists() {
        // A crash before journal publication must not block future restores.
        if next.exists() {
            fs::rename(
                &next,
                parent.join(format!(".restore-abandoned-{}.db", uuid::Uuid::now_v7())),
            )?;
        }
        return Ok(());
    }
    let safety = fs::read(&marker)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PathBuf>(&bytes).ok());
    let previous = parent.join(".restore-previous.db");
    if next.exists() {
        if let Err(e) = validate_backup(&next) {
            rollback(path, &marker, &next, safety.as_deref())?;
            return Err(e);
        }
        if path.exists() {
            // A previous successful restore may have left its rollback copy.
            if previous.exists() {
                fs::remove_file(&previous)?;
            }
            // The safety snapshot contains the full WAL. Keep it until verified.
            fs::rename(path, &previous)?;
        }
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
            match fs::remove_file(sidecar) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        fs::rename(&next, path)?;
    }
    if let Err(e) = validate_backup(path) {
        rollback(path, &marker, &next, safety.as_deref())?;
        return Err(e);
    }
    fs::remove_file(marker)?;
    Ok(())
}

fn rollback(path: &Path, marker: &Path, next: &Path, safety: Option<&Path>) -> DbResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| DbError::Corrupt("database has no parent".into()))?;
    if let Some(safety) = safety {
        validate_backup(safety)?;
        let rollback = parent.join(format!(".restore-rollback-{}.db", uuid::Uuid::now_v7()));
        fs::copy(safety, &rollback)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&rollback)?
            .sync_all()?;
        if path.exists() {
            fs::rename(
                path,
                parent.join(format!(".restore-rejected-{}.db", uuid::Uuid::now_v7())),
            )?;
        }
        for suffix in ["-wal", "-shm"] {
            let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
            if sidecar.exists() {
                fs::remove_file(sidecar)?;
            }
        }
        fs::rename(rollback, path)?;
    } else {
        // Legacy/incomplete journals may have no safety path. Never guess or
        // overwrite anything: clear the job only if the live DB is still valid.
        validate_backup(path)?;
    }
    if next.exists() {
        fs::rename(
            next,
            parent.join(format!(".restore-rejected-{}.db", uuid::Uuid::now_v7())),
        )?;
    }
    fs::remove_file(marker)?;
    Ok(())
}
