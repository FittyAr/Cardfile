//! Application-wide state shared by every view.

use std::sync::Arc;

use anyhow::Result;
use cardfile_core::{AppSettings, AuthMode, User, UserSettings};
use cardfile_db::Database;
use cardfile_editor::BlobStore;
use gpui_kit::{App, AppContext as _, Global, Task};
use zeroize::Zeroizing;

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
    /// Password used to unlock private/encrypted cards in this session, kept only
    /// while a protected card is open.
    pub card_password: Option<Zeroizing<String>>,
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

    /// Hash of the password that protects private and encrypted cards.
    pub fn lock_password_hash(cx: &App) -> Option<String> {
        let s = Self::session(cx)?;
        match Self::get(cx).app.auth_mode {
            AuthMode::NoPassword => s.settings.lock_password_hash.clone(),
            _ => s.user.password_hash.clone(),
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

/// Run blocking database work on the background executor.
pub fn run_db<T: Send + 'static>(
    cx: &App,
    f: impl FnOnce(()) -> cardfile_db::DbResult<T> + Send + 'static,
) -> Task<Result<T>> {
    cx.background_spawn(async move { f(()).map_err(anyhow::Error::from) })
}

/// [`BlobStore`] backed by the database, for the editor.
pub struct DbBlobs(pub Database);

impl BlobStore for DbBlobs {
    fn put(&self, data: &[u8], mime: &str, file_name: Option<&str>) -> Option<String> {
        self.0
            .put_blob(data, mime, file_name)
            .map(|m| m.id.0)
            .map_err(|e| tracing::error!("store blob: {e}"))
            .ok()
    }

    fn get(&self, id: &str) -> Option<(String, Vec<u8>)> {
        self.0
            .blob(id)
            .ok()
            .flatten()
            .map(|b| (b.meta.mime, b.data))
    }
}

impl cardfile_doc::BlobResolver for DbBlobs {
    fn resolve(&self, blob_id: &str) -> Option<(String, Vec<u8>)> {
        BlobStore::get(self, blob_id)
    }
}

pub fn blob_store(cx: &App) -> Arc<DbBlobs> {
    Arc::new(DbBlobs(AppState::db(cx)))
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
