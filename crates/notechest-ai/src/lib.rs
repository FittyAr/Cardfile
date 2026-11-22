//! Blocking client for chatting with LLM providers, with tool calling.
//!
//! Every network function blocks; callers run them on a background thread.
//! One provider-neutral set of types ([`Message`], [`ToolSpec`], [`ToolCall`])
//! is translated to each wire format ([`ApiKind`]): OpenAI Chat Completions
//! (and the many compatible servers, cloud and local), the OpenAI Responses
//! API, Anthropic Messages, Google Gemini and Azure OpenAI.
//!
//! API keys are never logged and are scrubbed from error messages.

mod agent;
mod config;
mod error;
mod http;
mod preset;
mod types;
mod wire;

pub use agent::{AgentEvent, AgentOutcome, run_agent};
pub use config::ProviderConfig;
pub use error::AiError;
pub use preset::{ApiKind, CUSTOM_PRESET_ID, PRESETS, Preset, preset};
pub use types::{ChatResponse, FinishReason, Message, Role, ToolCall, ToolSpec, Usage};

use wire::WireRequest;

fn build_request(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> WireRequest {
    match cfg.kind {
        ApiKind::OpenAiCompatible | ApiKind::AzureOpenAi => {
            wire::openai::build(cfg, system, messages, tools, max_tokens)
        }
        ApiKind::OpenAiResponses => {
            wire::responses::build(cfg, system, messages, tools, max_tokens)
        }
        ApiKind::Anthropic => wire::anthropic::build(cfg, system, messages, tools, max_tokens),
        ApiKind::Gemini => wire::gemini::build(cfg, system, messages, tools, max_tokens),
    }
}

fn parse_response(kind: ApiKind, body: &serde_json::Value) -> Result<ChatResponse, AiError> {
    match kind {
        ApiKind::OpenAiCompatible | ApiKind::AzureOpenAi => wire::openai::parse(body),
        ApiKind::OpenAiResponses => wire::responses::parse(body),
        ApiKind::Anthropic => wire::anthropic::parse(body),
        ApiKind::Gemini => wire::gemini::parse(body),
    }
}

fn chat_with_limit(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    max_tokens: Option<u32>,
) -> Result<ChatResponse, AiError> {
    http::validate(cfg)?;
    if cfg.model.trim().is_empty() {
        return Err(AiError::Config("no model selected".into()));
    }
    let req = build_request(cfg, system, messages, tools, max_tokens);
    let body = http::send(cfg, &req.url, Some(&req.body))?;
    parse_response(cfg.kind, &body)
}

/// Send one chat turn and return the model's reply.
///
/// `system` is the system prompt (system-role messages in `messages` are
/// appended to it). `tools` may be empty. Blocking.
pub fn chat(
    cfg: &ProviderConfig,
    system: &str,
    messages: &[Message],
    tools: &[ToolSpec],
) -> Result<ChatResponse, AiError> {
    chat_with_limit(cfg, system, messages, tools, None)
}

/// List the model ids the provider offers, sorted and deduplicated.
///
/// Returns [`AiError::Unsupported`] for Azure OpenAI, whose deployments are
/// not listable with an inference key. Blocking.
pub fn list_models(cfg: &ProviderConfig) -> Result<Vec<String>, AiError> {
    http::validate(cfg)?;
    let mut models = match cfg.kind {
        ApiKind::OpenAiCompatible | ApiKind::OpenAiResponses | ApiKind::AzureOpenAi => {
            let url = wire::openai::models_url(cfg).ok_or_else(|| {
                AiError::Unsupported("Azure OpenAI deployments cannot be listed".into())
            })?;
            wire::openai::parse_models(&http::send(cfg, &url, None)?)?
        }
        // Same `{"data":[{"id":..}]}` shape as OpenAI.
        ApiKind::Anthropic => {
            wire::openai::parse_models(&http::send(cfg, &wire::anthropic::models_url(cfg), None)?)?
        }
        ApiKind::Gemini => {
            wire::gemini::parse_models(&http::send(cfg, &wire::gemini::models_url(cfg), None)?)?
        }
    };
    models.sort();
    models.dedup();
    Ok(models)
}

/// Check that the provider is reachable, the key is accepted and the model
/// answers, by sending a tiny prompt with a small output limit. Blocking.
pub fn test_connection(cfg: &ProviderConfig) -> Result<(), AiError> {
    chat_with_limit(cfg, "", &[Message::user("ping")], &[], Some(16)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_incomplete_configs_without_network() {
        let mut cfg = ProviderConfig::from_preset(preset("openai").unwrap_or(&PRESETS[0]));
        assert_eq!(chat(&cfg, "", &[], &[]), Err(AiError::MissingApiKey));
        cfg.api_key = Some("k".into());
        cfg.model.clear();
        assert!(matches!(chat(&cfg, "", &[], &[]), Err(AiError::Config(_))));
        cfg.base_url = "ftp://x".into();
        assert!(matches!(list_models(&cfg), Err(AiError::Config(_))));

        let mut azure = ProviderConfig::from_preset(preset("azure").unwrap_or(&PRESETS[0]));
        azure.base_url = "https://res.openai.azure.com".into();
        azure.api_key = Some("k".into());
        assert!(matches!(list_models(&azure), Err(AiError::Unsupported(_))));
    }
}
