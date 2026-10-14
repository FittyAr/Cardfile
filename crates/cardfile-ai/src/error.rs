//! Typed errors of the AI client.

use thiserror::Error;

/// Everything that can go wrong while talking to a provider.
///
/// Messages never contain the API key: provider error bodies and transport
/// errors are scrubbed before they are wrapped here.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AiError {
    /// The provider answered with a non-success HTTP status.
    #[error("HTTP {status}: {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message extracted from the provider's response body.
        message: String,
    },
    /// The request could not be sent or the response could not be read.
    #[error("network error: {0}")]
    Network(String),
    /// The provider did not answer in time.
    #[error("the request timed out")]
    Timeout,
    /// The response was not in the expected format.
    #[error("unexpected response: {0}")]
    Decode(String),
    /// The provider needs an API key and none is configured.
    #[error("an API key is required for this provider")]
    MissingApiKey,
    /// The configuration is incomplete (missing base URL or model).
    #[error("invalid provider configuration: {0}")]
    Config(String),
    /// The operation is not supported by this provider.
    #[error("not supported: {0}")]
    Unsupported(String),
    /// The caller cancelled the operation.
    #[error("cancelled")]
    Cancelled,
}
