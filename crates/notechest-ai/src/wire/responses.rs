//! OpenAI Responses API (also Perplexity's Agent API).
//!
//! The conversation is sent statelessly as `input` items each turn.

use serde_json::{Map, Value, json};

use super::{WireRequest, arguments_string, count, new_call_id, parse_arguments, system_text};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::base;
use crate::types::{ChatResponse, FinishReason, Message, Role, ToolCall, ToolSpec, Usage};

/// Build a Responses request.
pub(crate) fn build(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> WireRequest {
    let mut input = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {}
            Role::User => input.push(json!({"role": "user", "content": m.content})),
            Role::Assistant => {
                if !m.content.is_empty() {
                    input.push(json!({"role": "assistant", "content": m.content}));
                }
                for c in &m.tool_calls {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": c.id,
                        "name": c.name,
                        "arguments": arguments_string(&c.arguments),
                    }));
                }
            }
            Role::Tool => input.push(json!({
                "type": "function_call_output",
                "call_id": m.tool_call_id.clone().unwrap_or_default(),
                "output": m.content,
            })),
        }
    }

    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model.trim()));
    let sys = system_text(system, messages);
    if !sys.is_empty() {
        body.insert("instructions".into(), json!(sys));
    }
    body.insert("input".into(), Value::Array(input));
    if !tools.is_empty() {
        let specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();
        body.insert("tools".into(), Value::Array(specs));
    }
    if let Some(t) = cfg.temperature {
        body.insert("temperature".into(), super::temperature(t));
    }
    if let Some(n) = max_tokens.or(cfg.max_tokens) {
        body.insert("max_output_tokens".into(), json!(n));
    }
    WireRequest {
        url: format!("{}/responses", base(cfg)),
        body: Value::Object(body),
    }
}

/// Parse a Responses reply.
pub(crate) fn parse(v: &Value) -> Result<ChatResponse, AiError> {
    let output = v
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| AiError::Decode("response has no output".into()))?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut refused = false;
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            if let Some(t) = part.get("text").and_then(Value::as_str) {
                                text.push_str(t);
                            }
                        }
                        Some("refusal") => {
                            refused = true;
                            if let Some(t) = part.get("refusal").and_then(Value::as_str) {
                                text.push_str(t);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("function_call") => {
                let Some(name) = item.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let id = item
                    .get("call_id")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map_or_else(new_call_id, str::to_owned);
                tool_calls.push(ToolCall {
                    id,
                    name: name.to_owned(),
                    arguments: parse_arguments(item.get("arguments")),
                    signature: None,
                });
            }
            _ => {}
        }
    }
    let incomplete = v
        .get("incomplete_details")
        .and_then(|d| d.get("reason"))
        .and_then(Value::as_str);
    let finish = if !tool_calls.is_empty() {
        FinishReason::ToolCalls
    } else if refused {
        FinishReason::ContentFilter
    } else {
        match incomplete {
            None => FinishReason::Stop,
            Some("max_output_tokens") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            Some(other) => FinishReason::Other(other.to_owned()),
        }
    };
    let usage = v.get("usage").filter(|u| u.is_object()).map(|u| Usage {
        input_tokens: count(u, "input_tokens"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::preset;

    #[test]
    fn builds_input_items() {
        let cfg = ProviderConfig::from_preset(preset("perplexity").unwrap());
        let call = ToolCall {
            id: "fc_1".into(),
            name: "search".into(),
            arguments: json!({"q":"a"}),
            signature: None,
        };
        let history = vec![
            Message::user("hi"),
            Message {
                tool_calls: vec![call.clone()],
                ..Message::assistant("Looking")
            },
            Message::tool_result(&call, "found", false),
        ];
        let tools = [ToolSpec::new("search", "d", json!({"type":"object"}))];
        let req = build(&cfg, "sys", &history, &tools, Some(50));
        assert_eq!(req.url, "https://api.perplexity.ai/v1/responses");
        assert_eq!(
            req.body,
            json!({
                "model": "perplexity/sonar",
                "instructions": "sys",
                "input": [
                    {"role":"user","content":"hi"},
                    {"role":"assistant","content":"Looking"},
                    {"type":"function_call","call_id":"fc_1","name":"search","arguments":"{\"q\":\"a\"}"},
                    {"type":"function_call_output","call_id":"fc_1","output":"found"}
                ],
                "tools": [{"type":"function","name":"search","description":"d","parameters":{"type":"object"}}],
                "max_output_tokens": 50
            })
        );
    }

    #[test]
    fn parses_output() {
        let v = json!({
            "output": [
                {"type":"reasoning","summary":[]},
                {"type":"message","role":"assistant","content":[{"type":"output_text","text":"Hi"}]},
                {"type":"function_call","id":"fc_x","call_id":"call_9","name":"search","arguments":"{\"q\":\"b\"}"}
            ],
            "usage": {"input_tokens": 7, "output_tokens": 2}
        });
        let r = parse(&v).unwrap();
        assert_eq!(r.message.content, "Hi");
        assert_eq!(r.finish, FinishReason::ToolCalls);
        assert_eq!(r.message.tool_calls[0].id, "call_9");
        assert_eq!(r.message.tool_calls[0].arguments, json!({"q":"b"}));
        assert_eq!(r.usage.map(|u| u.output_tokens), Some(2));

        let v = json!({"output":[],"incomplete_details":{"reason":"max_output_tokens"}});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::Length);
        assert!(parse(&json!({"id":"x"})).is_err());
    }
}
