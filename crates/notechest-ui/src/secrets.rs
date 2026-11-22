//! API keys of AI services, kept in the operating system's credential store
//! (Windows Credential Manager, the Secret Service on Linux) instead of the database.
//!
//! The keys are loaded once after sign-in on a background thread and cached in
//! memory. If the credential store is unavailable (e.g. a Linux session without a
//! Secret Service), keys stay in the user settings as before, and Settings says so.

use std::collections::HashMap;

use gpui_kit::{App, Global};
use notechest_core::UserId;
use zeroize::Zeroizing;

const SERVICE: &str = "NoteChest";
/// Credential-store service name before the project was renamed (2.0.0). Keys saved
/// under it are moved to [`SERVICE`] the first time they are read.
const LEGACY_SERVICE: &str = "Cardfile";

/// In-memory copy of the current user's AI keys, by provider id.
#[derive(Default)]
pub struct AiKeys {
    keys: HashMap<String, Zeroizing<String>>,
    /// Whether the OS credential store works.
    pub store_ok: bool,
}

impl Global for AiKeys {}

fn account(user: UserId, provider: &str) -> String {
    format!("ai:{user}:{provider}")
}

/// Whether the credential store is usable (blocking).
pub fn store_available() -> bool {
    keyring::Entry::store_status().is_ok()
}

fn read_from(service: &str, user: UserId, provider: &str) -> Option<Zeroizing<String>> {
    keyring::Entry::new(service, &account(user, provider))
        .and_then(|e| e.get_password())
        .map(Zeroizing::new)
        .ok()
}

fn write_to(
    service: &str,
    user: UserId,
    provider: &str,
    key: Option<&str>,
) -> Result<(), keyring::Error> {
    let entry = keyring::Entry::new(service, &account(user, provider))?;
    match key {
        Some(k) => entry.set_password(k),
        None => match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            other => other,
        },
    }
}

/// Read a key from the credential store (blocking).
///
/// A key saved before the rename (under the service name `Cardfile`) is moved: it is
/// stored under the new name, checked by reading it back, and only then removed from
/// the old one.
pub fn read(user: UserId, provider: &str) -> Option<Zeroizing<String>> {
    if let Some(key) = read_from(SERVICE, user, provider) {
        return Some(key);
    }
    let key = read_from(LEGACY_SERVICE, user, provider)?;
    let moved = write_to(SERVICE, user, provider, Some(key.as_str())).is_ok()
        && read_from(SERVICE, user, provider).is_some_and(|k| *k == *key);
    if moved {
        let _ = write_to(LEGACY_SERVICE, user, provider, None);
    }
    Some(key)
}

/// Write (or with `None`, delete) a key in the credential store (blocking).
///
/// The entry of the old service name is removed too, so that a deleted key cannot come
/// back through the fallback in [`read`].
pub fn write(user: UserId, provider: &str, key: Option<&str>) -> Result<(), keyring::Error> {
    let result = write_to(SERVICE, user, provider, key);
    let _ = write_to(LEGACY_SERVICE, user, provider, None);
    result
}

impl AiKeys {
    /// A copy of `provider`'s key, wiped from memory when dropped.
    pub fn get(cx: &App, provider: &str) -> Option<Zeroizing<String>> {
        cx.try_global::<Self>()?.keys.get(provider).cloned()
    }

    /// Whether a non-blank key is cached for `provider` (no copy is made).
    pub fn has(cx: &App, provider: &str) -> bool {
        cx.try_global::<Self>()
            .and_then(|k| k.keys.get(provider))
            .is_some_and(|k| !k.trim().is_empty())
    }

    /// Whether the OS credential store works.
    pub fn store_ok(cx: &App) -> bool {
        cx.try_global::<Self>().is_some_and(|k| k.store_ok)
    }

    /// Cache `provider`'s key (`None` forgets it). Does not touch the store.
    pub fn set(cx: &mut App, provider: &str, key: Option<String>) {
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        let keys = &mut cx.global_mut::<Self>().keys;
        match key {
            Some(k) => {
                keys.insert(provider.to_owned(), Zeroizing::new(k));
            }
            None => {
                keys.remove(provider);
            }
        }
    }

    /// Replace all cached keys (after loading them from the store).
    pub fn replace(cx: &mut App, keys: HashMap<String, Zeroizing<String>>, store_ok: bool) {
        cx.set_global(Self { keys, store_ok });
    }
}
