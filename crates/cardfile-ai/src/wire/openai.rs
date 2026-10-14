//! OpenAI Chat Completions, also used for Azure OpenAI and compatible servers.

use serde_json::{Map, Value, json};

use super::{WireRequest, arguments_string, count, new_call_id, parse_arguments, system_text};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::{base, encode_segment};
use crate::preset::ApiKind;
use crate::types::{ChatResponse, FinishReason, Message, Role, ToolCall, ToolSpec, Usage};

/// Chat Completions endpoint for `cfg` (plain or Azure).
pub(crate) fn endpoint(cfg: &ProviderConfig) -> String {
    let b = base(cfg);
    if cfg.kind != ApiKind::AzureOpenAi {
        return format!("{b}/chat/completions");
    }
    if b.ends_with("/openai/v1") {
        return format!("{b}/chat/completions");
    }
    match cfg.api_version.as_deref().map(str::trim) {
        Some(v) if !v.is_empty() && v != "v1" => format!(
            "{b}/openai/deployments/{}/chat/completions?api-version={}",
            encode_segment(cfg.model.trim()),
            encode_segment(v)
        ),
        _ => format!("{b}/openai/v1/chat/completions"),
    }
}

/// OpenAI's own API and Azure reject `max_tokens` for current (reasoning)
/// models and want `max_completion_tokens`; other servers only know `max_tokens`.
fn max_tokens_key(cfg: &ProviderConfig) -> &'static str {
    if cfg.kind == ApiKind::AzureOpenAi || base(cfg).contains("api.openai.com") {
        "max_completion_tokens"
    } else {
        "max_tokens"
    }
}

fn message_json(m: &Message) -> Option<Value> {
    match m.role {
        Role::System => None,
        Role::User => Some(json!({"role": "user", "content": m.content})),
        Role::Assistant => {
            let mut obj = Map::new();
            obj.insert("role".into(), json!("assistant"));
            if m.tool_calls.is_empty() || !m.content.is_empty() {
                obj.insert("content".into(), json!(m.content));
            } else {
                obj.insert("content".into(), Value::Null);
            }
            if !m.tool_calls.is_empty() {
                let calls: Vec<Value> = m
                    .tool_calls
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.id,
                            "type": "function",
                            "function": {"name": c.name, "arguments": arguments_string(&c.arguments)}
                        })
                    })
                    .collect();
                obj.insert("tool_calls".into(), Value::Array(calls));
            }
            Some(Value::Object(obj))
        }
        Role::Tool => Some(json!({
            "role": "tool",
            "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
            "content": m.content,
        })),
    }
}

/// Build a Chat Completions request.
pub(crate) fn build(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> WireRequest {
    let mut msgs = Vec::new();
    let sys = system_text(system, messages);
    if !sys.is_empty() {
        msgs.push(json!({"role": "system", "content": sys}));
    }
    msgs.extend(messages.iter().filter_map(message_json));

    let mut body = Map::new();
    body.insert("model".into(), json!(cfg.model.trim()));
    body.insert("messages".into(), Value::Array(msgs));
    if !tools.is_empty() {
        let specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {"name": t.name, "description": t.description, "parameters": t.parameters}
                })
            })
            .collect();
        body.insert("tools".into(), Value::Array(specs));
    }
    if let Some(t) = cfg.temperature {
        body.insert("temperature".into(), super::temperature(t));
    }
    if let Some(n) = max_tokens.or(cfg.max_tokens) {
        body.insert(max_tokens_key(cfg).into(), json!(n));
    }
    WireRequest {
        url: endpoint(cfg),
        body: Value::Object(body),
    }
}

/// Text from a `content` that is a string or an array of text parts.
fn content_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Parse a Chat Completions response.
pub(crate) fn parse(v: &Value) -> Result<ChatResponse, AiError> {
    let choice = v
        .get("choices")
        .and_then(|c| c.get(0))
        .ok_or_else(|| AiError::Decode("response has no choices".into()))?;
    let msg = choice
        .get("message")
        .ok_or_else(|| AiError::Decode("choice has no message".into()))?;
    let tool_calls: Vec<ToolCall> = msg
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(|c| {
                    let f = c.get("function")?;
                    let name = f.get("name")?.as_str()?.to_owned();
                    let id = c
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map_or_else(new_call_id, str::to_owned);
                    Some(ToolCall {
                        id,
                        name,
                        arguments: parse_arguments(f.get("arguments")),
                        signature: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let finish = match choice.get("finish_reason").and_then(Value::as_str) {
        // Some servers (e.g. Ollama) report "stop" even when calling tools.
        _ if !tool_calls.is_empty() => FinishReason::ToolCalls,
        Some("stop") | None => FinishReason::Stop,
        Some("length") => FinishReason::Length,
        Some("content_filter") => FinishReason::ContentFilter,
        Some("tool_calls" | "function_call") => FinishReason::ToolCalls,
        Some(other) => FinishReason::Other(other.to_owned()),
    };
    let usage = v.get("usage").filter(|u| u.is_object()).map(|u| Usage {
        input_tokens: count(u, "prompt_tokens"),
        output_tokens: count(u, "completion_tokens"),
    });
    Ok(ChatResponse {
        message: Message {
            tool_calls,
            ..Message::assistant(content_text(msg.get("content")))
        },
        finish,
        usage,
    })
}

/// URL that lists models, if the API supports it.
pub(crate) fn models_url(cfg: &ProviderConfig) -> Option<String> {
    (cfg.kind != ApiKind::AzureOpenAi).then(|| format!("{}/models", base(cfg)))
}

/// Parse a `GET /models` reply (`{"data":[{"id":..}]}`).
pub(crate) fn parse_models(v: &Value) -> Result<Vec<String>, AiError> {
    let data = v
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| AiError::Decode("model list has no data".into()))?;
    Ok(data
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::preset;

    fn cfg(id: &str) -> ProviderConfig {
        ProviderConfig::from_preset(preset(id).unwrap())
    }

    fn tool() -> ToolSpec {
        ToolSpec::new(
            "search",
            "Search cards",
            json!({"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}),
        )
    }

    #[test]
    fn builds_request_with_tool_round_trip() {
        let call = ToolCall {
            id: "call_1".into(),
            name: "search".into(),
            arguments: json!({"q":"rust"}),
            signature: None,
        };
        let history = vec![
            Message::user("find rust"),
            Message {
                tool_calls: vec![call.clone()],
                ..Message::assistant("")
            },
            Message::tool_result(&call, "2 cards", false),
        ];
        let mut c = cfg("mistral");
        c.temperature = Some(0.5);
        c.max_tokens = Some(100);
        let req = build(&c, "be brief", &history, &[tool()], None);
        assert_eq!(req.url, "https://api.mistral.ai/v1/chat/completions");
        assert_eq!(
            req.body,
            json!({
                "model": "mistral-large-latest",
                "messages": [
                    {"role":"system","content":"be brief"},
                    {"role":"user","content":"find rust"},
                    {"role":"assistant","content":null,"tool_calls":[
                        {"id":"call_1","type":"function","function":{"name":"search","arguments":"{\"q\":\"rust\"}"}}
                    ]},
                    {"role":"tool","tool_call_id":"call_1","content":"2 cards"}
                ],
                "tools": [{"type":"function","function":{"name":"search","description":"Search cards",
                    "parameters":{"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}}}],
                "temperature": 0.5,
                "max_tokens": 100
            })
        );
    }

    #[test]
    fn openai_uses_max_completion_tokens_and_omits_empty_tools() {
        let req = build(&cfg("openai"), "", &[Message::user("hi")], &[], Some(16));
        assert_eq!(req.body["max_completion_tokens"], json!(16));
        assert!(req.body.get("max_tokens").is_none());
        assert!(req.body.get("tools").is_none());
        assert_eq!(
            req.body["messages"],
            json!([{"role":"user","content":"hi"}])
        );
    }

    #[test]
    fn azure_endpoints() {
        let mut c = cfg("azure");
        c.base_url = "https://res.openai.azure.com/".into();
        c.model = "my deploy".into();
        assert_eq!(
            endpoint(&c),
            "https://res.openai.azure.com/openai/deployments/my%20deploy/chat/completions?api-version=2024-10-21"
        );
        c.api_version = Some("v1".into());
        assert_eq!(
            endpoint(&c),
            "https://res.openai.azure.com/openai/v1/chat/completions"
        );
        c.api_version = None;
        c.base_url = "https://res.openai.azure.com/openai/v1".into();
        assert_eq!(
            endpoint(&c),
            "https://res.openai.azure.com/openai/v1/chat/completions"
        );
        assert!(models_url(&c).is_none());
    }

    #[test]
    fn parses_multiple_tool_calls_with_string_and_object_args() {
        let v = json!({
            "choices":[{"finish_reason":"tool_calls","message":{"role":"assistant","content":null,
                "tool_calls":[
                    {"id":"a","type":"function","function":{"name":"search","arguments":"{\"q\":\"x\"}"}},
                    {"type":"function","function":{"name":"open","arguments":{"id":5}}}
                ]}}],
            "usage":{"prompt_tokens":10,"completion_tokens":3}
        });
        let r = parse(&v).unwrap();
        assert_eq!(r.finish, FinishReason::ToolCalls);
        assert_eq!(r.message.role, Role::Assistant);
        assert_eq!(r.message.tool_calls.len(), 2);
        assert_eq!(r.message.tool_calls[0].arguments, json!({"q":"x"}));
        assert_eq!(r.message.tool_calls[1].arguments, json!({"id":5}));
        assert_eq!(r.message.tool_calls[1].id.len(), 9);
        assert_eq!(
            r.usage,
            Some(Usage {
                input_tokens: 10,
                output_tokens: 3
            })
        );
    }

    #[test]
    fn parses_text_and_ollama_stop_with_tools() {
        let v = json!({"choices":[{"finish_reason":"stop","message":{"content":"Hello"}}]});
        let r = parse(&v).unwrap();
        assert_eq!(r.message.content, "Hello");
        assert_eq!(r.finish, FinishReason::Stop);
        assert!(r.usage.is_none());

        let v = json!({"choices":[{"finish_reason":"stop","message":{"content":"",
            "tool_calls":[{"id":"x","function":{"name":"f","arguments":"{}"}}]}}]});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::ToolCalls);

        let v = json!({"choices":[{"finish_reason":"length","message":{"content":[{"type":"text","text":"a"},{"type":"text","text":"b"}]}}]});
        let r = parse(&v).unwrap();
        assert_eq!(r.message.content, "ab");
        assert_eq!(r.finish, FinishReason::Length);

        assert!(matches!(parse(&json!({})), Err(AiError::Decode(_))));
    }

    #[test]
    fn parses_model_list() {
        let v = json!({"object":"list","data":[{"id":"a"},{"id":"b"}]});
        assert_eq!(parse_models(&v).unwrap(), vec!["a", "b"]);
    }
}
