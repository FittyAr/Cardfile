//! JSON-RPC 2.0 message classification and response building.

use serde_json::{Map, Value, json};

/// JSON-RPC and MCP error codes.
pub(crate) mod code {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
    /// MCP 2026-07-28: HTTP headers disagree with the body, or a required header is missing.
    pub const HEADER_MISMATCH: i64 = -32020;
    /// MCP 2026-07-28: the requested protocol version is not implemented.
    pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn parse(message: impl Into<String>) -> Self {
        Self::new(code::PARSE_ERROR, message)
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_REQUEST, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            code::METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(code::INVALID_PARAMS, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(code::INTERNAL_ERROR, message)
    }

    pub fn header_mismatch(message: impl Into<String>) -> Self {
        Self::new(
            code::HEADER_MISMATCH,
            format!("Header mismatch: {}", message.into()),
        )
    }

    pub fn unsupported_version(requested: &str) -> Self {
        Self {
            code: code::UNSUPPORTED_PROTOCOL_VERSION,
            message: "Unsupported protocol version".to_owned(),
            data: Some(json!({
                "supported": crate::version::supported_versions(),
                "requested": requested,
            })),
        }
    }

    fn to_value(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("code".into(), self.code.into());
        obj.insert("message".into(), self.message.clone().into());
        if let Some(data) = &self.data {
            obj.insert("data".into(), data.clone());
        }
        Value::Object(obj)
    }
}

/// A classified client-to-server message.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    Request {
        id: Value,
        method: String,
        params: Map<String, Value>,
    },
    Notification {
        method: String,
    },
    /// A response to a server-initiated request. This server never sends requests, so
    /// responses are accepted and ignored.
    Response,
}

/// Classifies one JSON-RPC message. On failure returns the request id (when it could be read)
/// together with the error to send back.
// The error is turned into a response right away; boxing it would only add noise.
#[allow(clippy::result_large_err)]
pub(crate) fn classify(value: &Value) -> Result<Message, (Option<Value>, RpcError)> {
    let Some(obj) = value.as_object() else {
        return Err((
            None,
            RpcError::invalid_request("Message must be a JSON object"),
        ));
    };
    let raw_id = obj.get("id");
    let id = raw_id.filter(|id| is_valid_id(id)).cloned();
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err((id, RpcError::invalid_request("\"jsonrpc\" must be \"2.0\"")));
    }
    match obj.get("method") {
        Some(Value::String(method)) => {
            let params = match obj.get("params") {
                None | Some(Value::Null) => Map::new(),
                Some(Value::Object(map)) => map.clone(),
                Some(_) => {
                    return Err((id, RpcError::invalid_params("\"params\" must be an object")));
                }
            };
            match raw_id {
                None => Ok(Message::Notification {
                    method: method.clone(),
                }),
                Some(raw) => match id {
                    Some(id) => Ok(Message::Request {
                        id,
                        method: method.clone(),
                        params,
                    }),
                    None => Err((
                        None,
                        RpcError::invalid_request(format!(
                            "Request id must be a string or an integer, got {raw}"
                        )),
                    )),
                },
            }
        }
        Some(_) => Err((id, RpcError::invalid_request("\"method\" must be a string"))),
        None if raw_id.is_some() && (obj.contains_key("result") || obj.contains_key("error")) => {
            Ok(Message::Response)
        }
        None => Err((id, RpcError::invalid_request("Missing \"method\""))),
    }
}

/// MCP request ids are strings or integers, never `null`.
fn is_valid_id(id: &Value) -> bool {
    match id {
        Value::String(_) => true,
        Value::Number(n) => n.is_i64() || n.is_u64(),
        _ => false,
    }
}

/// A result response.
pub(crate) fn success(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An error response. The id is omitted when it could not be determined.
pub(crate) fn failure(id: Option<&Value>, error: &RpcError) -> Value {
    let mut obj = Map::new();
    obj.insert("jsonrpc".into(), "2.0".into());
    if let Some(id) = id {
        obj.insert("id".into(), id.clone());
    }
    obj.insert("error".into(), error.to_value());
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_messages() {
        let req = json!({"jsonrpc":"2.0","id":1,"method":"ping"});
        assert!(matches!(classify(&req), Ok(Message::Request { .. })));
        let note = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(matches!(classify(&note), Ok(Message::Notification { .. })));
        let resp = json!({"jsonrpc":"2.0","id":"a","result":{}});
        assert_eq!(classify(&resp), Ok(Message::Response));
    }

    #[test]
    fn rejects_bad_messages() {
        let (_, e) = classify(&json!([1])).unwrap_err();
        assert_eq!(e.code, code::INVALID_REQUEST);
        let (id, e) = classify(&json!({"jsonrpc":"1.0","id":7,"method":"x"})).unwrap_err();
        assert_eq!((id, e.code), (Some(json!(7)), code::INVALID_REQUEST));
        let (_, e) = classify(&json!({"jsonrpc":"2.0","id":null,"method":"x"})).unwrap_err();
        assert_eq!(e.code, code::INVALID_REQUEST);
        let (_, e) =
            classify(&json!({"jsonrpc":"2.0","id":1,"method":"x","params":[1]})).unwrap_err();
        assert_eq!(e.code, code::INVALID_PARAMS);
    }

    #[test]
    fn failure_omits_unknown_id() {
        let v = failure(None, &RpcError::parse("bad"));
        assert!(v.get("id").is_none());
        assert_eq!(v["error"]["code"], json!(code::PARSE_ERROR));
    }
}
