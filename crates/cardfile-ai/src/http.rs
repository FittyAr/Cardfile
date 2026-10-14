//! Blocking HTTP transport shared by all wire formats.

use std::time::Duration;

use serde_json::Value;

use crate::config::ProviderConfig;
use crate::error::AiError;
use crate::preset::ApiKind;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLOUD_TIMEOUT: Duration = Duration::from_secs(120);
/// Local models can take minutes to load and generate on modest hardware.
const LOCAL_TIMEOUT: Duration = Duration::from_secs(300);
const BODY_LIMIT: u64 = 32 * 1024 * 1024;
const USER_AGENT: &str = concat!("Cardfile/", env!("CARGO_PKG_VERSION"));

fn agent(cfg: &ProviderConfig) -> ureq::Agent {
    let total = if cfg.is_local() {
        LOCAL_TIMEOUT
    } else {
        CLOUD_TIMEOUT
    };
    ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(total))
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build()
        .into()
}

/// Authentication and protocol headers for `cfg`, followed by its extra headers.
pub(crate) fn headers(cfg: &ProviderConfig) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let key = cfg.key();
    match cfg.kind {
        ApiKind::OpenAiCompatible | ApiKind::OpenAiResponses => {
            if let Some(k) = key {
                out.push(("Authorization".to_owned(), format!("Bearer {k}")));
            }
            if cfg.preset == "openrouter" {
                out.push(("X-Title".to_owned(), "Cardfile".to_owned()));
                out.push((
                    "HTTP-Referer".to_owned(),
                    "https://github.com/FittyAr/Cardfile".to_owned(),
                ));
            }
        }
        ApiKind::Anthropic => {
            if let Some(k) = key {
                out.push(("x-api-key".to_owned(), k.to_owned()));
            }
            out.push(("anthropic-version".to_owned(), "2023-06-01".to_owned()));
        }
        ApiKind::Gemini => {
            if let Some(k) = key {
                out.push(("x-goog-api-key".to_owned(), k.to_owned()));
            }
        }
        ApiKind::AzureOpenAi => {
            if let Some(k) = key {
                out.push(("api-key".to_owned(), k.to_owned()));
            }
        }
    }
    out.extend(cfg.extra_headers.iter().cloned());
    out
}

/// Fail early when the config cannot work.
pub(crate) fn validate(cfg: &ProviderConfig) -> Result<(), AiError> {
    let url = cfg.base_url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AiError::Config(
            "the base URL must start with http:// or https://".into(),
        ));
    }
    if cfg.requires_api_key() && cfg.key().is_none() {
        return Err(AiError::MissingApiKey);
    }
    Ok(())
}

/// Base URL without trailing slashes.
pub(crate) fn base(cfg: &ProviderConfig) -> &str {
    cfg.base_url.trim().trim_end_matches('/')
}

/// Percent-encode one URL path segment.
pub(crate) fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Remove the API key from text that may echo it (error bodies, transport errors).
fn scrub(cfg: &ProviderConfig, text: &str) -> String {
    let mut out = text.to_owned();
    if let Some(k) = cfg.key().filter(|k| k.len() >= 4) {
        out = out.replace(k, "<redacted>");
    }
    for (_, v) in &cfg.extra_headers {
        if v.len() >= 4 {
            out = out.replace(v.as_str(), "<redacted>");
        }
    }
    out
}

fn map_transport(cfg: &ProviderConfig, e: ureq::Error) -> AiError {
    match e {
        ureq::Error::Timeout(_) => AiError::Timeout,
        ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => AiError::Timeout,
        other => AiError::Network(scrub(cfg, &other.to_string())),
    }
}

/// POST `body` as JSON (or GET when `body` is `None`) and parse a JSON reply.
pub(crate) fn send(
    cfg: &ProviderConfig,
    url: &str,
    body: Option<&Value>,
) -> Result<Value, AiError> {
    let agent = agent(cfg);
    let hdrs = headers(cfg);
    let result = match body {
        Some(body) => {
            let mut req = agent.post(url).header("Content-Type", "application/json");
            for (k, v) in &hdrs {
                req = req.header(k.as_str(), v.as_str());
            }
            let payload =
                serde_json::to_string(body).map_err(|e| AiError::Decode(e.to_string()))?;
            req.send(payload)
        }
        None => {
            let mut req = agent.get(url);
            for (k, v) in &hdrs {
                req = req.header(k.as_str(), v.as_str());
            }
            req.call()
        }
    };
    let resp = result.map_err(|e| map_transport(cfg, e))?;
    let status = resp.status().as_u16();
    let text = resp
        .into_body()
        .into_with_config()
        .limit(BODY_LIMIT)
        .read_to_string()
        .map_err(|e| map_transport(cfg, e))?;
    if !(200..300).contains(&status) {
        return Err(AiError::Http {
            status,
            message: scrub(cfg, &error_message(&text)),
        });
    }
    serde_json::from_str(&text).map_err(|e| AiError::Decode(format!("invalid JSON: {e}")))
}

/// Extract a human-readable message from a provider error body.
pub(crate) fn error_message(body: &str) -> String {
    const MAX: usize = 500;
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let msg = parsed.as_ref().and_then(|v| {
        // Gemini sometimes wraps the error in an array.
        let v = v.as_array().and_then(|a| a.first()).unwrap_or(v);
        let err = v.get("error");
        err.and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .or_else(|| err.and_then(Value::as_str))
            .or_else(|| v.get("message").and_then(Value::as_str))
            .or_else(|| v.get("detail").and_then(Value::as_str))
            .or_else(|| {
                v.get("detail")
                    .and_then(|d| d.get("message"))
                    .and_then(Value::as_str)
            })
            .map(str::to_owned)
    });
    let msg = msg.unwrap_or_else(|| body.trim().to_owned());
    if msg.chars().count() > MAX {
        let cut: String = msg.chars().take(MAX).collect();
        format!("{cut}…")
    } else if msg.is_empty() {
        "no details".to_owned()
    } else {
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::preset;

    #[test]
    fn parses_provider_errors() {
        assert_eq!(
            error_message(r#"{"error":{"message":"Incorrect API key","type":"invalid"}}"#),
            "Incorrect API key"
        );
        assert_eq!(
            error_message(
                r#"{"type":"error","error":{"type":"not_found_error","message":"model: x"}}"#
            ),
            "model: x"
        );
        assert_eq!(
            error_message(r#"[{"error":{"code":400,"message":"bad schema","status":"INVALID"}}]"#),
            "bad schema"
        );
        assert_eq!(
            error_message(r#"{"error":"model not found"}"#),
            "model not found"
        );
        assert_eq!(error_message(r#"{"detail":"nope"}"#), "nope");
        assert_eq!(error_message("Bad Gateway"), "Bad Gateway");
        assert_eq!(error_message(""), "no details");
    }

    #[test]
    fn scrub_removes_key() {
        let mut cfg = crate::ProviderConfig::from_preset(preset("openai").unwrap());
        cfg.api_key = Some("sk-abcdef".into());
        assert_eq!(scrub(&cfg, "bad key sk-abcdef!"), "bad key <redacted>!");
    }

    #[test]
    fn auth_headers_per_kind() {
        let mut cfg = crate::ProviderConfig::from_preset(preset("anthropic").unwrap());
        cfg.api_key = Some("k1".into());
        let h = headers(&cfg);
        assert!(h.contains(&("x-api-key".into(), "k1".into())));
        assert!(h.contains(&("anthropic-version".into(), "2023-06-01".into())));
        cfg.kind = ApiKind::Gemini;
        assert_eq!(headers(&cfg), vec![("x-goog-api-key".into(), "k1".into())]);
        cfg.kind = ApiKind::AzureOpenAi;
        assert_eq!(headers(&cfg), vec![("api-key".into(), "k1".into())]);
        cfg.kind = ApiKind::OpenAiCompatible;
        cfg.extra_headers = vec![("X-A".into(), "b".into())];
        assert_eq!(
            headers(&cfg),
            vec![
                ("Authorization".into(), "Bearer k1".into()),
                ("X-A".into(), "b".into())
            ]
        );
    }

    #[test]
    fn encodes_segments() {
        assert_eq!(encode_segment("gemini-3.8-flash"), "gemini-3.8-flash");
        assert_eq!(encode_segment("a b/c"), "a%20b%2Fc");
    }
}
