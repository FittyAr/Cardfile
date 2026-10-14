//! User-editable provider configuration, persisted in the app settings.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::preset::{ApiKind, Preset, preset};

/// A configured provider. Serialized into the user's settings JSON.
///
/// A user-defined provider is a config whose `preset` is
/// [`crate::CUSTOM_PRESET_ID`]; it may use any [`ApiKind`].
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    /// Unique id (UUIDv7).
    pub id: String,
    /// Display name chosen by the user.
    pub name: String,
    /// Id of the preset this config was created from.
    pub preset: String,
    /// Wire format.
    pub kind: ApiKind,
    /// Base URL including the version segment, e.g. `https://api.openai.com/v1`.
    /// For Azure: the resource URL, e.g. `https://my-res.openai.azure.com`.
    pub base_url: String,
    /// Model id (Azure: deployment name).
    pub model: String,
    /// API key, if any. Never logged: `Debug` redacts it.
    pub api_key: Option<String>,
    /// Sampling temperature; provider default when `None`.
    pub temperature: Option<f32>,
    /// Output token limit; provider default when `None` (Anthropic needs one,
    /// so a default is applied there).
    pub max_tokens: Option<u32>,
    /// Extra HTTP headers sent with every request.
    pub extra_headers: Vec<(String, String)>,
    /// Azure `api-version`. `None` or `"v1"` uses Azure's `/openai/v1` route.
    pub api_version: Option<String>,
}

impl ProviderConfig {
    /// A new config filled from `preset`, with a fresh id and no key.
    pub fn from_preset(preset: &Preset) -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: preset.name.to_owned(),
            preset: preset.id.to_owned(),
            kind: preset.kind,
            base_url: preset.base_url.to_owned(),
            model: preset.default_model.to_owned(),
            api_key: None,
            temperature: None,
            max_tokens: None,
            extra_headers: Vec::new(),
            api_version: preset.default_api_version.map(str::to_owned),
        }
    }

    /// The built-in preset this config came from, if known.
    pub fn preset_info(&self) -> Option<&'static Preset> {
        preset(&self.preset)
    }

    /// The API key, if one is set and not blank.
    pub fn key(&self) -> Option<&str> {
        self.api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
    }

    /// Whether requests need an API key.
    pub fn requires_api_key(&self) -> bool {
        match self.preset_info() {
            // A known preset with its original kind keeps the preset's rule.
            Some(p) if p.id != crate::CUSTOM_PRESET_ID && p.kind == self.kind => p.requires_api_key,
            _ => self.kind.key_required_by_default(),
        }
    }

    /// Whether the provider runs on this machine or the local network.
    pub fn is_local(&self) -> bool {
        self.preset_info().is_some_and(|p| p.local) || is_local_url(&self.base_url)
    }

    /// Whether the config has everything needed to send a request.
    pub fn is_usable(&self) -> bool {
        let url = self.base_url.trim();
        (url.starts_with("http://") || url.starts_with("https://"))
            && !self.model.trim().is_empty()
            && (!self.requires_api_key() || self.key().is_some())
    }
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .extra_headers
            .iter()
            .map(|(k, _)| (k.as_str(), "<redacted>"))
            .collect();
        f.debug_struct("ProviderConfig")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("preset", &self.preset)
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("extra_headers", &headers)
            .field("api_version", &self.api_version)
            .finish()
    }
}

/// Host part of `url` (without scheme, credentials, port or path).
fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if let Some(v6) = authority.strip_prefix('[') {
        return v6.split(']').next().unwrap_or_default();
    }
    authority.split(':').next().unwrap_or_default()
}

fn is_local_url(url: &str) -> bool {
    let host = host_of(url.trim()).to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "host.docker.internal"
    {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CUSTOM_PRESET_ID, PRESETS};

    #[test]
    fn debug_redacts_secrets() {
        let mut cfg = ProviderConfig::from_preset(preset("openai").unwrap());
        cfg.api_key = Some("sk-very-secret".into());
        cfg.extra_headers = vec![("X-Token".into(), "hidden-value".into())];
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains("sk-very-secret"));
        assert!(!dbg.contains("hidden-value"));
        assert!(dbg.contains("<redacted>"));
        assert!(dbg.contains("X-Token"));
    }

    #[test]
    fn from_preset_and_usability() {
        for p in PRESETS {
            let cfg = ProviderConfig::from_preset(p);
            assert_eq!(cfg.preset, p.id);
            assert!(uuid::Uuid::parse_str(&cfg.id).is_ok());
            // Presets that need a key or a URL are not usable out of the box.
            let expect = !p.requires_api_key && !p.base_url.is_empty();
            assert_eq!(cfg.is_usable(), expect, "{}", p.id);
        }
        let mut cfg = ProviderConfig::from_preset(preset("anthropic").unwrap());
        cfg.api_key = Some("  ".into());
        assert!(!cfg.is_usable());
        cfg.api_key = Some("key".into());
        assert!(cfg.is_usable());
        cfg.model.clear();
        assert!(!cfg.is_usable());
    }

    #[test]
    fn custom_kinds_require_keys_where_needed() {
        let mut cfg = ProviderConfig::from_preset(preset(CUSTOM_PRESET_ID).unwrap());
        cfg.base_url = "http://localhost:8080/v1".into();
        cfg.model = "m".into();
        assert!(cfg.is_usable());
        cfg.kind = ApiKind::Anthropic;
        assert!(!cfg.is_usable());
        // A known preset switched to another kind follows the kind default.
        let mut ollama = ProviderConfig::from_preset(preset("ollama").unwrap());
        assert!(!ollama.requires_api_key());
        ollama.kind = ApiKind::Gemini;
        assert!(ollama.requires_api_key());
    }

    #[test]
    fn local_detection() {
        assert!(is_local_url("http://localhost:11434/v1"));
        assert!(is_local_url("http://127.0.0.1:1234"));
        assert!(is_local_url("http://[::1]:8080/v1"));
        assert!(is_local_url("http://192.168.1.20:8080"));
        assert!(is_local_url("http://user:pw@10.0.0.2/v1"));
        assert!(is_local_url("http://my-box.local:11434"));
        assert!(!is_local_url("https://api.openai.com/v1"));
        assert!(!is_local_url(""));
        let mut cfg = ProviderConfig::from_preset(preset(CUSTOM_PRESET_ID).unwrap());
        cfg.base_url = "http://localhost:5000/v1".into();
        assert!(cfg.is_local());
        assert!(ProviderConfig::from_preset(preset("ollama").unwrap()).is_local());
        assert!(!ProviderConfig::from_preset(preset("openai").unwrap()).is_local());
    }

    #[test]
    fn serde_round_trip_and_defaults() {
        let mut cfg = ProviderConfig::from_preset(preset("azure").unwrap());
        cfg.api_key = Some("k".into());
        cfg.extra_headers = vec![("a".into(), "b".into())];
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
        let partial: ProviderConfig =
            serde_json::from_str(r#"{"name":"x","kind":"gemini"}"#).unwrap();
        assert_eq!(partial.kind, ApiKind::Gemini);
        assert!(partial.extra_headers.is_empty());
    }
}
