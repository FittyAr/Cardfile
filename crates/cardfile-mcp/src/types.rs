//! Public types the application uses to describe its server and tools.

use std::time::Duration;

use serde_json::Value;

/// Identity of the server, reported to clients in `initialize` / `server/discover`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    /// Programmatic name, e.g. `"cardfile"`.
    pub name: String,
    /// Human-readable name for client UIs, e.g. `"Cardfile"`.
    pub title: Option<String>,
    /// Version of the application.
    pub version: String,
    /// Optional natural-language guidance for the model on how to use the tools.
    pub instructions: Option<String>,
}

impl ServerInfo {
    /// Server info with just a name and version.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            title: None,
            version: version.into(),
            instructions: None,
        }
    }

    /// Sets the human-readable title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the instructions for the model.
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
}

/// A tool definition as listed by `tools/list`.
///
/// Tool names should be 1–128 characters from `A-Z a-z 0-9 _ - .` and unique.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Unique, case-sensitive identifier, e.g. `"search_cards"`.
    pub name: String,
    /// Optional display name.
    pub title: Option<String>,
    /// What the tool does; read by the model.
    pub description: String,
    /// JSON Schema (2020-12 by default) for the `arguments` object. Must be an object schema;
    /// for a tool without parameters use `{"type":"object","additionalProperties":false}`.
    pub input_schema: Value,
    /// Optional JSON Schema for the structured result ([`ToolOutput::Json`]).
    pub output_schema: Option<Value>,
    /// The tool does not modify anything (`annotations.readOnlyHint`).
    pub read_only: bool,
    /// The tool may destroy or overwrite data (`annotations.destructiveHint`). Only meaningful
    /// when `read_only` is false.
    pub destructive: bool,
    /// Repeating a call with the same arguments has no additional effect
    /// (`annotations.idempotentHint`).
    pub idempotent: bool,
    /// The tool reaches entities outside the application, e.g. the internet
    /// (`annotations.openWorldHint`). Cardfile's tools work on the local database, so this
    /// defaults to false.
    pub open_world: bool,
}

impl Tool {
    /// A read-only tool with the given schema. Use the builder methods to adjust it.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            name: name.into(),
            title: None,
            description: description.into(),
            input_schema,
            output_schema: None,
            read_only: true,
            destructive: false,
            idempotent: false,
            open_world: false,
        }
    }

    /// Sets the display title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the output schema.
    pub fn with_output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Marks the tool as modifying data. `destructive` says whether it may delete or
    /// overwrite existing data (as opposed to only adding).
    pub fn writes(mut self, destructive: bool) -> Self {
        self.read_only = false;
        self.destructive = destructive;
        self
    }

    /// Marks the tool as idempotent.
    pub fn idempotent(mut self) -> Self {
        self.idempotent = true;
        self
    }
}

/// Successful result of a tool call.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolOutput {
    /// Plain text, sent as a single text content block.
    Text(String),
    /// Structured data. Sent as `structuredContent` and, for clients that only read
    /// `content`, also serialized into a text block. Protocol revisions before 2025-06-18 have
    /// no `structuredContent` (and 2025-06-18/2025-11-25 only accept objects there), in which
    /// case only the text block is sent.
    Json(Value),
}

/// Supplies authentication and tools to the server.
///
/// Methods are called from the server's worker threads, possibly concurrently. They may
/// block (e.g. on a database query); a slow call only occupies one worker.
pub trait Handler: Send + Sync + 'static {
    /// Who a request is acting for (e.g. a user id plus granted permissions). A legacy session
    /// remembers the principal that created it and rejects requests whose token
    /// authenticates to a different (`!=`) principal.
    type Principal: Clone + PartialEq + Send + Sync + 'static;

    /// Validates a bearer token from the `Authorization` header. `None` makes the server answer
    /// `401 Unauthorized`. Implementations should compare secrets in constant time and must not
    /// log the token.
    fn authenticate(&self, token: &str) -> Option<Self::Principal>;

    /// Tools available to `who`, in a stable order (clients cache the list).
    fn tools(&self, who: &Self::Principal) -> Vec<Tool>;

    /// Runs a tool listed by [`Handler::tools`]. `arguments` is always a JSON object; validate
    /// it against the schema. `Err(message)` is a tool execution error: it is returned to the
    /// model with `isError: true` so it can correct itself. Calls to unknown tools never reach
    /// this method.
    fn call_tool(
        &self,
        who: &Self::Principal,
        name: &str,
        arguments: Value,
    ) -> Result<ToolOutput, String>;
}

/// Listener and resource limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// TCP port on 127.0.0.1. `0` picks a free ephemeral port (see
    /// [`crate::ServerHandle::local_addr`]).
    pub port: u16,
    /// Path of the MCP endpoint; must start with `/`. Default `/mcp`.
    pub path: String,
    /// Largest accepted request body in bytes; bigger bodies get `413`. Default 4 MiB.
    pub max_body: usize,
    /// Worker threads serving requests concurrently (1–64). Default 4.
    pub threads: usize,
    /// Most legacy sessions kept at once; the least recently used is evicted. Default 64.
    pub max_sessions: usize,
    /// Legacy sessions unused for this long expire (clients then re-initialize). Default 12 h.
    pub session_idle_timeout: Duration,
    /// Freshness hint (`ttlMs`) for modern `tools/list` results, in milliseconds. Default 0:
    /// the tool set can change with the user's settings, so clients should not cache it.
    pub tools_ttl_ms: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 0,
            path: "/mcp".to_owned(),
            max_body: 4 * 1024 * 1024,
            threads: 4,
            max_sessions: 64,
            session_idle_timeout: Duration::from_secs(12 * 60 * 60),
            tools_ttl_ms: 0,
        }
    }
}

impl ServerConfig {
    /// Default configuration on a specific port.
    pub fn with_port(port: u16) -> Self {
        Self {
            port,
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::McpError> {
        let bad = |why: &str| Err(crate::McpError::InvalidConfig(why.to_owned()));
        if !self.path.starts_with('/') || self.path.contains(['?', '#', ' ']) {
            return bad("path must start with '/' and contain no '?', '#' or spaces");
        }
        if !(1..=64).contains(&self.threads) {
            return bad("threads must be between 1 and 64");
        }
        if self.max_body == 0 {
            return bad("max_body must be greater than zero");
        }
        Ok(())
    }
}
