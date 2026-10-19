//! Domain model shared by every Cardfile crate.
//!
//! This crate is deliberately free of UI and database dependencies so the rules
//! it encodes (lock modes, settings, ordering) can be tested in isolation.

mod blob_ref;
mod error;
mod ids;
mod model;
pub mod order;
mod settings;
mod time;

pub use blob_ref::BlobRef;
pub use error::{Error, Result};
pub use ids::{ActivityId, ApiKeyId, BlobId, CardId, RevisionId, TagId, UserId};
pub use model::{
    ActivityEntry, Actor, ApiKey, Blob, BlobMeta, Card, CardAccess, CardFilter, CardRevision,
    CardShare, CardSummary, EncryptedPayload, LockMode, NewCard, REDACTED, SearchScope,
    SharePermission, SortOrder, Tag, User, UserKind,
};
pub use settings::{AppSettings, AuthMode, DEFAULT_MCP_PORT, UserSettings};
pub use time::{Millis, now_millis};

/// Mask a title for display when a card is private and still locked.
///
/// Keeps the first `visible` characters and replaces the rest with `•`.
pub fn mask_title(title: &str, visible: usize) -> String {
    let mut out = String::with_capacity(title.len());
    for (i, ch) in title.chars().enumerate() {
        if i < visible || ch.is_whitespace() {
            out.push(ch);
        } else {
            out.push('•');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_after_visible_chars() {
        assert_eq!(mask_title("Secret plan", 2), "Se•••• ••••");
        assert_eq!(mask_title("ab", 5), "ab");
        assert_eq!(mask_title("", 3), "");
    }

    #[test]
    fn user_debug_redacts_secrets() {
        let user = User {
            id: UserId::new(),
            username: "alice".into(),
            display_name: "Alice".into(),
            password_hash: Some("argon-secret-hash".into()),
            is_admin: false,
            kind: UserKind::Human,
            lock_hash: Some("argon-lock-hash".into()),
            public_key: None,
            secret_key: Some(EncryptedPayload {
                v: 2,
                salt: String::new(),
                nonce: "the-nonce".into(),
                m_cost: 0,
                t_cost: 0,
                p_cost: 0,
                ciphertext: "sealed-ciphertext".into(),
            }),
            created_at: 0,
            updated_at: 0,
        };
        let out = format!("{user:?}");
        assert!(out.contains("alice"));
        for secret in ["secret-hash", "lock-hash", "sealed-ciphertext", "the-nonce"] {
            assert!(!out.contains(secret), "{out}");
        }
    }
}
