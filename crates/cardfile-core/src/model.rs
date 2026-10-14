use serde::{Deserialize, Serialize};

use crate::{BlobId, CardId, Millis, RevisionId, TagId, UserId};

/// A user account. In single-user and password-free modes there is exactly one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    /// Argon2id PHC string, `None` when the account has no password.
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub created_at: Millis,
    pub updated_at: Millis,
}

/// How a card is protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockMode {
    /// Freely editable.
    #[default]
    None,
    /// Visible but not editable until unlocked; guards against accidental edits.
    ReadOnly,
    /// Title masked and content hidden until the user re-enters their password.
    Private,
    /// Content encrypted at rest with a key derived from a password.
    Encrypted,
}

impl LockMode {
    pub const ALL: [LockMode; 4] = [Self::None, Self::ReadOnly, Self::Private, Self::Encrypted];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read_only",
            Self::Private => "private",
            Self::Encrypted => "encrypted",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }

    /// Whether the content is hidden until the user authenticates.
    pub fn hides_content(self) -> bool {
        matches!(self, Self::Private | Self::Encrypted)
    }
}

/// Ciphertext of an encrypted card plus the parameters needed to decrypt it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Format version of the envelope.
    pub v: u32,
    /// Argon2id salt (base64).
    pub salt: String,
    /// XChaCha20-Poly1305 nonce (base64).
    pub nonce: String,
    /// Argon2 parameters.
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    /// Ciphertext (base64) of the serialized document JSON.
    pub ciphertext: String,
}

/// A card with its full content.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: CardId,
    pub owner_id: UserId,
    pub title: String,
    /// Serialized `cardfile_doc::Document` (JSON). Empty for encrypted cards.
    pub content: String,
    /// Plain text extracted from the document, used for search and previews.
    pub plain_text: String,
    pub lock_mode: LockMode,
    pub encrypted: Option<EncryptedPayload>,
    pub is_pinned: bool,
    pub is_favorite: bool,
    /// Fractional index for manual ordering (see [`crate::order`]).
    pub sort_key: String,
    pub created_at: Millis,
    pub updated_at: Millis,
    pub deleted_at: Option<Millis>,
    pub rev: i64,
    pub tags: Vec<TagId>,
}

impl Card {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// Lightweight card data for list views.
#[derive(Clone, Debug, PartialEq)]
pub struct CardSummary {
    pub id: CardId,
    pub title: String,
    /// First characters of the plain text (empty for hidden content).
    pub preview: String,
    pub lock_mode: LockMode,
    pub is_pinned: bool,
    pub is_favorite: bool,
    pub sort_key: String,
    pub updated_at: Millis,
    pub deleted_at: Option<Millis>,
    pub tags: Vec<TagId>,
    /// Search snippet with matches wrapped in `\u{1}` … `\u{2}` markers.
    pub snippet: Option<String>,
}

/// Fields required to create a card.
#[derive(Clone, Debug, Default)]
pub struct NewCard {
    pub title: String,
    pub content: String,
    pub plain_text: String,
    pub lock_mode: LockMode,
}

/// A user-defined label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub owner_id: UserId,
    pub name: String,
    /// `#rrggbb`.
    pub color: String,
}

/// A snapshot of a card taken before an edit.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRevision {
    pub id: RevisionId,
    pub card_id: CardId,
    pub title: String,
    pub content: String,
    pub encrypted: Option<EncryptedPayload>,
    pub created_at: Millis,
}

/// Image or attachment bytes, content-addressed and deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    pub meta: BlobMeta,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobMeta {
    pub id: BlobId,
    pub mime: String,
    pub size: i64,
    pub file_name: Option<String>,
}

/// Which fields full-text search looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    TitleOnly,
    #[default]
    TitleAndContent,
    /// Title, content and tag names.
    Everything,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// The user's drag-and-drop order.
    #[default]
    Manual,
    UpdatedDesc,
    CreatedDesc,
    TitleAsc,
}

/// Query for the card list.
#[derive(Clone, Debug, Default)]
pub struct CardFilter {
    pub query: String,
    pub scope: SearchScope,
    pub tag: Option<TagId>,
    pub favorites_only: bool,
    /// List the recycle bin instead of live cards.
    pub deleted: bool,
    pub sort: SortOrder,
}
