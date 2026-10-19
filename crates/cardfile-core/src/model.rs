use serde::{Deserialize, Serialize};

use crate::{ActivityId, ApiKeyId, BlobId, CardId, Millis, RevisionId, TagId, UserId};

/// Whether an account belongs to a person or to the built-in AI assistant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserKind {
    #[default]
    Human,
    /// The single assistant account. It cannot sign in; AI clients act through it on
    /// behalf of the person whose API key (or chat) they use, so every change they
    /// make is attributed to it.
    Ai,
}

impl UserKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Ai => "ai",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "ai" { Self::Ai } else { Self::Human }
    }
}

/// A user account. In single-user and password-free modes there is exactly one person.
///
/// `Debug` redacts the password hashes and the sealed secret key so they never reach
/// logs.
#[derive(Clone, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    /// Argon2id PHC string, `None` when the account has no password.
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub kind: UserKind,
    /// Argon2id PHC string of a lock password separate from the sign-in password.
    /// `None` means private and encrypted cards are unlocked with the sign-in password.
    pub lock_hash: Option<String>,
    /// The user's X25519 public key (base64), used to share encrypted cards with them.
    pub public_key: Option<String>,
    /// The matching secret key, encrypted under the lock password.
    pub secret_key: Option<EncryptedPayload>,
    pub created_at: Millis,
    pub updated_at: Millis,
}

/// Placeholder printed instead of a secret by the hand-written `Debug` impls.
pub const REDACTED: &str = "<redacted>";

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redact = |set: bool| if set { Some(REDACTED) } else { None };
        f.debug_struct("User")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("display_name", &self.display_name)
            .field("password_hash", &redact(self.password_hash.is_some()))
            .field("is_admin", &self.is_admin)
            .field("kind", &self.kind)
            .field("lock_hash", &redact(self.lock_hash.is_some()))
            .field("public_key", &self.public_key)
            .field("secret_key", &redact(self.secret_key.is_some()))
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl User {
    /// Hash of the password that unlocks this user's private and encrypted cards.
    pub fn effective_lock_hash(&self) -> Option<&str> {
        self.lock_hash.as_deref().or(self.password_hash.as_deref())
    }

    /// Whether the user can receive shared encrypted cards.
    pub fn has_keys(&self) -> bool {
        self.public_key.is_some() && self.secret_key.is_some()
    }
}

/// Who performs a change. People act as themselves; the AI account acts on behalf of
/// the person who allowed it (through an API key or the in-app chat).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub user: UserId,
    pub on_behalf_of: Option<UserId>,
    /// How the AI got access: the API key name, or `chat`.
    pub via: Option<String>,
}

impl Actor {
    /// A person acting directly.
    pub fn user(id: UserId) -> Self {
        Self {
            user: id,
            on_behalf_of: None,
            via: None,
        }
    }

    /// The AI account acting for `principal`.
    pub fn ai(ai: UserId, principal: UserId, via: impl Into<String>) -> Self {
        Self {
            user: ai,
            on_behalf_of: Some(principal),
            via: Some(via.into()),
        }
    }

    /// The person whose data is being touched.
    pub fn principal(&self) -> UserId {
        self.on_behalf_of.unwrap_or(self.user)
    }
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
///
/// Version 1 derives the key from a password (salt and Argon2 costs set). Version 2
/// uses a random per-card key wrapped for each user with access (see
/// `cardfile_crypto`); salt and costs are then empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Format version of the envelope.
    pub v: u32,
    /// Argon2id salt (base64). Empty for version 2.
    #[serde(default)]
    pub salt: String,
    /// XChaCha20-Poly1305 nonce (base64).
    pub nonce: String,
    /// Argon2 parameters (0 for version 2).
    #[serde(default)]
    pub m_cost: u32,
    #[serde(default)]
    pub t_cost: u32,
    #[serde(default)]
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
    /// Who made the last change (`None` for cards older than attribution).
    pub updated_by: Option<UserId>,
}

impl Card {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// What a user may do with a card shared with them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharePermission {
    #[default]
    View,
    Edit,
}

impl SharePermission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Edit => "edit",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "edit" { Self::Edit } else { Self::View }
    }
}

/// A user's relation to a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardAccess {
    Owner,
    Shared(SharePermission),
}

impl CardAccess {
    pub fn can_edit(self) -> bool {
        matches!(self, Self::Owner | Self::Shared(SharePermission::Edit))
    }

    pub fn is_owner(self) -> bool {
        self == Self::Owner
    }
}

/// One user a card is shared with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardShare {
    pub card_id: CardId,
    pub user_id: UserId,
    pub permission: SharePermission,
    pub created_at: Millis,
}

/// An API key that lets an AI client act on behalf of its owner. Only a hash of the
/// secret is stored; the secret is shown once when the key is created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiKey {
    pub id: ApiKeyId,
    /// The person the AI acts for: it sees exactly what they see.
    pub user_id: UserId,
    pub name: String,
    /// First characters of the secret, to recognise the key in lists.
    pub prefix: String,
    /// Whether the AI may read and edit private cards. Encrypted cards are never
    /// available to it.
    pub allow_private: bool,
    /// Whether the AI may only read.
    pub read_only: bool,
    pub created_at: Millis,
    pub last_used_at: Option<Millis>,
    pub revoked_at: Option<Millis>,
}

/// A recorded change, used to show who (or which AI session) did what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityEntry {
    pub id: ActivityId,
    pub card_id: Option<CardId>,
    pub actor_id: UserId,
    pub on_behalf_of: Option<UserId>,
    pub via: Option<String>,
    /// Short machine-readable verb (`create`, `edit`, `delete`, `tag`, …).
    pub action: String,
    /// Human-readable detail, e.g. the card title.
    pub summary: String,
    pub created_at: Millis,
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
    pub owner_id: UserId,
    /// `Shared(..)` when the card belongs to someone else.
    pub access: CardAccess,
    /// Whether the owner shares the card with anyone.
    pub is_shared: bool,
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
    /// Who wrote the snapshotted version (`None` for revisions older than attribution).
    pub author_id: Option<UserId>,
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
    /// List cards other users share with this user instead of their own.
    pub shared_with_me: bool,
    pub sort: SortOrder,
}
