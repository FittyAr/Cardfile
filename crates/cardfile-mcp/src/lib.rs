//! Embedded [Model Context Protocol](https://modelcontextprotocol.io) server for Cardfile.
//!
//! The server speaks the **Streamable HTTP** transport on `127.0.0.1` only and runs on
//! background threads while the application is open. This crate is protocol and transport
//! only: the application supplies authentication and tools through [`Handler`].
//!
//! # Protocol support
//!
//! The server is *dual-era* (see [`ProtocolVersion`]):
//!
//! | Revision | Shape |
//! |----------|-------|
//! | `2026-07-28` | Stateless: every request carries `_meta` with its protocol version and client capabilities, mirrored in the `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers. `server/discover`, `tools/list`, `tools/call`. |
//! | `2025-11-25`, `2025-06-18`, `2025-03-26` | `initialize` handshake, `Mcp-Session-Id` session, `ping`, `tools/list`, `tools/call`. JSON-RPC batches only in `2025-03-26`. |
//!
//! Only the `tools` capability is advertised (`listChanged: false`). Responses are always a
//! single `application/json` body; `GET` (standalone SSE stream) answers `405`. `DELETE`
//! with an `Mcp-Session-Id` ends a legacy session.
//!
//! # Security
//!
//! * Bound to `127.0.0.1`; requests whose `Origin` (or `Host`) is not `localhost`,
//!   `127.0.0.1` or `[::1]` get `403` (DNS-rebinding protection).
//! * Every request needs `Authorization: Bearer <token>`, checked by
//!   [`Handler::authenticate`]; otherwise `401` with `WWW-Authenticate: Bearer`.
//! * A legacy session only accepts requests whose token maps to the principal that opened it.
//! * Bodies are limited to [`ServerConfig::max_body`] (`413` beyond).
//!
//! # Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use cardfile_mcp::{Handler, ServerConfig, ServerInfo, Tool, ToolOutput};
//! use serde_json::{Value, json};
//!
//! struct App { token: String }
//!
//! impl Handler for App {
//!     type Principal = ();
//!     fn authenticate(&self, token: &str) -> Option<()> {
//!         (token == self.token).then_some(())
//!     }
//!     fn tools(&self, _: &()) -> Vec<Tool> {
//!         vec![Tool::new("echo", "Returns its input", json!({
//!             "type": "object",
//!             "properties": { "text": { "type": "string" } },
//!             "required": ["text"]
//!         }))]
//!     }
//!     fn call_tool(&self, _: &(), _name: &str, args: Value) -> Result<ToolOutput, String> {
//!         args["text"].as_str().map(|t| ToolOutput::Text(t.to_owned()))
//!             .ok_or_else(|| "text is required".to_owned())
//!     }
//! }
//!
//! let handle = cardfile_mcp::start(
//!     ServerConfig::with_port(3917),
//!     ServerInfo::new("cardfile", "2.0.0").with_title("Cardfile"),
//!     Arc::new(App { token: "secret".into() }),
//! )?;
//! println!("MCP server at {}", handle.url());
//! handle.stop();
//! # Ok::<(), cardfile_mcp::McpError>(())
//! ```
//!
//! # Connecting clients
//!
//! [`client_config_snippets`] produces these for a given URL and token. With
//! `URL = http://127.0.0.1:3917/mcp` and `KEY` the bearer token:
//!
//! **Claude Code** (CLI):
//!
//! ```text
//! claude mcp add --transport http cardfile http://127.0.0.1:3917/mcp --header "Authorization: Bearer KEY"
//! ```
//!
//! or in a project's `.mcp.json`:
//!
//! ```json
//! { "mcpServers": { "cardfile": {
//!     "type": "http",
//!     "url": "http://127.0.0.1:3917/mcp",
//!     "headers": { "Authorization": "Bearer KEY" } } } }
//! ```
//!
//! **Claude Desktop** starts local servers from `claude_desktop_config.json`
//! (`%APPDATA%\Claude\` on Windows) and reaches HTTP servers through the
//! [`mcp-remote`](https://github.com/geelen/mcp-remote) bridge (requires Node.js). The header
//! goes through an environment variable because Claude Desktop on Windows does not escape
//! spaces in `args`:
//!
//! ```json
//! { "mcpServers": { "cardfile": {
//!     "command": "npx",
//!     "args": ["-y", "mcp-remote", "http://127.0.0.1:3917/mcp",
//!              "--header", "Authorization:${AUTH_HEADER}",
//!              "--transport", "http-only", "--allow-http"],
//!     "env": { "AUTH_HEADER": "Bearer KEY" } } } }
//! ```
//!
//! **Cursor** (`~/.cursor/mcp.json` or a project's `.cursor/mcp.json`):
//!
//! ```json
//! { "mcpServers": { "cardfile": {
//!     "url": "http://127.0.0.1:3917/mcp",
//!     "headers": { "Authorization": "Bearer KEY" } } } }
//! ```
//!
//! **VS Code** (`.vscode/mcp.json`, or the user `mcp.json`):
//!
//! ```json
//! { "servers": { "cardfile": {
//!     "type": "http",
//!     "url": "http://127.0.0.1:3917/mcp",
//!     "headers": { "Authorization": "Bearer KEY" } } } }
//! ```

mod clients;
mod error;
mod jsonrpc;
mod protocol;
mod server;
mod session;
mod transport;
mod types;
mod version;

pub use clients::client_config_snippets;
pub use error::McpError;
pub use server::{ServerHandle, start};
pub use types::{Handler, ServerConfig, ServerInfo, Tool, ToolOutput};
pub use version::{ProtocolVersion, supported_versions};
