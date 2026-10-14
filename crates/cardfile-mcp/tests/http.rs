//! End-to-end tests over real HTTP on an ephemeral port.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cardfile_mcp::{Handler, McpError, ServerConfig, ServerHandle, ServerInfo, Tool, ToolOutput};
use serde_json::{Value, json};

const ALICE: &str = "alice-token";
const BOB: &str = "bob-token";
const MODERN: &str = "2026-07-28";

struct TestHandler;

impl Handler for TestHandler {
    type Principal = String;

    fn authenticate(&self, token: &str) -> Option<String> {
        match token {
            ALICE => Some("alice".into()),
            BOB => Some("bob".into()),
            _ => None,
        }
    }

    fn tools(&self, _who: &String) -> Vec<Tool> {
        vec![
            Tool::new(
                "echo",
                "Echoes text",
                json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}),
            )
            .with_title("Echo"),
            Tool::new("whoami", "Returns the caller", json!({"type":"object"})),
            Tool::new("slow", "Sleeps", json!({"type":"object"})),
        ]
    }

    fn call_tool(&self, who: &String, name: &str, args: Value) -> Result<ToolOutput, String> {
        match name {
            "echo" => args["text"]
                .as_str()
                .map(|t| ToolOutput::Text(t.to_owned()))
                .ok_or_else(|| "text must be a string".to_owned()),
            "whoami" => Ok(ToolOutput::Json(json!({ "user": who }))),
            "slow" => {
                std::thread::sleep(Duration::from_millis(1500));
                Ok(ToolOutput::Text("done".into()))
            }
            _ => Err("unreachable".into()),
        }
    }
}

fn start_with(config: ServerConfig) -> ServerHandle {
    cardfile_mcp::start(
        config,
        ServerInfo::new("cardfile-test", "1.2.3")
            .with_title("Cardfile Test")
            .with_instructions("Use the tools."),
        Arc::new(TestHandler),
    )
    .unwrap()
}

fn start() -> ServerHandle {
    start_with(ServerConfig::default())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

struct Reply {
    status: u16,
    session: Option<String>,
    www_auth: Option<String>,
    body: Option<Value>,
}

fn post(url: &str, headers: &[(&str, &str)], body: &str) -> Reply {
    let mut req = agent()
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.send(body).unwrap();
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_owned())
    };
    let session = header("mcp-session-id");
    let www_auth = header("www-authenticate");
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap();
    let body =
        (!text.is_empty()).then(|| serde_json::from_str(&text).unwrap_or(Value::String(text)));
    Reply {
        status,
        session,
        www_auth,
        body,
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// Opens a legacy session and returns its id.
fn initialize(url: &str, token: &str, version: &str) -> (String, Value) {
    let auth = bearer(token);
    let r = post(
        url,
        &[("Authorization", &auth)],
        &json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{
            "protocolVersion": version,
            "capabilities": {},
            "clientInfo": {"name":"test","version":"0"}
        }})
        .to_string(),
    );
    assert_eq!(r.status, 200);
    (r.session.expect("session header"), r.body.unwrap())
}

fn session_post(url: &str, session: &str, version: &str, body: Value) -> Reply {
    let auth = bearer(ALICE);
    post(
        url,
        &[
            ("Authorization", &auth),
            ("Mcp-Session-Id", session),
            ("MCP-Protocol-Version", version),
        ],
        &body.to_string(),
    )
}

fn meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion": MODERN,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name":"test","version":"0"}
    })
}

fn modern_post(url: &str, method: &str, name: Option<&str>, params: Value) -> Reply {
    let auth = bearer(ALICE);
    let mut headers = vec![
        ("Authorization", auth.as_str()),
        ("MCP-Protocol-Version", MODERN),
        ("Mcp-Method", method),
    ];
    if let Some(name) = name {
        headers.push(("Mcp-Name", name));
    }
    post(
        url,
        &headers,
        &json!({"jsonrpc":"2.0","id":"m1","method":method,"params":params}).to_string(),
    )
}

#[test]
fn legacy_session_lifecycle() {
    let server = start();
    let url = server.url();
    let v = "2025-11-25";
    let (session, init) = initialize(&url, ALICE, v);
    assert_eq!(session.len(), 64);
    let result = &init["result"];
    assert_eq!(result["protocolVersion"], v);
    assert_eq!(
        result["capabilities"],
        json!({"tools":{"listChanged":false}})
    );
    assert_eq!(result["serverInfo"]["name"], "cardfile-test");
    assert_eq!(result["serverInfo"]["title"], "Cardfile Test");
    assert_eq!(result["instructions"], "Use the tools.");
    assert!(result["capabilities"].get("resources").is_none());

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    assert_eq!(r.status, 202);
    assert!(r.body.is_none());

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
    );
    assert_eq!(r.body.unwrap()["result"], json!({}));

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    );
    assert_eq!(r.status, 200);
    let tools = r.body.unwrap()["result"]["tools"].clone();
    assert_eq!(tools[0]["name"], "echo");
    assert_eq!(tools[0]["title"], "Echo");
    assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["text"]));

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hi"}}}),
    );
    let body = r.body.unwrap();
    assert_eq!(body["id"], 3);
    assert_eq!(
        body["result"]["content"],
        json!([{"type":"text","text":"hi"}])
    );
    assert_eq!(body["result"]["isError"], false);

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{}}}),
    );
    let body = r.body.unwrap();
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(
        body["result"]["content"][0]["text"],
        "text must be a string"
    );

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"whoami"}}),
    );
    let body = r.body.unwrap();
    assert_eq!(body["result"]["structuredContent"], json!({"user":"alice"}));
    assert_eq!(body["result"]["content"][0]["text"], r#"{"user":"alice"}"#);

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"nope"}}),
    );
    assert_eq!(r.body.unwrap()["error"]["code"], -32602);

    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":7,"method":"resources/list"}),
    );
    let body = r.body.unwrap();
    assert_eq!(body["error"]["code"], -32601);
    assert_eq!(body["id"], 7);

    // Wrong protocol version header for this session.
    let r = session_post(
        &url,
        &session,
        "2025-06-18",
        json!({"jsonrpc":"2.0","id":8,"method":"ping"}),
    );
    assert_eq!(r.status, 400);

    // Batches are not allowed after 2025-03-26.
    let r = session_post(
        &url,
        &session,
        v,
        json!([{"jsonrpc":"2.0","id":9,"method":"ping"}]),
    );
    assert_eq!(r.status, 400);

    // DELETE terminates the session.
    let auth = bearer(ALICE);
    let resp = agent()
        .delete(&url)
        .header("Authorization", &auth)
        .header("Mcp-Session-Id", &session)
        .call()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 204);
    let r = session_post(
        &url,
        &session,
        v,
        json!({"jsonrpc":"2.0","id":10,"method":"ping"}),
    );
    assert_eq!(r.status, 404);
}

#[test]
fn version_negotiation_and_batches() {
    let server = start();
    let url = server.url();
    let (_, init) = initialize(&url, ALICE, "2024-11-05");
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    let (_, init) = initialize(&url, ALICE, "2025-06-18");
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");

    let old = "2025-03-26";
    let (session, init) = initialize(&url, ALICE, old);
    assert_eq!(init["result"]["protocolVersion"], old);
    assert!(init["result"]["serverInfo"].get("title").is_none());
    let r = session_post(
        &url,
        &session,
        old,
        json!([
            {"jsonrpc":"2.0","method":"notifications/initialized"},
            {"jsonrpc":"2.0","id":1,"method":"ping"},
            {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"whoami"}},
            {"jsonrpc":"2.0","id":3,"method":"bogus"}
        ]),
    );
    assert_eq!(r.status, 200);
    let replies = r.body.unwrap();
    let replies = replies.as_array().unwrap();
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["result"], json!({}));
    // 2025-03-26 has no structuredContent.
    assert!(replies[1]["result"].get("structuredContent").is_none());
    assert_eq!(replies[2]["error"]["code"], -32601);

    let r = session_post(
        &url,
        &session,
        old,
        json!([{"jsonrpc":"2.0","method":"notifications/x"}]),
    );
    assert_eq!(r.status, 202);
}

#[test]
fn modern_requests() {
    let server = start();
    let url = server.url();

    let r = modern_post(&url, "server/discover", None, json!({"_meta": meta()}));
    assert_eq!(r.status, 200);
    let result = r.body.unwrap()["result"].clone();
    assert_eq!(result["resultType"], "complete");
    assert_eq!(result["supportedVersions"][0], MODERN);
    assert_eq!(
        result["capabilities"],
        json!({"tools":{"listChanged":false}})
    );
    assert_eq!(
        result["_meta"]["io.modelcontextprotocol/serverInfo"]["version"],
        "1.2.3"
    );
    assert_eq!(result["instructions"], "Use the tools.");
    assert!(result["ttlMs"].is_u64());
    assert!(r.session.is_none());

    let r = modern_post(&url, "tools/list", None, json!({"_meta": meta()}));
    let result = r.body.unwrap()["result"].clone();
    assert_eq!(result["tools"].as_array().unwrap().len(), 3);
    assert_eq!(result["cacheScope"], "private");

    let r = modern_post(
        &url,
        "tools/call",
        Some("echo"),
        json!({"_meta": meta(), "name":"echo", "arguments":{"text":"yo"}}),
    );
    assert_eq!(r.status, 200);
    let result = r.body.unwrap()["result"].clone();
    assert_eq!(result["content"][0]["text"], "yo");
    assert_eq!(result["resultType"], "complete");

    // Base64-encoded Mcp-Name.
    let r = modern_post(
        &url,
        "tools/call",
        Some("=?base64?ZWNobw==?="),
        json!({"_meta": meta(), "name":"echo", "arguments":{"text":"b64"}}),
    );
    assert_eq!(r.status, 200);

    // Header/body mismatch.
    let r = modern_post(
        &url,
        "tools/call",
        Some("whoami"),
        json!({"_meta": meta(), "name":"echo", "arguments":{"text":"x"}}),
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.body.unwrap()["error"]["code"], -32020);

    // Missing required _meta.
    let r = modern_post(
        &url,
        "tools/list",
        None,
        json!({"_meta": {"io.modelcontextprotocol/protocolVersion": MODERN}}),
    );
    assert_eq!(r.status, 400);
    assert_eq!(r.body.unwrap()["error"]["code"], -32602);

    // Unsupported version (header and body agree).
    let auth = bearer(ALICE);
    let r = post(
        &url,
        &[
            ("Authorization", &auth),
            ("MCP-Protocol-Version", "1900-01-01"),
            ("Mcp-Method", "tools/list"),
        ],
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"1900-01-01",
            "io.modelcontextprotocol/clientCapabilities":{}
        }}})
        .to_string(),
    );
    assert_eq!(r.status, 400);
    let err = r.body.unwrap()["error"].clone();
    assert_eq!(err["code"], -32022);
    assert_eq!(err["data"]["requested"], "1900-01-01");
    assert_eq!(err["data"]["supported"][0], MODERN);

    // Unknown method.
    let r = modern_post(&url, "resources/list", None, json!({"_meta": meta()}));
    assert_eq!(r.status, 404);
    assert_eq!(r.body.unwrap()["error"]["code"], -32601);

    // Notification.
    let r = post(
        &url,
        &[("Authorization", &auth), ("MCP-Protocol-Version", MODERN)],
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{}}"#,
    );
    assert_eq!(r.status, 202);

    // A legacy client that skipped initialize.
    let r = post(
        &url,
        &[("Authorization", &auth)],
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    );
    assert_eq!(r.status, 400);
}

#[test]
fn transport_errors() {
    let server = start();
    let url = server.url();
    let auth = bearer(ALICE);

    let r = post(&url, &[("Authorization", &auth)], "{not json");
    assert_eq!(r.status, 400);
    let body = r.body.unwrap();
    assert_eq!(body["error"]["code"], -32700);
    assert!(body.get("id").is_none());

    let r = post(&url, &[], "{}");
    assert_eq!(r.status, 401);
    assert!(r.www_auth.unwrap().starts_with("Bearer"));

    let r = post(&url, &[("Authorization", "Bearer wrong")], "{}");
    assert_eq!(r.status, 401);
    assert!(r.www_auth.unwrap().contains("invalid_token"));

    let r = post(&url, &[("Authorization", "Basic YWxpY2U6eA==")], "{}");
    assert_eq!(r.status, 401);

    let r = post(
        &url,
        &[("Authorization", &auth), ("Origin", "https://evil.example")],
        "{}",
    );
    assert_eq!(r.status, 403);

    // A local origin is fine.
    let r = post(
        &url,
        &[
            ("Authorization", &auth),
            ("Origin", "http://localhost:5173"),
        ],
        &json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}
        }})
        .to_string(),
    );
    assert_eq!(r.status, 200);
    let session = r.session.unwrap();

    let r = session_post(
        &url,
        "deadbeef",
        "2025-11-25",
        json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
    );
    assert_eq!(r.status, 404);

    // Bob may not use Alice's session.
    let bob = bearer(BOB);
    let r = post(
        &url,
        &[("Authorization", &bob), ("Mcp-Session-Id", &session)],
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
    );
    assert_eq!(r.status, 403);

    let resp = agent()
        .get(&url)
        .header("Authorization", &auth)
        .call()
        .unwrap();
    assert_eq!(resp.status().as_u16(), 405);

    let other = url.replace("/mcp", "/other");
    let r = post(&other, &[("Authorization", &auth)], "{}");
    assert_eq!(r.status, 404);

    let r = post(
        &url,
        &[("Authorization", &auth)],
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    );
    assert_eq!(r.status, 400);
}

#[test]
fn body_size_limit() {
    let server = start_with(ServerConfig {
        max_body: 64,
        ..ServerConfig::default()
    });
    let auth = bearer(ALICE);
    let big = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","x":"{}"}}"#,
        "a".repeat(200)
    );
    let r = post(&server.url(), &[("Authorization", &auth)], &big);
    assert_eq!(r.status, 413);
}

#[test]
fn slow_tool_does_not_block_other_requests() {
    let server = start();
    let url = server.url();
    let (session, _) = initialize(&url, ALICE, "2025-11-25");
    let slow = {
        let (url, session) = (url.clone(), session.clone());
        std::thread::spawn(move || {
            session_post(
                &url,
                &session,
                "2025-11-25",
                json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"slow"}}),
            )
        })
    };
    std::thread::sleep(Duration::from_millis(200));
    let started = Instant::now();
    let r = session_post(
        &url,
        &session,
        "2025-11-25",
        json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
    );
    assert_eq!(r.status, 200);
    assert!(started.elapsed() < Duration::from_millis(1000));
    let r = slow.join().unwrap();
    assert_eq!(r.body.unwrap()["result"]["content"][0]["text"], "done");
}

#[test]
fn stop_frees_the_port_and_port_in_use_is_reported() {
    let server = start();
    let addr = server.local_addr();
    assert!(addr.ip().is_loopback());
    assert_eq!(server.url(), format!("http://{addr}/mcp"));

    let err = cardfile_mcp::start(
        ServerConfig::with_port(addr.port()),
        ServerInfo::new("x", "1"),
        Arc::new(TestHandler),
    )
    .unwrap_err();
    assert!(matches!(err, McpError::PortInUse { port } if port == addr.port()));

    server.stop();
    let listener = TcpListener::bind(addr).expect("port is free after stop");
    drop(listener);

    // Dropping the handle stops the server too.
    let server = start_with(ServerConfig::with_port(addr.port()));
    drop(server);
    TcpListener::bind(addr).expect("port is free after drop");
}

#[test]
fn invalid_config_is_rejected() {
    let err = cardfile_mcp::start(
        ServerConfig {
            path: "mcp".into(),
            ..ServerConfig::default()
        },
        ServerInfo::new("x", "1"),
        Arc::new(TestHandler),
    )
    .unwrap_err();
    assert!(matches!(err, McpError::InvalidConfig(_)));
}
