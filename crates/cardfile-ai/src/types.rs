//! Provider-neutral chat types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Author of a [`Message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Extra system instructions (merged into the provider's system prompt).
    System,
    /// The human.
    User,
    /// The model.
    Assistant,
    /// The result of a tool call.
    Tool,
}

/// A tool the model may call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Function name (`[a-zA-Z0-9_-]`, at most 64 characters for most providers).
    pub name: String,
    /// What the tool does; the model reads this to decide when to call it.
    pub description: String,
    /// JSON Schema of the arguments; must be an `object` schema.
    pub parameters: Value,
}

impl ToolSpec {
    /// Create a tool specification.
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// A tool invocation requested by the model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Call id used to match the result; generated when the provider sends none.
    pub id: String,
    /// Name of the tool.
    pub name: String,
    /// Arguments, normally a JSON object. If the provider sent a string that is
    /// not valid JSON it is kept as a JSON string.
    pub arguments: Value,
    /// Opaque provider data that must be echoed back with the call (Gemini's
    /// `thoughtSignature`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// One entry of a conversation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Author.
    pub role: Role,
    /// Text content (may be empty for an assistant turn that only calls tools).
    pub content: String,
    /// Tool calls made by the assistant in this turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For [`Role::Tool`]: id of the call this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// For [`Role::Tool`]: name of the tool that produced the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// For [`Role::Tool`]: whether the tool failed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl Message {
    fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
            is_error: false,
        }
    }

    /// A system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// A user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// An assistant message without tool calls.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }

    /// The result of `call`.
    pub fn tool_result(call: &ToolCall, content: impl Into<String>, is_error: bool) -> Self {
        Self {
            tool_call_id: Some(call.id.clone()),
            tool_name: Some(call.name.clone()),
            is_error,
            ..Self::new(Role::Tool, content)
        }
    }
}

/// Why the model stopped generating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural end of the answer.
    Stop,
    /// The model wants tool results.
    ToolCalls,
    /// The output token limit was reached.
    Length,
    /// The provider filtered or refused the content.
    ContentFilter,
    /// Any other provider-specific reason.
    Other(String),
}

/// Token usage reported by the provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Prompt tokens.
    pub input_tokens: u64,
    /// Generated tokens.
    pub output_tokens: u64,
}

/// One model turn.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatResponse {
    /// The assistant message, including any tool calls.
    pub message: Message,
    /// Why generation stopped.
    pub finish: FinishReason,
    /// Token usage, when reported.
    pub usage: Option<Usage>,
}
