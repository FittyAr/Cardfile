//! Built-in provider presets and the wire formats they speak.
//!
//! Model lists are suggestions checked against the providers' docs in
//! October 2026; users can always type any model id.

use serde::{Deserialize, Serialize};

/// Wire format used to talk to a provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKind {
    /// OpenAI Chat Completions (`/chat/completions`, `tools` / `tool_calls`).
    /// Spoken by OpenAI and most other providers and local servers.
    #[default]
    OpenAiCompatible,
    /// OpenAI Responses API (`/responses`, `function_call` items).
    OpenAiResponses,
    /// Anthropic Messages API (`/messages`, `tool_use` / `tool_result` blocks).
    Anthropic,
    /// Google Gemini `generateContent` (`functionCall` / `functionResponse`).
    Gemini,
    /// Azure OpenAI: Chat Completions on a deployment URL with `api-key` auth.
    AzureOpenAi,
}

impl ApiKind {
    /// Every kind, in the order a settings UI should offer them.
    pub const ALL: [ApiKind; 5] = [
        ApiKind::OpenAiCompatible,
        ApiKind::OpenAiResponses,
        ApiKind::Anthropic,
        ApiKind::Gemini,
        ApiKind::AzureOpenAi,
    ];

    /// Stable identifier, identical to the serialized form.
    pub fn id(self) -> &'static str {
        match self {
            ApiKind::OpenAiCompatible => "open_ai_compatible",
            ApiKind::OpenAiResponses => "open_ai_responses",
            ApiKind::Anthropic => "anthropic",
            ApiKind::Gemini => "gemini",
            ApiKind::AzureOpenAi => "azure_open_ai",
        }
    }

    /// Human-readable name (a technical label, not translated).
    pub fn display_name(self) -> &'static str {
        match self {
            ApiKind::OpenAiCompatible => "OpenAI-compatible (Chat Completions)",
            ApiKind::OpenAiResponses => "OpenAI Responses",
            ApiKind::Anthropic => "Anthropic Messages",
            ApiKind::Gemini => "Google Gemini",
            ApiKind::AzureOpenAi => "Azure OpenAI",
        }
    }

    /// Whether a configuration of this kind without a known preset should be
    /// assumed to need an API key.
    pub fn key_required_by_default(self) -> bool {
        matches!(
            self,
            ApiKind::Anthropic | ApiKind::Gemini | ApiKind::AzureOpenAi
        )
    }
}

/// A built-in provider template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// Stable id stored in [`crate::ProviderConfig::preset`].
    pub id: &'static str,
    /// Display name (a brand name, not translated).
    pub name: &'static str,
    /// Wire format.
    pub kind: ApiKind,
    /// Default base URL; empty when the user must supply one.
    pub base_url: &'static str,
    /// Example URL shown as a placeholder when `base_url` is empty.
    pub url_placeholder: &'static str,
    /// Suggested model ids (or deployment names for Azure).
    pub models: &'static [&'static str],
    /// Model selected by default; empty when the user must choose.
    pub default_model: &'static str,
    /// Whether the provider rejects requests without an API key.
    pub requires_api_key: bool,
    /// Whether the provider runs on the user's machine.
    pub local: bool,
    /// Where users get an API key or read the setup docs.
    pub key_url: &'static str,
    /// Default `api-version` (Azure only).
    pub default_api_version: Option<&'static str>,
}

/// Id of the user-defined preset.
pub const CUSTOM_PRESET_ID: &str = "custom";

/// All built-in presets. The last one is the custom (user-defined) entry.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "openai",
        name: "OpenAI",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.openai.com/v1",
        url_placeholder: "",
        models: &[
            "gpt-6.1-sol",
            "gpt-6-astra",
            "gpt-6-luna",
            "gpt-5.6-sol",
            "gpt-5.6-luna",
            "gpt-5.5",
        ],
        default_model: "gpt-6.1-sol",
        requires_api_key: true,
        local: false,
        key_url: "https://platform.openai.com/api-keys",
        default_api_version: None,
    },
    Preset {
        id: "anthropic",
        name: "Anthropic (Claude)",
        kind: ApiKind::Anthropic,
        base_url: "https://api.anthropic.com/v1",
        url_placeholder: "",
        models: &[
            "claude-sonnet-5-5",
            "claude-opus-5-5",
            "claude-haiku-5-5",
            "claude-fable-5-1",
        ],
        default_model: "claude-sonnet-5-5",
        requires_api_key: true,
        local: false,
        key_url: "https://console.anthropic.com/settings/keys",
        default_api_version: None,
    },
    Preset {
        id: "gemini",
        name: "Google Gemini",
        kind: ApiKind::Gemini,
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        url_placeholder: "",
        models: &[
            "gemini-3.8-flash",
            "gemini-4-argon",
            "gemini-3.6-flash",
            "gemini-3.5-flash-lite",
            "gemini-3.1-pro-preview",
            "gemini-flash-latest",
        ],
        default_model: "gemini-3.8-flash",
        requires_api_key: true,
        local: false,
        key_url: "https://aistudio.google.com/app/apikey",
        default_api_version: None,
    },
    Preset {
        id: "mistral",
        name: "Mistral AI",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.mistral.ai/v1",
        url_placeholder: "",
        models: &[
            "mistral-large-latest",
            "mistral-medium-latest",
            "mistral-small-latest",
            "codestral-latest",
            "ministral-8b-latest",
        ],
        default_model: "mistral-large-latest",
        requires_api_key: true,
        local: false,
        key_url: "https://console.mistral.ai/api-keys",
        default_api_version: None,
    },
    Preset {
        id: "groq",
        name: "Groq",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.groq.com/openai/v1",
        url_placeholder: "",
        models: &[
            "openai/gpt-oss-120b",
            "openai/gpt-oss-20b",
            "qwen/qwen3-32b",
            "moonshotai/kimi-k2-instruct-0905",
        ],
        default_model: "openai/gpt-oss-120b",
        requires_api_key: true,
        local: false,
        key_url: "https://console.groq.com/keys",
        default_api_version: None,
    },
    Preset {
        id: "deepseek",
        name: "DeepSeek",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.deepseek.com/v1",
        url_placeholder: "",
        models: &["deepseek-v4-flash", "deepseek-v4-pro"],
        default_model: "deepseek-v4-flash",
        requires_api_key: true,
        local: false,
        key_url: "https://platform.deepseek.com/api_keys",
        default_api_version: None,
    },
    Preset {
        id: "xai",
        name: "xAI (Grok)",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.x.ai/v1",
        url_placeholder: "",
        models: &["grok-4.7", "grok-4.6", "grok-4.5", "grok-build-0.1"],
        default_model: "grok-4.7",
        requires_api_key: true,
        local: false,
        key_url: "https://console.x.ai",
        default_api_version: None,
    },
    Preset {
        id: "openrouter",
        name: "OpenRouter",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://openrouter.ai/api/v1",
        url_placeholder: "",
        models: &[
            "openrouter/auto",
            "anthropic/claude-sonnet-5.5",
            "openai/gpt-6.1-sol",
            "google/gemini-3.8-flash",
            "x-ai/grok-4.7",
            "deepseek/deepseek-v4-flash",
        ],
        default_model: "openrouter/auto",
        requires_api_key: true,
        local: false,
        key_url: "https://openrouter.ai/settings/keys",
        default_api_version: None,
    },
    Preset {
        id: "together",
        name: "Together AI",
        kind: ApiKind::OpenAiCompatible,
        base_url: "https://api.together.xyz/v1",
        url_placeholder: "",
        models: &[
            "moonshotai/Kimi-K2.5",
            "zai-org/GLM-5.1",
            "deepseek-ai/DeepSeek-V4-Pro",
            "MiniMaxAI/MiniMax-M2.7",
            "google/gemma-4-31B-it",
        ],
        default_model: "moonshotai/Kimi-K2.5",
        requires_api_key: true,
        local: false,
        key_url: "https://api.together.ai/settings/api-keys",
        default_api_version: None,
    },
    // Perplexity retired its Sonar Chat Completions API in September 2026; its
    // Agent API speaks the OpenAI Responses format.
    Preset {
        id: "perplexity",
        name: "Perplexity",
        kind: ApiKind::OpenAiResponses,
        base_url: "https://api.perplexity.ai/v1",
        url_placeholder: "",
        models: &[
            "perplexity/sonar",
            "openai/gpt-6.1-sol",
            "anthropic/claude-sonnet-5-5",
            "google/gemini-3.8-flash",
            "xai/grok-4.7",
        ],
        default_model: "perplexity/sonar",
        requires_api_key: true,
        local: false,
        key_url: "https://www.perplexity.ai/account/api/keys",
        default_api_version: None,
    },
    Preset {
        id: "azure",
        name: "Azure OpenAI",
        kind: ApiKind::AzureOpenAi,
        base_url: "",
        url_placeholder: "https://YOUR-RESOURCE.openai.azure.com",
        models: &["gpt-6.1-sol", "gpt-6-astra", "gpt-6-luna", "gpt-5.5"],
        default_model: "gpt-6.1-sol",
        requires_api_key: true,
        local: false,
        key_url: "https://portal.azure.com/#view/Microsoft_Azure_ProjectOxford/CognitiveServicesHub/~/OpenAI",
        default_api_version: Some("2024-10-21"),
    },
    Preset {
        id: "ollama",
        name: "Ollama",
        kind: ApiKind::OpenAiCompatible,
        base_url: "http://localhost:11434/v1",
        url_placeholder: "",
        models: &[
            "qwen3",
            "gpt-oss:20b",
            "llama3.3",
            "gemma3",
            "mistral-small3.2",
        ],
        default_model: "qwen3",
        requires_api_key: false,
        local: true,
        key_url: "https://ollama.com/download",
        default_api_version: None,
    },
    Preset {
        id: "lmstudio",
        name: "LM Studio",
        kind: ApiKind::OpenAiCompatible,
        base_url: "http://localhost:1234/v1",
        url_placeholder: "",
        models: &["openai/gpt-oss-20b", "qwen/qwen3-8b", "google/gemma-3-12b"],
        default_model: "openai/gpt-oss-20b",
        requires_api_key: false,
        local: true,
        key_url: "https://lmstudio.ai/docs/app/api",
        default_api_version: None,
    },
    Preset {
        id: CUSTOM_PRESET_ID,
        name: "Custom (OpenAI-compatible)",
        kind: ApiKind::OpenAiCompatible,
        base_url: "",
        url_placeholder: "http://localhost:8080/v1",
        models: &[],
        default_model: "",
        requires_api_key: false,
        local: false,
        key_url: "",
        default_api_version: None,
    },
];

/// Look up a built-in preset by id.
pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_fields_sane() {
        let mut ids = HashSet::new();
        for p in PRESETS {
            assert!(ids.insert(p.id), "duplicate preset id {}", p.id);
            assert!(!p.name.is_empty(), "{} has no name", p.id);
            if !p.default_model.is_empty() {
                assert!(
                    p.models.contains(&p.default_model),
                    "{}: default model not in suggestions",
                    p.id
                );
            }
            if p.id != CUSTOM_PRESET_ID {
                assert!(!p.default_model.is_empty(), "{} has no default model", p.id);
                assert!(!p.key_url.is_empty(), "{} has no key url", p.id);
            }
            if p.base_url.is_empty() {
                assert!(
                    !p.url_placeholder.is_empty(),
                    "{} needs a placeholder",
                    p.id
                );
            } else {
                assert!(!p.base_url.ends_with('/'), "{} base url ends with /", p.id);
            }
            assert_eq!(
                p.kind == ApiKind::AzureOpenAi,
                p.default_api_version.is_some()
            );
            if p.local {
                assert!(!p.requires_api_key);
            }
        }
        assert_eq!(PRESETS.last().map(|p| p.id), Some(CUSTOM_PRESET_ID));
    }

    #[test]
    fn required_presets_exist() {
        for id in [
            "openai",
            "anthropic",
            "gemini",
            "mistral",
            "groq",
            "deepseek",
            "xai",
            "openrouter",
            "together",
            "perplexity",
            "azure",
            "ollama",
            "lmstudio",
            "custom",
        ] {
            assert!(preset(id).is_some(), "missing preset {id}");
        }
        assert_eq!(
            preset("anthropic").map(|p| p.default_model),
            Some("claude-sonnet-5-5")
        );
    }

    #[test]
    fn kind_ids_match_serde() {
        for kind in ApiKind::ALL {
            let json = serde_json::to_string(&kind).unwrap_or_default();
            assert_eq!(json, format!("\"{}\"", kind.id()));
            assert!(!kind.display_name().is_empty());
        }
    }
}
