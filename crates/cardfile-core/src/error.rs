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
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
