//! Streamable HTTP transport rules, independent of the HTTP server library.
//!
//! One endpoint serves both eras:
//! * **Legacy** (2025-03-26 … 2025-11-25): `initialize` without a session header creates a
//!   session (`Mcp-Session-Id` response header); later requests must carry it. `DELETE`
//!   ends it.
//! * **Modern** (2026-07-28): no session; each request carries `_meta` with its protocol
//!   version, mirrored in the `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` headers.

use std::io::Read;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Map, Value};

use crate::jsonrpc::{self, Message, RpcError, code};
use crate::protocol::Protocol;
use crate::session::SessionStore;
use crate::types::Handler;
use crate::version::ProtocolVersion;

const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
const SESSION_HEADER: &str = "Mcp-Session-Id";

/// A request as seen by the transport. Header names are lower-case.
pub(crate) struct HttpRequest<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(String, String)],
    pub content_length: Option<usize>,
    pub body: &'a mut dyn Read,
}

impl HttpRequest<'_> {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.trim())
    }
}

/// A response to write back.
#[derive(Debug)]
pub(crate) struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn text(status: u16, text: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "text/plain; charset=utf-8".to_owned())],
            body: text.as_bytes().to_vec(),
        }
    }

    fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type", "application/json".to_owned())],
            body: value.to_string().into_bytes(),
        }
    }

    fn error(status: u16, id: Option<&Value>, error: &RpcError) -> Self {
        Self::json(status, &jsonrpc::failure(id, error))
    }

    fn with_header(mut self, name: &'static str, value: String) -> Self {
        self.headers.push((name, value));
        self
    }
}

/// The MCP endpoint: transport checks, sessions and dispatch.
pub(crate) struct Endpoint<H: Handler> {
    protocol: Protocol<H>,
    sessions: SessionStore<H::Principal>,
    path: String,
    max_body: usize,
}

impl<H: Handler> Endpoint<H> {
    pub fn new(
        protocol: Protocol<H>,
        path: String,
        max_body: usize,
        max_sessions: usize,
        idle: Duration,
    ) -> Self {
        Self {
            protocol,
            sessions: SessionStore::new(max_sessions, idle),
            path,
            max_body,
        }
    }

    pub fn handle(&self, req: &mut HttpRequest<'_>) -> HttpResponse {
        let path = req.url.split(['?', '#']).next().unwrap_or_default();
        if path != self.path {
            return HttpResponse::text(404, "Not Found");
        }
        // DNS-rebinding protection: a web page may only reach us from a local origin.
        if req.header("origin").is_some_and(|o| !is_local_origin(o))
            || req.header("host").is_some_and(|h| !is_local_host(h))
        {
            return HttpResponse::error(
                403,
                None,
                &RpcError::invalid_request("Forbidden: origin not allowed"),
            );
        }
        let principal = match bearer_token(req.header("authorization")) {
            None => return unauthorized(false),
            Some(token) => match self.protocol.handler().authenticate(token) {
                None => return unauthorized(true),
                Some(p) => p,
            },
        };
        match req.method {
            "POST" => self.post(req, principal),
            "DELETE" => self.delete(req, &principal),
            // No standalone SSE stream (allowed by every revision).
            _ => HttpResponse::text(405, "Method Not Allowed")
                .with_header("Allow", "POST, DELETE".to_owned()),
        }
    }

    fn delete(&self, req: &HttpRequest<'_>, principal: &H::Principal) -> HttpResponse {
        let Some(id) = req.header("mcp-session-id") else {
            return HttpResponse::error(
                400,
                None,
                &RpcError::invalid_request("Missing Mcp-Session-Id header"),
            );
        };
        match self.sessions.touch(id) {
            None => session_not_found(),
            Some(s) if s.principal != *principal => foreign_session(),
            Some(_) => {
                self.sessions.remove(id);
                HttpResponse::empty(204)
            }
        }
    }

    fn post(&self, req: &mut HttpRequest<'_>, principal: H::Principal) -> HttpResponse {
        if req.content_length.is_some_and(|n| n > self.max_body) {
            return HttpResponse::text(413, "Payload Too Large");
        }
        if let Some(ct) = req.header("content-type")
            && !media_type_is(ct, "application/json")
        {
            return HttpResponse::text(415, "Content-Type must be application/json");
        }
        if let Some(accept) = req.header("accept")
            && !accepts_json(accept)
        {
            return HttpResponse::text(406, "Accept must include application/json");
        }
        let mut body = Vec::new();
        let limit = u64::try_from(self.max_body)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if req.body.take(limit).read_to_end(&mut body).is_err() {
            return HttpResponse::text(400, "Could not read the request body");
        }
        if body.len() > self.max_body {
            return HttpResponse::text(413, "Payload Too Large");
        }
        let value: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return HttpResponse::error(
                    400,
                    None,
                    &RpcError::parse(format!("Parse error: {e}")),
                );
            }
        };

        if let Some(session_id) = req.header("mcp-session-id") {
            return self.legacy(req, session_id, principal, &value);
        }
        if value.get("method").and_then(Value::as_str) == Some("initialize") {
            return self.initialize(principal, &value);
        }
        self.modern(req, &principal, &value)
    }

    /// `initialize` without a session: negotiate a legacy revision and open a session.
    fn initialize(&self, principal: H::Principal, value: &Value) -> HttpResponse {
        let (id, params) = match jsonrpc::classify(value) {
            Ok(Message::Request { id, params, .. }) => (id, params),
            Ok(_) => {
                return HttpResponse::error(
                    400,
                    None,
                    &RpcError::invalid_request("initialize must be a request"),
                );
            }
            Err((id, e)) => return HttpResponse::error(400, id.as_ref(), &e),
        };
        let Some(requested) = params.get("protocolVersion").and_then(Value::as_str) else {
            return HttpResponse::error(
                400,
                Some(&id),
                &RpcError::invalid_params("Missing \"protocolVersion\""),
            );
        };
        let version = Protocol::<H>::negotiate(requested);
        let Some(session_id) = self.sessions.create(principal, version) else {
            return HttpResponse::error(
                500,
                Some(&id),
                &RpcError::internal("Could not create a session"),
            );
        };
        tracing::debug!(%version, "MCP session initialized");
        let result = self.protocol.initialize_result(version);
        HttpResponse::json(200, &jsonrpc::success(&id, result))
            .with_header(SESSION_HEADER, session_id)
    }

    /// A request inside a legacy session.
    fn legacy(
        &self,
        req: &HttpRequest<'_>,
        session_id: &str,
        principal: H::Principal,
        value: &Value,
    ) -> HttpResponse {
        let Some(session) = self.sessions.touch(session_id) else {
            return session_not_found();
        };
        if session.principal != principal {
            return foreign_session();
        }
        let version = session.version;
        if let Some(header) = req.header("mcp-protocol-version")
            && header != version.as_str()
        {
            return HttpResponse::error(
                400,
                None,
                &RpcError::invalid_request(format!(
                    "MCP-Protocol-Version {header} does not match the negotiated version {version}"
                )),
            );
        }

        if let Value::Array(batch) = value {
            if !version.allows_batches() {
                return HttpResponse::error(
                    400,
                    None,
                    &RpcError::invalid_request(format!(
                        "JSON-RPC batches are not allowed in protocol version {version}"
                    )),
                );
            }
            if batch.is_empty() {
                return HttpResponse::error(400, None, &RpcError::invalid_request("Empty batch"));
            }
            let replies: Vec<Value> = batch
                .iter()
                .filter_map(|item| match jsonrpc::classify(item) {
                    Err((id, e)) => Some(jsonrpc::failure(id.as_ref(), &e)),
                    Ok(msg) => self.legacy_message(&principal, version, msg),
                })
                .collect();
            return if replies.is_empty() {
                HttpResponse::empty(202)
            } else {
                HttpResponse::json(200, &Value::Array(replies))
            };
        }

        match jsonrpc::classify(value) {
            Err((id, e)) => HttpResponse::error(400, id.as_ref(), &e),
            Ok(msg) => match self.legacy_message(&principal, version, msg) {
                None => HttpResponse::empty(202),
                Some(reply) => HttpResponse::json(200, &reply),
            },
        }
    }

    /// Handles one legacy message; `None` for notifications and responses.
    fn legacy_message(
        &self,
        principal: &H::Principal,
        version: ProtocolVersion,
        msg: Message,
    ) -> Option<Value> {
        match msg {
            // `notifications/initialized`, `notifications/cancelled` (calls are synchronous,
            // nothing to cancel) and responses need no action.
            Message::Notification { .. } | Message::Response => None,
            Message::Request { id, method, .. } if method == "initialize" => {
                Some(jsonrpc::failure(
                    Some(&id),
                    &RpcError::invalid_request("Session already initialized"),
                ))
            }
            Message::Request { id, method, params } => Some(
                match self.protocol.call(principal, version, &method, &params) {
                    Ok(result) => jsonrpc::success(&id, result),
                    Err(e) => jsonrpc::failure(Some(&id), &e),
                },
            ),
        }
    }

    /// A sessionless request: modern protocol, or a legacy client that skipped `initialize`.
    fn modern(
        &self,
        req: &HttpRequest<'_>,
        principal: &H::Principal,
        value: &Value,
    ) -> HttpResponse {
        let header_version = req.header("mcp-protocol-version");
        if value.is_array() {
            return HttpResponse::error(
                400,
                None,
                &RpcError::invalid_request(
                    "JSON-RPC batches are only allowed in a 2025-03-26 session",
                ),
            );
        }
        let (id, method, params) = match jsonrpc::classify(value) {
            Err((id, e)) => return HttpResponse::error(400, id.as_ref(), &e),
            Ok(Message::Response) => {
                return HttpResponse::error(
                    400,
                    None,
                    &RpcError::invalid_request("Clients must not send responses"),
                );
            }
            Ok(Message::Notification { .. }) => return HttpResponse::empty(202),
            Ok(Message::Request { id, method, params }) => (id, method, params),
        };
        let bad = |status: u16, e: RpcError| HttpResponse::error(status, Some(&id), &e);

        let meta = params.get("_meta").and_then(Value::as_object);
        let Some(body_version) = meta
            .and_then(|m| m.get(META_PROTOCOL_VERSION))
            .and_then(Value::as_str)
        else {
            let legacy_client = match header_version {
                None => true,
                Some(h) => ProtocolVersion::parse(h).is_some_and(|v| !v.is_modern()),
            };
            return if legacy_client {
                bad(
                    400,
                    RpcError::invalid_request(
                        "Missing Mcp-Session-Id header: send an initialize request first",
                    ),
                )
            } else {
                bad(
                    400,
                    RpcError::invalid_params(format!(
                        "Missing required _meta field {META_PROTOCOL_VERSION}"
                    )),
                )
            };
        };
        if !meta
            .and_then(|m| m.get(META_CLIENT_CAPABILITIES))
            .is_some_and(Value::is_object)
        {
            return bad(
                400,
                RpcError::invalid_params(format!(
                    "Missing required _meta field {META_CLIENT_CAPABILITIES}"
                )),
            );
        }
        match header_version {
            None => {
                return bad(
                    400,
                    RpcError::header_mismatch("missing MCP-Protocol-Version header"),
                );
            }
            Some(h) if h != body_version => {
                return bad(
                    400,
                    RpcError::header_mismatch(format!(
                        "MCP-Protocol-Version header value '{h}' does not match body value '{body_version}'"
                    )),
                );
            }
            Some(_) => {}
        }
        let version = match ProtocolVersion::parse(body_version) {
            Some(v) if v.is_modern() => v,
            _ => return bad(400, RpcError::unsupported_version(body_version)),
        };
        if let Err(e) = check_mirrored_headers(req, &method, &params) {
            return bad(400, e);
        }

        match self.protocol.call(principal, version, &method, &params) {
            Ok(result) => HttpResponse::json(200, &jsonrpc::success(&id, result)),
            Err(e) => bad(modern_status(e.code), e),
        }
    }
}

/// Validates `Mcp-Method` and, for `tools/call`, `Mcp-Name` against the body.
fn check_mirrored_headers(
    req: &HttpRequest<'_>,
    method: &str,
    params: &Map<String, Value>,
) -> Result<(), RpcError> {
    match req.header("mcp-method") {
        None => return Err(RpcError::header_mismatch("missing Mcp-Method header")),
        Some(h) if h != method => {
            return Err(RpcError::header_mismatch(format!(
                "Mcp-Method header value '{h}' does not match body value '{method}'"
            )));
        }
        Some(_) => {}
    }
    if method == "tools/call" {
        let Some(raw) = req.header("mcp-name") else {
            return Err(RpcError::header_mismatch("missing Mcp-Name header"));
        };
        let Some(header_name) = decode_header_value(raw) else {
            return Err(RpcError::header_mismatch("malformed Mcp-Name header"));
        };
        if let Some(body_name) = params.get("name").and_then(Value::as_str)
            && header_name != body_name
        {
            return Err(RpcError::header_mismatch(format!(
                "Mcp-Name header value '{header_name}' does not match body value '{body_name}'"
            )));
        }
    }
    Ok(())
}

/// Decodes the `=?base64?…?=` sentinel form used for non-ASCII header values.
fn decode_header_value(raw: &str) -> Option<String> {
    match raw
        .strip_prefix("=?base64?")
        .and_then(|rest| rest.strip_suffix("?="))
    {
        Some(encoded) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .ok()?;
            String::from_utf8(bytes).ok()
        }
        None => Some(raw.to_owned()),
    }
}

/// HTTP status for a JSON-RPC error on the modern path.
fn modern_status(code: i64) -> u16 {
    match code {
        code::METHOD_NOT_FOUND => 404,
        code::INTERNAL_ERROR => 500,
        _ => 400,
    }
}

fn unauthorized(token_given: bool) -> HttpResponse {
    let challenge = if token_given {
        r#"Bearer realm="mcp", error="invalid_token""#
    } else {
        r#"Bearer realm="mcp""#
    };
    HttpResponse::text(401, "Unauthorized").with_header("WWW-Authenticate", challenge.to_owned())
}

fn session_not_found() -> HttpResponse {
    HttpResponse::error(404, None, &RpcError::invalid_request("Session not found"))
}

fn foreign_session() -> HttpResponse {
    HttpResponse::error(
        403,
        None,
        &RpcError::invalid_request("Session belongs to a different principal"),
    )
}

/// Extracts the token from `Authorization: Bearer <token>` (scheme is case-insensitive).
fn bearer_token(header: Option<&str>) -> Option<&str> {
    let (scheme, token) = header?.trim().split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

fn media_type_is(value: &str, expected: &str) -> bool {
    value
        .split(';')
        .next()
        .is_some_and(|m| m.trim().eq_ignore_ascii_case(expected))
}

fn accepts_json(accept: &str) -> bool {
    accept.split(',').any(|item| {
        ["application/json", "application/*", "*/*"]
            .iter()
            .any(|m| media_type_is(item, m))
    })
}

fn is_loopback_name(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "[::1]"
}

/// Strips an optional `:port` from `host[:port]` / `[v6][:port]`.
fn host_without_port(authority: &str) -> &str {
    if authority.starts_with('[') {
        match authority.find(']') {
            Some(end) => &authority[..=end],
            None => authority,
        }
    } else {
        authority.split(':').next().unwrap_or(authority)
    }
}

fn is_local_host(host_header: &str) -> bool {
    is_loopback_name(host_without_port(host_header))
}

fn is_local_origin(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.split_once("://") else {
        return false;
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        return false;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    !authority.contains('@') && is_loopback_name(host_without_port(authority))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_rules() {
        assert!(is_local_origin("http://localhost:3000"));
        assert!(is_local_origin("https://127.0.0.1"));
        assert!(is_local_origin("http://[::1]:8080"));
        assert!(!is_local_origin("http://localhost.evil.com"));
        assert!(!is_local_origin("http://evil.com"));
        assert!(!is_local_origin("null"));
        assert!(!is_local_origin("file://localhost"));
        assert!(!is_local_origin("http://localhost@evil.com"));
        assert!(is_local_host("127.0.0.1:4000"));
        assert!(is_local_host("[::1]:4000"));
        assert!(!is_local_host("attacker.example:4000"));
    }

    #[test]
    fn bearer_parsing() {
        assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(Some("bearer  abc ")), Some("abc"));
        assert_eq!(bearer_token(Some("Basic abc")), None);
        assert_eq!(bearer_token(Some("Bearer ")), None);
        assert_eq!(bearer_token(None), None);
    }

    #[test]
    fn header_value_decoding() {
        assert_eq!(decode_header_value("echo").as_deref(), Some("echo"));
        assert_eq!(
            decode_header_value("=?base64?SGVsbG8sIOS4lueVjA==?=").as_deref(),
            Some("Hello, 世界")
        );
        assert_eq!(decode_header_value("=?base64?***?="), None);
    }

    #[test]
    fn accept_rules() {
        assert!(accepts_json("application/json, text/event-stream"));
        assert!(accepts_json("*/*"));
        assert!(!accepts_json("text/event-stream"));
    }
}
