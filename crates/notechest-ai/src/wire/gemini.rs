//! Google Gemini `generateContent`.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use super::{
    WireRequest, arguments_object, count, new_call_id, parse_arguments, push_merged, system_text,
};
use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::http::{base, encode_segment};
use crate::types::{ChatResponse, FinishReason, Message, Role, ToolCall, ToolSpec, Usage};

/// Schema keywords Gemini's `parameters` (an OpenAPI subset) accepts.
/// Anything else (`additionalProperties`, `$schema`, `$ref`, `default`, …)
/// makes the request fail, so it is dropped.
const ALLOWED_KEYS: &[&str] = &[
    "type",
    "format",
    "title",
    "description",
    "nullable",
    "enum",
    "items",
    "properties",
    "required",
    "anyOf",
    "minItems",
    "maxItems",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "minProperties",
    "maxProperties",
    "propertyOrdering",
];

/// Reduce a JSON Schema to the subset Gemini accepts.
pub(crate) fn sanitize_schema(schema: &Value) -> Value {
    let Value::Object(obj) = schema else {
        return schema.clone();
    };
    let mut out = Map::new();
    for (key, value) in obj {
        match key.as_str() {
            "properties" => {
                if let Value::Object(props) = value {
                    let props: Map<String, Value> = props
                        .iter()
                        .map(|(name, s)| (name.clone(), sanitize_schema(s)))
                        .collect();
                    out.insert(key.clone(), Value::Object(props));
                }
            }
            "items" => {
                out.insert(key.clone(), sanitize_schema(value));
            }
            "anyOf" => {
                if let Value::Array(list) = value {
                    out.insert(
                        key.clone(),
                        Value::Array(list.iter().map(sanitize_schema).collect()),
                    );
                }
            }
            // JSON Schema allows `"type": ["string", "null"]`; Gemini wants a
            // single type plus `nullable`.
            "type" => match value {
                Value::Array(types) => {
                    let mut non_null = types.iter().filter(|t| t.as_str() != Some("null"));
                    if let Some(t) = non_null.next() {
                        out.insert(key.clone(), t.clone());
                    }
                    if types.iter().any(|t| t.as_str() == Some("null")) {
                        out.insert("nullable".into(), Value::Bool(true));
                    }
                }
                other => {
                    out.insert(key.clone(), other.clone());
                }
            },
            "const" => {
                out.insert("enum".into(), Value::Array(vec![value.clone()]));
            }
            // Gemini only accepts string enums.
            "enum" => {
                if let Value::Array(list) = value {
                    let list = list
                        .iter()
                        .map(|v| match v {
                            Value::String(_) => v.clone(),
                            other => Value::String(other.to_string()),
                        })
                        .collect();
                    out.insert(key.clone(), Value::Array(list));
                }
            }
            k if ALLOWED_KEYS.contains(&k) => {
                out.insert(key.clone(), value.clone());
            }
            _ => {}
        }
    }
    // `required` may only name declared properties.
    if let (Some(Value::Array(req)), Some(Value::Object(props))) =
        (out.get("required"), out.get("properties"))
    {
        let kept: Vec<Value> = req
            .iter()
            .filter(|r| r.as_str().is_some_and(|n| props.contains_key(n)))
            .cloned()
            .collect();
        out.insert("required".into(), Value::Array(kept));
    }
    Value::Object(out)
}

/// Function declaration for `tool`; parameters are omitted when the tool
/// takes none, because Gemini rejects an object schema without properties.
fn declaration(tool: &ToolSpec) -> Value {
    let mut decl = json!({"name": tool.name, "description": tool.description});
    let params = sanitize_schema(&tool.parameters);
    let has_props = params
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|p| !p.is_empty());
    if has_props {
        decl["parameters"] = params;
    }
    decl
}

fn model_path(model: &str) -> String {
    let m = model.trim();
    encode_segment(m.strip_prefix("models/").unwrap_or(m))
}

/// Build a `generateContent` request. The model's turns use role `model`;
/// function responses go in `user` turns; same-role turns are merged.
pub(crate) fn build(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> WireRequest {
    // Gemini needs the function name in each response; look it up by id when
    // the tool message lacks it.
    let names: HashMap<&str, &str> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter())
        .map(|c| (c.id.as_str(), c.name.as_str()))
        .collect();

    let mut contents: Vec<Value> = Vec::new();
    for m in messages {
        match m.role {
            Role::System => {}
            Role::User => {
                let mut parts = Vec::new();
                if !m.content.is_empty() {
                    parts.push(json!({"text": m.content}));
                }
                push_merged(&mut contents, "user", "parts", parts);
            }
            Role::Assistant => {
                let mut parts = Vec::new();
                if !m.content.is_empty() {
                    parts.push(json!({"text": m.content}));
                }
                for c in &m.tool_calls {
                    let mut part = json!({
                        "functionCall": {
                            "id": c.id,
                            "name": c.name,
                            "args": arguments_object(&c.arguments),
                        }
                    });
                    if let Some(sig) = &c.signature {
                        part["thoughtSignature"] = json!(sig);
                    }
                    parts.push(part);
                }
                push_merged(&mut contents, "model", "parts", parts);
            }
            Role::Tool => {
                let id = m.tool_call_id.clone().unwrap_or_default();
                let name = m
                    .tool_name
                    .clone()
                    .or_else(|| names.get(id.as_str()).map(|n| (*n).to_owned()))
                    .unwrap_or_default();
                let response = if m.is_error {
                    json!({"error": m.content})
                } else {
                    json!({"result": m.content})
                };
                let part = json!({
                    "functionResponse": {"id": id, "name": name, "response": response}
                });
                push_merged(&mut contents, "user", "parts", vec![part]);
            }
        }
    }

    let mut body = Map::new();
    let sys = system_text(system, messages);
    if !sys.is_empty() {
        body.insert(
            "systemInstruction".into(),
            json!({"parts": [{"text": sys}]}),
        );
    }
    body.insert("contents".into(), Value::Array(contents));
    if !tools.is_empty() {
        let decls: Vec<Value> = tools.iter().map(declaration).collect();
        body.insert("tools".into(), json!([{"functionDeclarations": decls}]));
    }
    let mut gen_cfg = Map::new();
    if let Some(t) = cfg.temperature {
        gen_cfg.insert("temperature".into(), super::temperature(t));
    }
    if let Some(n) = max_tokens.or(cfg.max_tokens) {
        gen_cfg.insert("maxOutputTokens".into(), json!(n));
    }
    if !gen_cfg.is_empty() {
        body.insert("generationConfig".into(), Value::Object(gen_cfg));
    }
    WireRequest {
        url: format!(
            "{}/models/{}:generateContent",
            base(cfg),
            model_path(&cfg.model)
        ),
        body: Value::Object(body),
    }
}

/// Parse a `generateContent` response.
pub(crate) fn parse(v: &Value) -> Result<ChatResponse, AiError> {
    let usage = v
        .get("usageMetadata")
        .filter(|u| u.is_object())
        .map(|u| Usage {
            input_tokens: count(u, "promptTokenCount"),
            output_tokens: count(u, "candidatesTokenCount") + count(u, "thoughtsTokenCount"),
        });
    let Some(candidate) = v.get("candidates").and_then(|c| c.get(0)) else {
        // The whole prompt was blocked.
        if v.get("promptFeedback")
            .and_then(|f| f.get("blockReason"))
            .is_some()
        {
            return Ok(ChatResponse {
                message: Message::assistant(""),
                finish: FinishReason::ContentFilter,
                usage,
            });
        }
        return Err(AiError::Decode("response has no candidates".into()));
    };
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let parts = candidate
        .get("content")
        .and_then(|c| c.get("parts"))
        .and_then(Value::as_array);
    for part in parts.into_iter().flatten() {
        if let Some(call) = part.get("functionCall") {
            let Some(name) = call.get("name").and_then(Value::as_str) else {
                continue;
            };
            let id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map_or_else(new_call_id, str::to_owned);
            tool_calls.push(ToolCall {
                id,
                name: name.to_owned(),
                arguments: parse_arguments(call.get("args")),
                signature: part
                    .get("thoughtSignature")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        } else if part.get("thought").and_then(Value::as_bool) != Some(true)
            && let Some(t) = part.get("text").and_then(Value::as_str)
        {
            text.push_str(t);
        }
    }
    let finish = match candidate.get("finishReason").and_then(Value::as_str) {
        _ if !tool_calls.is_empty() => FinishReason::ToolCalls,
        Some("STOP") | None => FinishReason::Stop,
        Some("MAX_TOKENS") => FinishReason::Length,
        Some("SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII") => {
            FinishReason::ContentFilter
        }
        Some(other) => FinishReason::Other(other.to_owned()),
    };
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
    format!("{}/models?pageSize=1000", base(cfg))
}

/// Parse a model list, keeping models that support `generateContent`.
pub(crate) fn parse_models(v: &Value) -> Result<Vec<String>, AiError> {
    let models = v
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| AiError::Decode("model list has no models".into()))?;
    Ok(models
        .iter()
        .filter(|m| {
            m.get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .is_none_or(|methods| {
                    methods
                        .iter()
                        .any(|x| x.as_str() == Some("generateContent"))
                })
        })
        .filter_map(|m| m.get("name").and_then(Value::as_str))
        .map(|n| n.strip_prefix("models/").unwrap_or(n).to_owned())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::preset;

    #[test]
    fn sanitizes_schema() {
        let schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "q": {"type": "string", "default": "x", "examples": ["a"]},
                "tags": {"type": "array", "items": {"type": ["string", "null"], "additionalProperties": false}},
                "mode": {"const": "fast"},
                "level": {"type": "integer", "enum": [1, 2]},
                "additionalProperties": {"type": "boolean"}
            },
            "required": ["q", "missing"],
            "$defs": {"x": {}}
        });
        assert_eq!(
            sanitize_schema(&schema),
            json!({
                "type": "object",
                "properties": {
                    "q": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string", "nullable": true}},
                    "mode": {"enum": ["fast"]},
                    "level": {"type": "integer", "enum": ["1", "2"]},
                    "additionalProperties": {"type": "boolean"}
                },
                "required": ["q"]
            })
        );
    }

    #[test]
    fn builds_request() {
        let mut cfg = ProviderConfig::from_preset(preset("gemini").unwrap());
        cfg.max_tokens = Some(200);
        let a = ToolCall {
            id: "c1".into(),
            name: "search".into(),
            arguments: json!({"q":"a"}),
            signature: Some("sig==".into()),
        };
        let b = ToolCall {
            id: "c2".into(),
            name: "open".into(),
            arguments: json!({"id":2}),
            signature: None,
        };
        let mut result_b = Message::tool_result(&b, "nope", true);
        result_b.tool_name = None; // name recovered from the call
        let history = vec![
            Message::user("hi"),
            Message {
                tool_calls: vec![a.clone(), b.clone()],
                ..Message::assistant("")
            },
            Message::tool_result(&a, "ok", false),
            result_b,
        ];
        let tools = [
            ToolSpec::new(
                "search",
                "d",
                json!({"type":"object","additionalProperties":false,"properties":{"q":{"type":"string"}}}),
            ),
            ToolSpec::new("now", "time", json!({"type":"object","properties":{}})),
        ];
        let req = build(&cfg, "sys", &history, &tools, None);
        assert_eq!(
            req.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.8-flash:generateContent"
        );
        assert_eq!(
            req.body,
            json!({
                "systemInstruction": {"parts":[{"text":"sys"}]},
                "contents": [
                    {"role":"user","parts":[{"text":"hi"}]},
                    {"role":"model","parts":[
                        {"functionCall":{"id":"c1","name":"search","args":{"q":"a"}},"thoughtSignature":"sig=="},
                        {"functionCall":{"id":"c2","name":"open","args":{"id":2}}}
                    ]},
                    {"role":"user","parts":[
                        {"functionResponse":{"id":"c1","name":"search","response":{"result":"ok"}}},
                        {"functionResponse":{"id":"c2","name":"open","response":{"error":"nope"}}}
                    ]}
                ],
                "tools": [{"functionDeclarations":[
                    {"name":"search","description":"d","parameters":{"type":"object","properties":{"q":{"type":"string"}}}},
                    {"name":"now","description":"time"}
                ]}],
                "generationConfig": {"maxOutputTokens": 200}
            })
        );
    }

    #[test]
    fn parses_function_calls() {
        let v = json!({
            "candidates":[{"content":{"role":"model","parts":[
                {"text":"thinking...","thought":true},
                {"text":"Sure."},
                {"functionCall":{"name":"search","args":{"q":"x"}},"thoughtSignature":"abc"},
                {"functionCall":{"name":"search","args":{"q":"y"}}}
            ]},"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":4,"thoughtsTokenCount":2}
        });
        let r = parse(&v).unwrap();
        assert_eq!(r.message.content, "Sure.");
        assert_eq!(r.finish, FinishReason::ToolCalls);
        assert_eq!(r.message.tool_calls.len(), 2);
        assert_eq!(r.message.tool_calls[0].signature.as_deref(), Some("abc"));
        assert_ne!(r.message.tool_calls[0].id, r.message.tool_calls[1].id);
        assert_eq!(
            r.usage,
            Some(Usage {
                input_tokens: 11,
                output_tokens: 6
            })
        );
    }

    #[test]
    fn parses_blocked_and_limits() {
        let v = json!({"promptFeedback":{"blockReason":"SAFETY"}});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::ContentFilter);
        let v = json!({"candidates":[{"content":{"parts":[{"text":"a"}]},"finishReason":"MAX_TOKENS"}]});
        assert_eq!(parse(&v).unwrap().finish, FinishReason::Length);
        assert!(parse(&json!({})).is_err());
    }

    #[test]
    fn parses_model_list() {
        let v = json!({"models":[
            {"name":"models/gemini-3.8-flash","supportedGenerationMethods":["generateContent","countTokens"]},
            {"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]}
        ]});
        assert_eq!(parse_models(&v).unwrap(), vec!["gemini-3.8-flash"]);
    }
}
