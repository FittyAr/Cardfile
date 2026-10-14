//! Request building and response parsing for each wire format.
//!
//! Each module is pure (no I/O) so it can be tested with canned JSON.

pub(crate) mod anthropic;
pub(crate) mod gemini;
pub(crate) mod openai;
pub(crate) mod responses;

use serde_json::{Map, Value};

use crate::types::{Message, Role};

/// Output token limit used when the API requires one and the user set none.
pub(crate) const DEFAULT_MAX_TOKENS: u32 = 8192;

/// A ready-to-send request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WireRequest {
    pub url: String,
    pub body: Value,
}

/// Generate a tool-call id for providers that do not send one. Nine
/// alphanumeric characters satisfy the strictest provider (Mistral).
pub(crate) fn new_call_id() -> String {
    let simple = uuid::Uuid::now_v7().simple().to_string();
    let tail = simple
        .get(simple.len().saturating_sub(9)..)
        .unwrap_or(&simple);
    tail.to_owned()
}

/// Normalize tool-call arguments that may arrive as a JSON string or a value.
pub(crate) fn parse_arguments(raw: Option<&Value>) -> Value {
    match raw {
        None | Some(Value::Null) => Value::Object(Map::new()),
        Some(Value::String(s)) if s.trim().is_empty() => Value::Object(Map::new()),
        Some(Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        Some(v) => v.clone(),
    }
}

/// Arguments as the JSON string OpenAI-style APIs expect.
pub(crate) fn arguments_string(args: &Value) -> String {
    match args {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Arguments as a JSON object (Anthropic, Gemini).
pub(crate) fn arguments_object(args: &Value) -> Value {
    match args {
        Value::Object(_) => args.clone(),
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v @ Value::Object(_)) => v,
            _ => Value::Object(Map::new()),
        },
        _ => Value::Object(Map::new()),
    }
}

/// The system prompt plus any system-role messages in the history.
pub(crate) fn system_text(system: &str, messages: &[Message]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if !system.trim().is_empty() {
        parts.push(system);
    }
    parts.extend(
        messages
            .iter()
            .filter(|m| m.role == Role::System && !m.content.trim().is_empty())
            .map(|m| m.content.as_str()),
    );
    parts.join("\n\n")
}

/// Append `obj` to `out`, merging its parts into the previous entry when both
/// have the same role (Anthropic and Gemini require alternating roles).
pub(crate) fn push_merged(out: &mut Vec<Value>, role: &str, parts_key: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.get("role").and_then(Value::as_str) == Some(role)
        && let Some(existing) = last.get_mut(parts_key).and_then(Value::as_array_mut)
    {
        existing.extend(parts);
        return;
    }
    let mut obj = Map::new();
    obj.insert("role".to_owned(), Value::String(role.to_owned()));
    obj.insert(parts_key.to_owned(), Value::Array(parts));
    out.push(Value::Object(obj));
}

/// Temperature as JSON without `f32` → `f64` noise (0.2 stays 0.2, not
/// 0.20000000298…).
pub(crate) fn temperature(t: f32) -> Value {
    t.to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(Value::Null, Value::Number)
}

/// Read a token count field as `u64`.
pub(crate) fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn arguments_normalization() {
        assert_eq!(parse_arguments(Some(&json!("{\"a\":1}"))), json!({"a":1}));
        assert_eq!(parse_arguments(Some(&json!({"a":1}))), json!({"a":1}));
        assert_eq!(parse_arguments(Some(&json!(""))), json!({}));
        assert_eq!(parse_arguments(None), json!({}));
        assert_eq!(parse_arguments(Some(&json!("not json"))), json!("not json"));
        assert_eq!(arguments_string(&json!({"a":1})), "{\"a\":1}");
        assert_eq!(arguments_string(&json!("raw")), "raw");
        assert_eq!(arguments_object(&json!("{\"a\":1}")), json!({"a":1}));
        assert_eq!(arguments_object(&json!([1])), json!({}));
    }

    #[test]
    fn call_ids_are_short_alphanumeric() {
        let a = new_call_id();
        assert_eq!(a.len(), 9);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn merges_same_role() {
        let mut out = Vec::new();
        push_merged(&mut out, "user", "content", vec![json!(1)]);
        push_merged(&mut out, "user", "content", vec![json!(2)]);
        push_merged(&mut out, "assistant", "content", vec![json!(3)]);
        push_merged(&mut out, "assistant", "content", vec![]);
        assert_eq!(
            Value::Array(out),
            json!([{"role":"user","content":[1,2]},{"role":"assistant","content":[3]}])
        );
    }

    #[test]
    fn system_text_merges_system_messages() {
        let msgs = [Message::system("extra"), Message::user("hi")];
        assert_eq!(system_text("base", &msgs), "base\n\nextra");
        assert_eq!(system_text("", &msgs), "extra");
    }
}
