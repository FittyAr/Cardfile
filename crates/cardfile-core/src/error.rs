use thiserror::Error;

/// Errors produced by domain rules.
#[derive(Debug, Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("the card is locked")]
    Locked,
    #[error("wrong password")]
    WrongPassword,
    #[error("permission denied")]
    PermissionDenied,
    /// A save does not match the card's current protection (plain content for an
    /// encrypted card, or ciphertext for one that is not), e.g. because the card was
    /// encrypted or decrypted after the editor loaded it.
    #[error("the card's protection changed; reopen it before saving")]
    LockModeMismatch,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
