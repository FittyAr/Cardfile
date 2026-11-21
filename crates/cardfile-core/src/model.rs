use serde::{Deserialize, Serialize};

use crate::{ActivityId, ApiKeyId, BlobId, CardId, Millis, RevisionId, TagId, UserId};

/// Whether an account belongs to a person or to the built-in AI assistant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserKind {
    /// A person who signs in.
    #[default]
    Human,
    /// The single assistant account. It cannot sign in; AI clients act through it on
    /// behalf of the person whose API key (or chat) they use, so every change they
    /// make is attributed to it.
    Ai,
}

impl UserKind {
    /// Stable name stored in the database (`human` or `ai`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Ai => "ai",
        }
    }

    /// Inverse of [`Self::as_str`]; anything other than `ai` is a human.
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
    /// Primary key (UUIDv7).
    pub id: UserId,
    /// Unique sign-in name.
    pub username: String,
    /// Name shown in the UI and activity history.
    pub display_name: String,
    /// Argon2id PHC string, `None` when the account has no password.
    pub password_hash: Option<String>,
    /// Whether the user may manage other accounts and installation settings.
    pub is_admin: bool,
    /// Person or the AI assistant account.
    pub kind: UserKind,
    /// Argon2id PHC string of a lock password separate from the sign-in password.
    /// `None` means private and encrypted cards are unlocked with the sign-in password.
    pub lock_hash: Option<String>,
    /// The user's X25519 public key (base64), used to share encrypted cards with them.
    pub public_key: Option<String>,
    /// The matching secret key, encrypted under the lock password.
    pub secret_key: Option<EncryptedPayload>,
    /// When the account was created.
    pub created_at: Millis,
    /// When the account row last changed.
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
    /// The account that performs the change (the AI account when an AI acts).
    pub user: UserId,
    /// The person the AI acts for; `None` when a person acts directly.
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
    /// Every mode, from least to most protective.
    pub const ALL: [LockMode; 4] = [Self::None, Self::ReadOnly, Self::Private, Self::Encrypted];

    /// Stable name stored in the database and settings.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ReadOnly => "read_only",
            Self::Private => "private",
            Self::Encrypted => "encrypted",
        }
    }

    /// Inverse of [`Self::as_str`]; `None` for unknown names.
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
    /// Argon2 time cost (iterations); 0 for version 2.
    #[serde(default)]
    pub t_cost: u32,
    /// Argon2 parallelism; 0 for version 2.
    #[serde(default)]
    pub p_cost: u32,
    /// Ciphertext (base64) of the serialized document JSON.
    pub ciphertext: String,
    /// Version 2: a public fingerprint of the card key that sealed this payload (see
    /// `cardfile_crypto::CardKey::id`), so storage can refuse a save made with a key
    /// that was rotated away. Empty for version 1 and for payloads written before key
    /// fingerprints existed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub key_id: String,
}

/// A card with its full content.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    /// Primary key (UUIDv7).
    pub id: CardId,
    /// The user who created the card and controls sharing.
    pub owner_id: UserId,
    /// Title in plain text (stored even for encrypted cards).
    pub title: String,
    /// Serialized `cardfile_doc::Document` (JSON). Empty for encrypted cards.
    pub content: String,
    /// Plain text extracted from the document, used for search and previews.
    pub plain_text: String,
    /// How the card is protected.
    pub lock_mode: LockMode,
    /// Ciphertext of the document when `lock_mode` is [`LockMode::Encrypted`].
    pub encrypted: Option<EncryptedPayload>,
    /// Pinned cards are listed before the rest.
    pub is_pinned: bool,
    /// Marked as favourite by the owner.
    pub is_favorite: bool,
    /// Fractional index for manual ordering (see [`crate::order`]).
    pub sort_key: String,
    /// When the card was created.
    pub created_at: Millis,
    /// When the card last changed.
    pub updated_at: Millis,
    /// When the card was moved to the recycle bin; `None` while it is live.
    pub deleted_at: Option<Millis>,
    /// Change counter, incremented on every update (for sync and conflict detection).
    pub rev: i64,
    /// Tags attached to the card.
    pub tags: Vec<TagId>,
    /// Who made the last change (`None` for cards older than attribution).
    pub updated_by: Option<UserId>,
}

impl Card {
    /// Whether the card is in the recycle bin.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// What a user may do with a card shared with them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharePermission {
    /// Read the card only.
    #[default]
    View,
    /// Read and edit the card. Deleting and sharing stay with the owner.
    Edit,
}

impl SharePermission {
    /// Stable name stored in the database (`view` or `edit`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Edit => "edit",
        }
    }

    /// Inverse of [`Self::as_str`]; anything other than `edit` is view-only.
    pub fn parse(s: &str) -> Self {
        if s == "edit" { Self::Edit } else { Self::View }
    }
}

/// A user's relation to a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardAccess {
    /// The user owns the card.
    Owner,
    /// Another user shares the card with this one.
    Shared(SharePermission),
}

impl CardAccess {
    /// Whether the user may change the card's content.
    pub fn can_edit(self) -> bool {
        matches!(self, Self::Owner | Self::Shared(SharePermission::Edit))
    }

    /// Whether the user owns the card.
    pub fn is_owner(self) -> bool {
        self == Self::Owner
    }
}

/// One user a card is shared with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardShare {
    /// The shared card.
    pub card_id: CardId,
    /// The user the card is shared with.
    pub user_id: UserId,
    /// What that user may do with it.
    pub permission: SharePermission,
    /// When the share was created.
    pub created_at: Millis,
}

/// An API key that lets an AI client act on behalf of its owner. Only a hash of the
/// secret is stored; the secret is shown once when the key is created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiKey {
    /// Primary key (UUIDv7).
    pub id: ApiKeyId,
    /// The person the AI acts for: it sees exactly what they see.
    pub user_id: UserId,
    /// Label the owner gave the key.
    pub name: String,
    /// First characters of the secret, to recognise the key in lists.
    pub prefix: String,
    /// Whether the AI may read and edit private cards. Encrypted cards are never
    /// available to it.
    pub allow_private: bool,
    /// Whether the AI may only read.
    pub read_only: bool,
    /// When the key was created.
    pub created_at: Millis,
    /// When an AI client last authenticated with it.
    pub last_used_at: Option<Millis>,
    /// When the key was revoked; a revoked key no longer authenticates.
    pub revoked_at: Option<Millis>,
}

/// A recorded change, used to show who (or which AI session) did what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityEntry {
    /// Primary key (UUIDv7).
    pub id: ActivityId,
    /// The card affected, if the change concerned one.
    pub card_id: Option<CardId>,
    /// The account that made the change.
    pub actor_id: UserId,
    /// The person an AI acted for, if any.
    pub on_behalf_of: Option<UserId>,
    /// How the AI got access (API key name or `chat`).
    pub via: Option<String>,
    /// Short machine-readable verb (`create`, `edit`, `delete`, `tag`, …).
    pub action: String,
    /// Human-readable detail, e.g. the card title.
    pub summary: String,
    /// When the change happened.
    pub created_at: Millis,
}

/// Lightweight card data for list views.
#[derive(Clone, Debug, PartialEq)]
pub struct CardSummary {
    /// The card's id.
    pub id: CardId,
    /// Title as stored (the UI masks private titles itself).
    pub title: String,
    /// First characters of the plain text (empty for hidden content).
    pub preview: String,
    /// How the card is protected.
    pub lock_mode: LockMode,
    /// Pinned cards are listed first.
    pub is_pinned: bool,
    /// Marked as favourite.
    pub is_favorite: bool,
    /// Fractional index for manual ordering.
    pub sort_key: String,
    /// When the card last changed.
    pub updated_at: Millis,
    /// When the card was moved to the recycle bin.
    pub deleted_at: Option<Millis>,
    /// Tags attached to the card.
    pub tags: Vec<TagId>,
    /// The card's owner.
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
    /// Title in plain text.
    pub title: String,
    /// Serialized `cardfile_doc::Document` (JSON).
    pub content: String,
    /// Plain text extracted from the document, for search and previews.
    pub plain_text: String,
    /// Initial protection of the card.
    pub lock_mode: LockMode,
}

/// A user-defined label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    /// Primary key (UUIDv7).
    pub id: TagId,
    /// The user the tag belongs to; tags are per user.
    pub owner_id: UserId,
    /// Display name, unique per owner.
    pub name: String,
    /// `#rrggbb`.
    pub color: String,
}

/// A snapshot of a card taken before an edit.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRevision {
    /// Primary key (UUIDv7).
    pub id: RevisionId,
    /// The card this is a snapshot of.
    pub card_id: CardId,
    /// Title at the time of the snapshot.
    pub title: String,
    /// Serialized document at the time of the snapshot (empty when encrypted).
    pub content: String,
    /// Ciphertext at the time of the snapshot, for encrypted cards.
    pub encrypted: Option<EncryptedPayload>,
    /// When the snapshot was taken.
    pub created_at: Millis,
    /// Who wrote the snapshotted version (`None` for revisions older than attribution).
    pub author_id: Option<UserId>,
}

/// Image or attachment bytes, content-addressed and deduplicated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    /// Id, type and size.
    pub meta: BlobMeta,
    /// Raw bytes (ciphertext for sealed blobs).
    pub data: Vec<u8>,
}

/// Everything about a blob except its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobMeta {
    /// Content hash (BLAKE3 hex), or a random id for sealed blobs.
    pub id: BlobId,
    /// MIME type, e.g. `image/png`.
    pub mime: String,
    /// Length of the stored bytes.
    pub size: i64,
    /// Original file name, when the blob came from a file.
    pub file_name: Option<String>,
}

/// Which fields full-text search looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchScope {
    /// Match card titles only.
    TitleOnly,
    /// Match titles and document text.
    #[default]
    TitleAndContent,
    /// Title, content and tag names.
    Everything,
}

/// Order of the card list. Pinned cards always come first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// The user's drag-and-drop order.
    #[default]
    Manual,
    /// Most recently changed first.
    UpdatedDesc,
    /// Most recently created first.
    CreatedDesc,
    /// Alphabetical by title.
    TitleAsc,
}

/// Query for the card list.
#[derive(Clone, Debug, Default)]
pub struct CardFilter {
    /// Full-text search terms; empty lists everything.
    pub query: String,
    /// Which fields `query` searches.
    pub scope: SearchScope,
    /// Only cards carrying this tag.
    pub tag: Option<TagId>,
    /// Only favourite cards.
    pub favorites_only: bool,
    /// List the recycle bin instead of live cards.
    pub deleted: bool,
    /// List cards other users share with this user instead of their own.
    pub shared_with_me: bool,
    /// Order of the results.
    pub sort: SortOrder,
}
