//! Application-wide state shared by every view.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use anyhow::Result;
use cardfile_core::{
    Actor, AppSettings, BlobRef, Card, CardAccess, CardId, User, UserId, UserSettings,
};
use cardfile_db::Database;
use cardfile_editor::BlobStore;
use gpui_kit::{App, AppContext as _, Global, Task};

use crate::paths::DataPaths;

/// The signed-in user and their preferences.
#[derive(Clone, Debug)]
pub struct Session {
    pub user: User,
    pub settings: UserSettings,
}

pub struct AppState {
    pub paths: DataPaths,
    pub db: Option<Database>,
    pub app: AppSettings,
    pub session: Option<Session>,
}

impl Global for AppState {}

impl AppState {
    pub fn get(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn get_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn db(cx: &App) -> Database {
        // Views that need the database only exist after it was opened.
        Self::get(cx).db.clone().unwrap_or_else(|| {
            tracing::error!("database requested before it was opened");
            Database::open_in_memory().unwrap_or_else(|e| panic!("in-memory database: {e}"))
        })
    }

    pub fn session(cx: &App) -> Option<&Session> {
        Self::get(cx).session.as_ref()
    }

    pub fn user_settings(cx: &App) -> UserSettings {
        Self::session(cx)
            .map(|s| s.settings.clone())
            .unwrap_or_default()
    }

    /// Hash of the password that protects the user's private and encrypted cards.
    pub fn lock_password_hash(cx: &App) -> Option<String> {
        Self::session(cx)?
            .user
            .effective_lock_hash()
            .map(str::to_owned)
    }

    /// The signed-in person, as the author of changes.
    pub fn actor(cx: &App) -> Option<Actor> {
        Self::session(cx).map(|s| Actor::user(s.user.id))
    }

    /// Replace the session's user record (e.g. after a key pair was created).
    pub fn refresh_user(cx: &mut App, user: User) {
        if let Some(s) = Self::get_mut(cx).session.as_mut()
            && s.user.id == user.id
        {
            s.user = user;
        }
    }

    /// Update the session's settings and persist them in the background.
    pub fn update_user_settings(cx: &mut App, f: impl FnOnce(&mut UserSettings)) {
        let Some(session) = Self::get_mut(cx).session.as_mut() else {
            return;
        };
        f(&mut session.settings);
        let (id, settings) = (session.user.id, session.settings.clone());
        let db = Self::db(cx);
        // Each change writes the whole settings; the writer keeps an older snapshot
        // from landing after a newer one.
        static WRITER: LazyLock<SnapshotWriter<UserId>> = LazyLock::new(SnapshotWriter::default);
        let ticket = WRITER.ticket();
        run_db(cx, move |_| {
            WRITER
                .write(id, ticket, || db.save_user_settings(id, &settings))
                .unwrap_or(Ok(()))
        })
        .detach_and_log_err(cx);
    }

    /// Persist the installation settings ([`Self::app`]) in the background, in order.
    pub fn save_app_settings(cx: &App) {
        static WRITER: LazyLock<SnapshotWriter<()>> = LazyLock::new(SnapshotWriter::default);
        let app = Self::get(cx).app.clone();
        let db = Self::db(cx);
        let ticket = WRITER.ticket();
        run_db(cx, move |_| {
            WRITER
                .write((), ticket, || db.save_app_settings(&app))
                .unwrap_or(Ok(()))
        })
        .detach_and_log_err(cx);
    }
}

/// Orders background writes of whole snapshots (settings, a card's content): writes
/// run one at a time, and a snapshot is skipped when a newer one of the same key was
/// already written, so an older state can never overwrite a newer one.
pub struct SnapshotWriter<K> {
    next: AtomicU64,
    written: parking_lot::Mutex<HashMap<K, u64>>,
}

impl<K> Default for SnapshotWriter<K> {
    fn default() -> Self {
        Self {
            next: AtomicU64::new(0),
            written: parking_lot::Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash> SnapshotWriter<K> {
    /// A ticket for a snapshot taken now; later tickets are newer.
    pub fn ticket(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Run `write` (blocking; call it on the background executor) unless a newer
    /// snapshot of `key` was already written. `None` means it was skipped.
    pub fn write<T, E>(
        &self,
        key: K,
        ticket: u64,
        write: impl FnOnce() -> Result<T, E>,
    ) -> Option<Result<T, E>> {
        let mut written = self.written.lock();
        if written.get(&key).is_some_and(|w| *w > ticket) {
            return None;
        }
        let result = write();
        if result.is_ok() {
            written.insert(key, ticket);
        }
        Some(result)
    }
}

/// Released once some background work finished; lets another task wait for it
/// without awaiting the work's own task (which its owner awaits).
#[derive(Clone, Default)]
pub struct Latch(Arc<(parking_lot::Mutex<bool>, parking_lot::Condvar)>);

impl Latch {
    pub fn release(&self) {
        let (done, cvar) = &*self.0;
        *done.lock() = true;
        cvar.notify_all();
    }

    /// Block until the latch is released. Background executor only.
    pub fn block(&self) {
        let (done, cvar) = &*self.0;
        let mut done = done.lock();
        while !*done {
            cvar.wait(&mut done);
        }
    }
}

/// Load a card together with what `user` may do with it. Cards the user cannot see
/// are reported as not found.
pub fn load_card(
    db: &Database,
    id: CardId,
    user: UserId,
) -> cardfile_db::DbResult<(Card, CardAccess)> {
    let access = db
        .card_access(id, user)?
        .ok_or(cardfile_db::DbError::Domain(
            cardfile_core::Error::NotFound(id.to_string()),
        ))?;
    Ok((db.card(id)?, access))
}

/// Run blocking database work on the background executor.
pub fn run_db<T: Send + 'static>(
    cx: &App,
    f: impl FnOnce(()) -> cardfile_db::DbResult<T> + Send + 'static,
) -> Task<Result<T>> {
    cx.background_spawn(async move { f(()).map_err(anyhow::Error::from) })
}

/// [`BlobStore`] backed by the database, for the editor. While [`Self::set_sealing`]
/// is on (the open card is encrypted) new blobs are sealed; reads open sealed
/// references whatever the setting.
pub struct DbBlobs {
    db: Database,
    sealing: AtomicBool,
}

impl DbBlobs {
    pub fn new(db: Database) -> Self {
        Self {
            db,
            sealing: AtomicBool::new(false),
        }
    }

    pub fn set_sealing(&self, on: bool) {
        self.sealing.store(on, Ordering::Relaxed);
    }

    /// Store `data` sealed and return its reference (see [`BlobRef`]).
    pub fn put_sealed(&self, data: &[u8]) -> Result<String> {
        let id = BlobRef::new_sealed_id();
        let (key, sealed) = cardfile_crypto::seal_blob(data, id.as_bytes())?;
        self.db.put_sealed_blob(&id, &sealed)?;
        Ok(BlobRef::sealed(&id, &key))
    }

    /// Bytes of a blob reference, opening sealed blobs.
    pub fn read(&self, reference: &str) -> Result<Option<(String, Vec<u8>)>> {
        let r = BlobRef::parse(reference);
        let Some(blob) = self.db.blob(r.id)? else {
            return Ok(None);
        };
        let data = match r.key {
            Some(key) => cardfile_crypto::open_blob(key, &blob.data, r.id.as_bytes())?.to_vec(),
            None => blob.data,
        };
        Ok(Some((blob.meta.mime, data)))
    }
}

impl BlobStore for DbBlobs {
    fn put(&self, data: &[u8], mime: &str, file_name: Option<&str>) -> Option<String> {
        let stored = if self.sealing.load(Ordering::Relaxed) {
            self.put_sealed(data)
        } else {
            self.db
                .put_blob(data, mime, file_name)
                .map(|m| m.id.0)
                .map_err(anyhow::Error::from)
        };
        stored
            .map_err(|e| tracing::error!("store blob: {e:#}"))
            .ok()
    }

    fn get(&self, id: &str) -> Option<(String, Vec<u8>)> {
        self.read(id)
            .map_err(|e| tracing::warn!("read blob: {e:#}"))
            .ok()
            .flatten()
    }

    fn is_private(&self, id: &str) -> bool {
        BlobRef::parse(id).key.is_some()
    }
}

impl cardfile_doc::BlobResolver for DbBlobs {
    fn resolve(&self, blob_id: &str) -> Option<(String, Vec<u8>)> {
        BlobStore::get(self, blob_id)
    }
}

pub fn blob_store(cx: &App) -> Arc<DbBlobs> {
    Arc::new(DbBlobs::new(AppState::db(cx)))
}

pub trait TaskExt {
    fn detach_and_log_err(self, cx: &App);
}

impl<T: 'static> TaskExt for Task<Result<T>> {
    fn detach_and_log_err(self, cx: &App) {
        cx.foreground_executor()
            .spawn(async move {
                if let Err(e) = self.await {
                    tracing::error!("{e:#}");
                }
            })
            .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_snapshot_never_overwrites_a_newer_one() {
        let writer = SnapshotWriter::<u8>::default();
        let (old, new) = (writer.ticket(), writer.ticket());
        let mut stored = 0;
        assert_eq!(
            writer.write(1, new, || {
                stored = new;
                Ok::<_, ()>(())
            }),
            Some(Ok(()))
        );
        assert_eq!(
            writer.write(1, old, || {
                stored = old;
                Ok::<_, ()>(())
            }),
            None
        );
        assert_eq!(stored, new);
        // Other keys are independent.
        assert!(writer.write(2, old, || Ok::<_, ()>(())).is_some());
    }

    #[test]
    fn a_failed_write_does_not_block_older_retries() {
        let writer = SnapshotWriter::<u8>::default();
        let (old, new) = (writer.ticket(), writer.ticket());
        assert_eq!(
            writer.write(1, new, || Err::<(), _>("disk full")),
            Some(Err("disk full"))
        );
        assert!(writer.write(1, old, || Ok::<_, ()>(())).is_some());
    }

    #[test]
    fn a_latch_releases_waiters() {
        let latch = Latch::default();
        let waiter = latch.clone();
        let thread = std::thread::spawn(move || waiter.block());
        latch.release();
        thread.join().unwrap();
        latch.block();
    }
}
