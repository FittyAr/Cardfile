use thiserror::Error;

/// Errors from the storage layer.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Query(#[from] diesel::result::Error),
    #[error("cannot open database: {0}")]
    Connection(#[from] diesel::ConnectionError),
    #[error("migration failed: {0}")]
    Migration(Box<dyn std::error::Error + Send + Sync>),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt data: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Domain(#[from] cardfile_core::Error),
}

impl DbError {
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::Query(diesel::result::Error::NotFound)
                | Self::Domain(cardfile_core::Error::NotFound(_))
        )
    }

    /// A UNIQUE constraint was violated (e.g. duplicate username or tag name).
    pub fn is_unique_violation(&self) -> bool {
        matches!(
            self,
            Self::Query(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _
            ))
        )
    }
}

pub type DbResult<T> = Result<T, DbError>;
