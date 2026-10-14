//! Errors returned when starting the server.

use std::net::SocketAddr;

use thiserror::Error;

/// Why the MCP server could not start.
#[derive(Debug, Error)]
pub enum McpError {
    /// Another program already listens on the requested port.
    #[error("port {port} is already in use")]
    PortInUse {
        /// The requested port.
        port: u16,
    },
    /// Binding the listener failed for another reason (e.g. permission denied).
    #[error("could not listen on {addr}: {source}")]
    Bind {
        /// The address that was requested.
        addr: SocketAddr,
        /// The OS error.
        #[source]
        source: std::io::Error,
    },
    /// The [`crate::ServerConfig`] is invalid.
    #[error("invalid MCP server configuration: {0}")]
    InvalidConfig(String),
    /// The HTTP server or its worker threads could not be started.
    #[error("could not start the MCP server: {0}")]
    Start(String),
}
