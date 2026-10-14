//! Application-wide state shared by every view.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
        run_db(cx, move |_| db.save_user_settings(id, &settings)).detach_and_log_err(cx);
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
