//! Domain model shared by every Cardfile crate.
//!
//! This crate is deliberately free of UI and database dependencies so the rules
//! it encodes (lock modes, settings, ordering) can be tested in isolation.

mod error;
mod ids;
mod model;
pub mod order;
mod settings;
mod time;

pub use error::{Error, Result};
pub use ids::{BlobId, CardId, RevisionId, TagId, UserId};
pub use model::{
    Blob, BlobMeta, Card, CardFilter, CardRevision, CardSummary, EncryptedPayload, LockMode,
    NewCard, SearchScope, SortOrder, Tag, User,
};
pub use settings::{AppSettings, AuthMode, UserSettings};
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
}
