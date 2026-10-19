//! End-to-end `run_agent` test against a tiny local mock of an
//! OpenAI-compatible server (no external network).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::thread;

use cardfile_ai::{
    AgentEvent, AgentOutcome, AiError, Message, ProviderConfig, Role, ToolSpec, preset, run_agent,
};
use serde_json::{Value, json};

struct Captured {
    request_line: String,
    headers: Vec<String>,
    body: Value,
}

/// Serve one canned JSON reply per incoming request, in order, and return
/// what was received.
fn serve(replies: Vec<(u16, Value)>) -> (String, thread::JoinHandle<Vec<Captured>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let mut captured = Vec::new();
        for (status, reply) in replies {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = Vec::new();
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let line = line.trim_end().to_owned();
                if line.is_empty() {
                    break;
                }
                if let Some((k, v)) = line.split_once(':')
                    && k.eq_ignore_ascii_case("content-length")
                {
                    len = v.trim().parse().unwrap();
                }
                headers.push(line);
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            captured.push(Captured {
                request_line: request_line.trim_end().to_owned(),
                headers,
                body: serde_json::from_slice(&body).unwrap_or(Value::Null),
            });
            let payload = reply.to_string();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            )
            .unwrap();
            stream.flush().unwrap();
        }
        captured
    });
    (format!("http://{addr}/v1"), handle)
}

fn config(base_url: String) -> ProviderConfig {
    let mut cfg = ProviderConfig::from_preset(preset("custom").unwrap());
    cfg.base_url = base_url;
    cfg.model = "mock-model".into();
    cfg.api_key = Some("test-key".into());
    cfg
}

#[test]
fn run_agent_round_trips_one_tool_call() {
    let (url, server) = serve(vec![
        (
            200,
            json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
                "tool_calls":[{"id":"call_1","type":"function",
                    "function":{"name":"count_cards","arguments":"{\"tag\":\"work\"}"}}]}}]}),
        ),
        (
            200,
            json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant",
                "content":"You have 3 work cards."}}],
                "usage":{"prompt_tokens":30,"completion_tokens":7}}),
        ),
    ]);
    let cfg = config(url);
    let tools = [ToolSpec::new(
        "count_cards",
        "Count cards with a tag",
        json!({"type":"object","properties":{"tag":{"type":"string"}},"required":["tag"]}),
    )];
    let mut history = vec![Message::user("How many work cards?")];
    let mut events = Vec::new();
    let mut exec = |call: &cardfile_ai::ToolCall| {
        assert_eq!(call.name, "count_cards");
        assert_eq!(call.arguments, json!({"tag":"work"}));
        Ok::<_, String>("3".to_owned())
    };
    let cancel = AtomicBool::new(false);
    let outcome = run_agent(
        &cfg,
        "You manage cards.",
        &mut history,
        &tools,
        &mut exec,
        &mut |e| events.push(e),
        5,
        &cancel,
    )
    .unwrap();
    assert_eq!(outcome, AgentOutcome::Finished);

    let roles: Vec<Role> = history.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        vec![Role::User, Role::Assistant, Role::Tool, Role::Assistant]
    );
    assert_eq!(history[2].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(history[3].content, "You have 3 work cards.");
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0], AgentEvent::ToolCall(c) if c.id == "call_1"));
    assert!(
        matches!(&events[1], AgentEvent::ToolResult { result, is_error: false, .. } if result == "3")
    );
    assert_eq!(
        events[2],
        AgentEvent::AssistantText("You have 3 work cards.".into())
    );

    let captured = server.join().unwrap();
    assert_eq!(captured.len(), 2);
    assert_eq!(
        captured[0].request_line,
        "POST /v1/chat/completions HTTP/1.1"
    );
    assert!(
        captured[0]
            .headers
            .iter()
            .any(|h| h.eq_ignore_ascii_case("authorization: Bearer test-key"))
    );
    assert_eq!(
        captured[0].body["tools"][0]["function"]["name"],
        "count_cards"
    );
    let second = &captured[1].body["messages"];
    assert_eq!(
        second[0],
        json!({"role":"system","content":"You manage cards."})
    );
    assert_eq!(
        second[2]["tool_calls"][0]["function"]["arguments"],
        "{\"tag\":\"work\"}"
    );
    assert_eq!(
        second[3],
        json!({"role":"tool","tool_call_id":"call_1","content":"3"})
    );
}

#[test]
fn step_limit_is_reported() {
    let tool_reply = |id: &str| {
        json!({"choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
            "tool_calls":[{"id":id,"type":"function",
                "function":{"name":"count_cards","arguments":"{}"}}]}}]})
    };
    let (url, server) = serve(vec![
        (200, tool_reply("call_1")),
        (200, tool_reply("call_2")),
    ]);
    let cfg = config(url);
    let tools = [ToolSpec::new(
        "count_cards",
        "Count cards",
        json!({"type":"object","properties":{}}),
    )];
    let mut history = vec![Message::user("Count forever")];
    let cancel = AtomicBool::new(false);
    let outcome = run_agent(
        &cfg,
        "",
        &mut history,
        &tools,
        &mut |_| Ok("1".to_owned()),
        &mut |_| {},
        2,
        &cancel,
    )
    .unwrap();
    assert_eq!(outcome, AgentOutcome::StepLimit);
    // Every tool call still got its result.
    assert_eq!(history.last().map(|m| m.role), Some(Role::Tool));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn http_errors_are_parsed_and_scrubbed() {
    let (url, server) = serve(vec![(
        401,
        json!({"error":{"message":"Incorrect API key provided: test-key","type":"invalid_request_error"}}),
    )]);
    let cfg = config(url);
    let err = cardfile_ai::chat(&cfg, "", &[Message::user("hi")], &[]).unwrap_err();
    assert_eq!(
        err,
        AiError::Http {
            status: 401,
            message: "Incorrect API key provided: <redacted>".into()
        }
    );
    assert!(!err.to_string().contains("test-key"));
    server.join().unwrap();
}

#[test]
fn cancelled_before_start_sends_nothing() {
    let cfg = config("http://127.0.0.1:9/v1".into());
    let mut history = vec![Message::user("hi")];
    let cancel = AtomicBool::new(true);
    let result = run_agent(
        &cfg,
        "",
        &mut history,
        &[],
        &mut |_| Ok(String::new()),
        &mut |_| {},
        3,
        &cancel,
    );
    assert_eq!(result, Err(AiError::Cancelled));
    assert_eq!(history.len(), 1);
}

#[test]
fn list_models_reads_openai_shape() {
    let (url, server) = serve(vec![(
        200,
        json!({"object":"list","data":[{"id":"b-model"},{"id":"a-model"},{"id":"a-model"}]}),
    )]);
    let models = cardfile_ai::list_models(&config(url)).unwrap();
    assert_eq!(models, vec!["a-model", "b-model"]);
    let captured = server.join().unwrap();
    assert_eq!(captured[0].request_line, "GET /v1/models HTTP/1.1");
}
