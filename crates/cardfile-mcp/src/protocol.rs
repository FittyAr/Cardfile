//! MCP method dispatch, independent of the HTTP transport.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use serde_json::{Map, Value, json};

use crate::jsonrpc::RpcError;
use crate::types::{Handler, ServerInfo, Tool, ToolOutput};
use crate::version::ProtocolVersion;

const SERVER_INFO_META: &str = "io.modelcontextprotocol/serverInfo";

/// Serves MCP methods for one handler.
pub(crate) struct Protocol<H: Handler> {
    handler: Arc<H>,
    info: ServerInfo,
    tools_ttl_ms: u64,
}

impl<H: Handler> Protocol<H> {
    pub fn new(handler: Arc<H>, info: ServerInfo, tools_ttl_ms: u64) -> Self {
        Self {
            handler,
            info,
            tools_ttl_ms,
        }
    }

    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Picks the legacy revision for an `initialize` request: the client's if we implement it,
    /// otherwise our newest legacy one.
    pub fn negotiate(requested: &str) -> ProtocolVersion {
        match ProtocolVersion::parse(requested) {
            Some(v) if !v.is_modern() => v,
            _ => ProtocolVersion::LATEST_LEGACY,
        }
    }

    /// The `InitializeResult` for a legacy session.
    pub fn initialize_result(&self, version: ProtocolVersion) -> Value {
        let mut result = Map::new();
        result.insert("protocolVersion".into(), version.as_str().into());
        result.insert("capabilities".into(), capabilities());
        result.insert("serverInfo".into(), self.implementation(version));
        if let Some(instructions) = &self.info.instructions {
            result.insert("instructions".into(), instructions.clone().into());
        }
        Value::Object(result)
    }

    /// Runs one request (anything except `initialize`).
    pub fn call(
        &self,
        who: &H::Principal,
        version: ProtocolVersion,
        method: &str,
        params: &Map<String, Value>,
    ) -> Result<Value, RpcError> {
        let modern = version.is_modern();
        let mut result = match method {
            "server/discover" if modern => self.discover(),
            "ping" if !modern => Map::new(),
            "tools/list" => self.list_tools(who, version, params)?,
            "tools/call" => self.call_tool(who, version, params)?,
            _ => return Err(RpcError::method_not_found(method)),
        };
        if modern {
            result.insert("resultType".into(), "complete".into());
            let meta = result
                .entry("_meta")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(meta) = meta {
                meta.insert(SERVER_INFO_META.into(), self.implementation(version));
            }
        }
        Ok(Value::Object(result))
    }

    fn discover(&self) -> Map<String, Value> {
        let mut result = Map::new();
        result.insert(
            "supportedVersions".into(),
            json!(crate::version::supported_versions()),
        );
        result.insert("capabilities".into(), capabilities());
        if let Some(instructions) = &self.info.instructions {
            result.insert("instructions".into(), instructions.clone().into());
        }
        // Identical for every caller.
        result.insert("ttlMs".into(), 0.into());
        result.insert("cacheScope".into(), "public".into());
        result
    }

    fn list_tools(
        &self,
        who: &H::Principal,
        version: ProtocolVersion,
        params: &Map<String, Value>,
    ) -> Result<Map<String, Value>, RpcError> {
        // Everything fits in one page, so no cursor is ever issued.
        if params.get("cursor").is_some_and(|c| !c.is_null()) {
            return Err(RpcError::invalid_params("Invalid cursor"));
        }
        let tools: Vec<Value> = self
            .handler
            .tools(who)
            .iter()
            .map(|t| tool_json(t, version))
            .collect();
        let mut result = Map::new();
        result.insert("tools".into(), Value::Array(tools));
        if version.is_modern() {
            result.insert("ttlMs".into(), self.tools_ttl_ms.into());
            // The list depends on the caller's permissions.
            result.insert("cacheScope".into(), "private".into());
        }
        Ok(result)
    }

    fn call_tool(
        &self,
        who: &H::Principal,
        version: ProtocolVersion,
        params: &Map<String, Value>,
    ) -> Result<Map<String, Value>, RpcError> {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Err(RpcError::invalid_params("Missing tool \"name\""));
        };
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => Value::Object(Map::new()),
            Some(v @ Value::Object(_)) => v.clone(),
            Some(_) => return Err(RpcError::invalid_params("\"arguments\" must be an object")),
        };
        if !self.handler.tools(who).iter().any(|t| t.name == name) {
            return Err(RpcError::invalid_params(format!("Unknown tool: {name}")));
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.handler.call_tool(who, name, arguments)
        }));
        let mut result = Map::new();
        let (text, structured, is_error) = match outcome {
            Err(_) => {
                tracing::error!(tool = name, "MCP tool panicked");
                return Err(RpcError::internal(format!(
                    "Tool {name} failed unexpectedly"
                )));
            }
            Ok(Err(message)) => (message, None, true),
            Ok(Ok(ToolOutput::Text(text))) => (text, None, false),
            Ok(Ok(ToolOutput::Json(value))) => {
                let accepts = version.is_modern() || (version.has_titles() && value.is_object());
                (value.to_string(), accepts.then_some(value), false)
            }
        };
        result.insert("content".into(), json!([{ "type": "text", "text": text }]));
        if let Some(structured) = structured {
            result.insert("structuredContent".into(), structured);
        }
        result.insert("isError".into(), is_error.into());
        Ok(result)
    }

    /// The `Implementation` object describing this server.
    fn implementation(&self, version: ProtocolVersion) -> Value {
        let mut obj = Map::new();
        obj.insert("name".into(), self.info.name.clone().into());
        if version.has_titles()
            && let Some(title) = &self.info.title
        {
            obj.insert("title".into(), title.clone().into());
        }
        obj.insert("version".into(), self.info.version.clone().into());
        Value::Object(obj)
    }
}

/// Only tools are offered; resources and prompts are not advertised, so their methods are
/// "method not found".
fn capabilities() -> Value {
    json!({ "tools": { "listChanged": false } })
}

/// Serializes a tool for the given revision.
fn tool_json(tool: &Tool, version: ProtocolVersion) -> Value {
    let mut annotations = Map::new();
    if !version.has_titles()
        && let Some(title) = &tool.title
    {
        // 2025-03-26 only knows the title as an annotation.
        annotations.insert("title".into(), title.clone().into());
    }
    annotations.insert("readOnlyHint".into(), tool.read_only.into());
    annotations.insert(
        "destructiveHint".into(),
        (!tool.read_only && tool.destructive).into(),
    );
    annotations.insert("idempotentHint".into(), tool.idempotent.into());
    annotations.insert("openWorldHint".into(), tool.open_world.into());

    let mut obj = Map::new();
    obj.insert("name".into(), tool.name.clone().into());
    if version.has_titles()
        && let Some(title) = &tool.title
    {
        obj.insert("title".into(), title.clone().into());
    }
    obj.insert("description".into(), tool.description.clone().into());
    obj.insert("inputSchema".into(), tool.input_schema.clone());
    if version.has_titles()
        && let Some(schema) = &tool.output_schema
    {
        obj.insert("outputSchema".into(), schema.clone());
    }
    obj.insert("annotations".into(), Value::Object(annotations));
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl Handler for Echo {
        type Principal = ();
        fn authenticate(&self, _: &str) -> Option<()> {
            Some(())
        }
        fn tools(&self, _: &()) -> Vec<Tool> {
            vec![
                Tool::new("echo", "Echo", json!({"type":"object"})).with_title("Echo"),
                Tool::new("data", "Data", json!({"type":"object"})),
                Tool::new("boom", "Panics", json!({"type":"object"})).writes(true),
            ]
        }
        fn call_tool(&self, _: &(), name: &str, args: Value) -> Result<ToolOutput, String> {
            match name {
                "echo" => Ok(ToolOutput::Text(args.to_string())),
                "data" => Ok(ToolOutput::Json(json!([1, 2]))),
                _ => panic!("boom"),
            }
        }
    }

    fn proto() -> Protocol<Echo> {
        Protocol::new(Arc::new(Echo), ServerInfo::new("t", "1").with_title("T"), 0)
    }

    fn params(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn negotiates_versions() {
        type P = Protocol<Echo>;
        assert_eq!(P::negotiate("2025-06-18"), ProtocolVersion::V2025_06_18);
        assert_eq!(P::negotiate("2024-11-05"), ProtocolVersion::LATEST_LEGACY);
        assert_eq!(P::negotiate("2026-07-28"), ProtocolVersion::LATEST_LEGACY);
    }

    #[test]
    fn tool_json_follows_revision() {
        let p = proto();
        let old = p
            .call(&(), ProtocolVersion::V2025_03_26, "tools/list", &Map::new())
            .unwrap();
        assert!(old["tools"][0].get("title").is_none());
        assert_eq!(old["tools"][0]["annotations"]["title"], "Echo");
        assert_eq!(old["tools"][2]["annotations"]["destructiveHint"], true);
        let new = p
            .call(&(), ProtocolVersion::V2026_07_28, "tools/list", &Map::new())
            .unwrap();
        assert_eq!(new["tools"][0]["title"], "Echo");
        assert_eq!(new["resultType"], "complete");
        assert_eq!(new["cacheScope"], "private");
        assert_eq!(new["_meta"][SERVER_INFO_META]["title"], "T");
    }

    #[test]
    fn structured_content_only_where_allowed() {
        let p = proto();
        let args = params(json!({"name":"data"}));
        let legacy = p
            .call(&(), ProtocolVersion::V2025_11_25, "tools/call", &args)
            .unwrap();
        assert!(legacy.get("structuredContent").is_none());
        assert_eq!(legacy["content"][0]["text"], "[1,2]");
        let modern = p
            .call(&(), ProtocolVersion::V2026_07_28, "tools/call", &args)
            .unwrap();
        assert_eq!(modern["structuredContent"], json!([1, 2]));
    }

    #[test]
    fn protocol_errors() {
        let p = proto();
        let v = ProtocolVersion::V2025_11_25;
        let e = p
            .call(&(), v, "tools/call", &params(json!({"name":"nope"})))
            .unwrap_err();
        assert_eq!(e.code, crate::jsonrpc::code::INVALID_PARAMS);
        let e = p
            .call(&(), v, "tools/call", &params(json!({"name":"boom"})))
            .unwrap_err();
        assert_eq!(e.code, crate::jsonrpc::code::INTERNAL_ERROR);
        let e = p.call(&(), v, "server/discover", &Map::new()).unwrap_err();
        assert_eq!(e.code, crate::jsonrpc::code::METHOD_NOT_FOUND);
        let e = p
            .call(&(), ProtocolVersion::LATEST, "ping", &Map::new())
            .unwrap_err();
        assert_eq!(e.code, crate::jsonrpc::code::METHOD_NOT_FOUND);
        let e = p
            .call(&(), v, "tools/list", &params(json!({"cursor":"x"})))
            .unwrap_err();
        assert_eq!(e.code, crate::jsonrpc::code::INVALID_PARAMS);
    }
}
