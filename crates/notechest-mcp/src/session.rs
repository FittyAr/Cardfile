//! Legacy (`initialize`-based) sessions tracked through the `Mcp-Session-Id` header.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::version::ProtocolVersion;

/// What a session remembers between requests.
#[derive(Debug, Clone)]
pub(crate) struct SessionInfo<P> {
    /// The principal whose token initialized the session. Later requests must authenticate
    /// to the same principal, so a leaked session id alone is useless.
    pub principal: P,
    /// The legacy revision negotiated by `initialize`.
    pub version: ProtocolVersion,
}

struct Entry<P> {
    info: SessionInfo<P>,
    last_seen: Instant,
}

/// Bounded, idle-expiring session table.
pub(crate) struct SessionStore<P> {
    map: Mutex<HashMap<String, Entry<P>>>,
    max: usize,
    idle: Duration,
}

impl<P: Clone> SessionStore<P> {
    pub fn new(max: usize, idle: Duration) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            max: max.max(1),
            idle,
        }
    }

    /// The table stays consistent even if a holder panicked, so a poisoned lock is reused.
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Entry<P>>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Creates a session and returns its id, evicting the least recently used session when
    /// the table is full. `None` when the OS random source fails.
    pub fn create(&self, principal: P, version: ProtocolVersion) -> Option<String> {
        let id = new_session_id()?;
        let now = Instant::now();
        let mut map = self.lock();
        let idle = self.idle;
        map.retain(|_, e| now.duration_since(e.last_seen) < idle);
        while map.len() >= self.max {
            let oldest = map
                .iter()
                .min_by_key(|(_, e)| e.last_seen)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(key) => {
                    map.remove(&key);
                }
                None => break,
            }
        }
        map.insert(
            id.clone(),
            Entry {
                info: SessionInfo { principal, version },
                last_seen: now,
            },
        );
        Some(id)
    }

    /// Looks a session up and refreshes its idle timer. Expired sessions are removed.
    pub fn touch(&self, id: &str) -> Option<SessionInfo<P>> {
        let now = Instant::now();
        let mut map = self.lock();
        let expired = match map.get_mut(id) {
            None => return None,
            Some(entry) if now.duration_since(entry.last_seen) >= self.idle => true,
            Some(entry) => {
                entry.last_seen = now;
                return Some(entry.info.clone());
            }
        };
        if expired {
            map.remove(id);
        }
        None
    }

    /// Terminates a session. Returns whether it existed.
    pub fn remove(&self, id: &str) -> bool {
        self.lock().remove(id).is_some()
    }
}

/// 256 bits from the OS CSPRNG, hex encoded (visible ASCII as the spec requires).
fn new_session_id() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_touch_remove() {
        let store = SessionStore::new(2, Duration::from_secs(60));
        let a = store.create(1u32, ProtocolVersion::V2025_11_25).unwrap();
        assert_eq!(a.len(), 64);
        assert_eq!(store.touch(&a).unwrap().principal, 1);
        assert!(store.remove(&a));
        assert!(store.touch(&a).is_none());
        assert!(!store.remove(&a));
    }

    #[test]
    fn evicts_least_recently_used() {
        let store = SessionStore::new(2, Duration::from_secs(60));
        let a = store.create(1u32, ProtocolVersion::V2025_11_25).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        let b = store.create(2u32, ProtocolVersion::V2025_11_25).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        store.touch(&a);
        let c = store.create(3u32, ProtocolVersion::V2025_11_25).unwrap();
        assert!(store.touch(&a).is_some());
        assert!(store.touch(&b).is_none());
        assert!(store.touch(&c).is_some());
    }

    #[test]
    fn expires_idle_sessions() {
        let store = SessionStore::new(4, Duration::ZERO);
        let a = store.create(1u32, ProtocolVersion::V2025_06_18).unwrap();
        assert!(store.touch(&a).is_none());
    }
}
