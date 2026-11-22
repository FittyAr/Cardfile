//! Anthropic Messages API.

use serde_json::{Map, Value, json};

use super::{
    DEFAULT_MAX_TOKENS, WireRequest, arguments_object, count, new_call_id, push_merged, system_text,
};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::base;
use crate::types::{ChatResponse, FinishReason, Message, Role, ToolCall, ToolSpec, Usage};

/// Build a Messages request. Tool results become `tool_result` blocks in a
/// user turn and consecutive same-role turns are merged, as the API requires.
pub(crate) fn build(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> WireRequest {
    let mut msgs: Vec<Value> = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {}
            Role::User => {
                let mut blocks = Vec::new();
                if !m.content.trim().is_empty() {
                    blocks.push(json!({"type": "text", "text": m.content}));
                }
                push_merged(&mut msgs, "user", "content", blocks);
            }
            Role::Assistant => {
                let mut blocks = Vec::new();
                if !m.content.trim().is_empty() {
                    blocks.push(json!({"type": "text", "text": m.content}));
                }
                for c in &m.tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": c.id,
                        "name": c.name,
                        "input": arguments_object(&c.arguments),
                    }));
                }
                push_merged(&mut msgs, "assistant", "content", blocks);
            }
            Role::Tool => {
                let mut block = json!({
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content,
                });
                if m.is_error {
                    block["is_error"] = json!(true);
                }
                push_merged(&mut msgs, "user", "content", vec![block]);
            }
        }
    }

    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model.trim()));
    body.insert(
        "max_tokens".into(),
        json!(max_tokens.or(cfg.max_tokens).unwrap_or(DEFAULT_MAX_TOKENS)),
    );
    let sys = system_text(system, messages);
    if !sys.is_empty() {
        body.insert("system".into(), json!(sys));
    }
    body.insert("messages".into(), Value::Array(msgs));
    if !tools.is_empty() {
        let specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({"name": t.name, "description": t.description, "input_schema": t.parameters})
            })
            .collect();
        body.insert("tools".into(), Value::Array(specs));
    }
    if let Some(t) = cfg.temperature {
        body.insert("temperature".into(), super::temperature(t));
    }
    WireRequest {
        url: format!("{}/messages", base(cfg)),
        body: Value::Object(body),
    }
}

/// Parse a Messages response.
pub(crate) fn parse(v: &Value) -> Result<ChatResponse, AiError> {
    let content = v
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| AiError::Decode("response has no content".into()))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                let Some(name) = block.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let id = block
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map_or_else(new_call_id, str::to_owned);
                tool_calls.push(ToolCall {
                    id,
                    name: name.to_owned(),
                    arguments: block.get("input").cloned().unwrap_or_else(|| json!({})),
                    signature: None,
                });
            }
            _ => {}
        }
    }
    let finish = match v.get("stop_reason").and_then(Value::as_str) {
        _ if !tool_calls.is_empty() => FinishReason::ToolCalls,
        Some("end_turn" | "stop_sequence") | None => FinishReason::Stop,
        Some("tool_use") => FinishReason::ToolCalls,
        Some("max_tokens" | "model_context_window_exceeded") => FinishReason::Length,
        Some("refusal") => FinishReason::ContentFilter,
        Some(other) => FinishReason::Other(other.to_owned()),
    };
    let usage = v.get("usage").filter(|u| u.is_object()).map(|u| Usage {
        input_tokens: count(u, "input_tokens")
            + count(u, "cache_read_input_tokens")
            + count(u, "cache_creation_input_tokens"),
        output_tokens: count(u, "output_tokens"),
    });
    Ok(ChatResponse {
        message: Message {
            tool_calls,
            ..Message::assistant(text)
        },
        finish,
        usage,
    })
}

/// URL that lists models.
pub(crate) fn models_url(cfg: &ProviderConfig) -> String {
    format!("{}/models?limit=1000", base(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::preset;

    #[test]
    fn builds_request_with_merged_tool_results() {
        let mut cfg = ProviderConfig::from_preset(preset("anthropic").unwrap());
        cfg.temperature = Some(0.2);
        let a = ToolCall {
            id: "toolu_a".into(),
            name: "search".into(),
            arguments: json!("{\"q\":\"x\"}"),
            signature: None,
        };
        let b = ToolCall {
            id: "toolu_b".into(),
            name: "open".into(),
            arguments: json!({"id":1}),
            signature: None,
        };
        let history = vec![
            Message::system("extra rules"),
            Message::user("go"),
            Message {
                tool_calls: vec![a.clone(), b.clone()],
                ..Message::assistant("On it")
            },
            Message::tool_result(&a, "r1", false),
            Message::tool_result(&b, "boom", true),
            Message::user("and?"),
        ];
        let tools = [ToolSpec::new("search", "d", json!({"type":"object"}))];
        let req = build(&cfg, "sys", &history, &tools, None);
        assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(
            req.body,
            json!({
                "model": "claude-sonnet-5-5",
                "max_tokens": 8192,
                "system": "sys\n\nextra rules",
                "temperature": 0.2,
                "tools": [{"name":"search","description":"d","input_schema":{"type":"object"}}],
                "messages": [
                    {"role":"user","content":[{"type":"text","text":"go"}]},
                    {"role":"assistant","content":[
                        {"type":"text","text":"On it"},
                        {"type":"tool_use","id":"toolu_a","name":"search","input":{"q":"x"}},
                        {"type":"tool_use","id":"toolu_b","name":"open","input":{"id":1}}
                    ]},
                    {"role":"user","content":[
                        {"type":"tool_result","tool_use_id":"toolu_a","content":"r1"},
                        {"type":"tool_result","tool_use_id":"toolu_b","content":"boom","is_error":true},
                        {"type":"text","text":"and?"}
                    ]}
                ]
            })
        );
    }

    #[test]
    fn parses_tool_use_response() {
        let v = json!({
            "id":"msg_1","type":"message","role":"assistant",
            "content":[
                {"type":"text","text":"Let me check."},
                {"type":"tool_use","id":"toolu_1","name":"search","input":{"q":"rust"}},
                {"type":"tool_use","id":"toolu_2","name":"search","input":{"q":"gpui"}}
            ],
            "stop_reason":"tool_use",
            "usage":{"input_tokens":20,"output_tokens":9,"cache_read_input_tokens":5}
        });
        let r = parse(&v).unwrap();
        assert_eq!(r.message.content, "Let me check.");
        assert_eq!(r.finish, FinishReason::ToolCalls);
        assert_eq!(r.message.tool_calls.len(), 2);
        assert_eq!(r.message.tool_calls[1].arguments, json!({"q":"gpui"}));
        assert_eq!(
            r.usage,
            Some(Usage {
                input_tokens: 25,
                output_tokens: 9
            })
        );
    }

    #[test]
    fn parses_stop_reasons() {
        let v = json!({"content":[{"type":"text","text":"x"}],"stop_reason":"max_tokens"});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::Length);
        let v = json!({"content":[],"stop_reason":"refusal"});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::ContentFilter);
        let v = json!({"content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn"});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::Stop);
        assert!(parse(&json!({"type":"error"})).is_err());
    }
}
