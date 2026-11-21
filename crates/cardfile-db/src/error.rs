use thiserror::Error;

/// Errors from the storage layer.
#[derive(Debug, Error)]
pub enum DbError {
    /// Another local process holds the collection's operating-system lock.
    #[error("this collection is already open in another Cardfile process")]
    AlreadyOpen,
    /// A prepared restoration prevents further writes until restart.
    #[error("a collection restore is pending; restart Cardfile")]
    RestorePending,
    /// A Diesel query failed (including `NotFound` for a missing row).
    #[error("database error: {0}")]
    Query(#[from] diesel::result::Error),
    /// The SQLite file could not be opened.
    #[error("cannot open database: {0}")]
    Connection(#[from] diesel::ConnectionError),
    /// Applying the embedded migrations failed.
    #[error("migration failed: {0}")]
    Migration(Box<dyn std::error::Error + Send + Sync>),
    /// A file system operation (backup, restore, lock file) failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// Stored data could not be parsed (bad UUID, JSON, payload); the string
    /// says what.
    #[error("corrupt data: {0}")]
    Corrupt(String),
    /// A domain rule was broken (permissions, locks, invalid input).
    #[error(transparent)]
    Domain(#[from] cardfile_core::Error),
}

impl DbError {
    /// Whether the error means a record does not exist, whether Diesel or a
    /// domain check reported it.
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

/// Result type of the storage layer.
pub type DbResult<T> = Result<T, DbError>;
