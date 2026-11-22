use thiserror::Error;

/// Errors produced by domain rules.
#[derive(Debug, Error)]
pub enum Error {
    /// A requested record does not exist (or is not visible to the caller); the
    /// string names what was looked up.
    #[error("not found: {0}")]
    NotFound(String),
    /// Input broke a domain rule; the string explains which.
    #[error("invalid input: {0}")]
    Invalid(String),
    /// The card is locked (read-only, private or encrypted) and must be unlocked first.
    #[error("the card is locked")]
    Locked,
    /// The password given to unlock or sign in did not match.
    #[error("wrong password")]
    WrongPassword,
    /// The actor may not perform this operation (not the owner, view-only share,
    /// read-only API key, not an admin, …).
    #[error("permission denied")]
    PermissionDenied,
    /// A save does not match the card's current protection (plain content for an
    /// encrypted card, or ciphertext for one that is not), e.g. because the card was
    /// encrypted or decrypted after the editor loaded it.
    #[error("the card's protection changed; reopen it before saving")]
    LockModeMismatch,
}

/// Result type of domain operations, defaulting to [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
