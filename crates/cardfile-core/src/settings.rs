use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{LockMode, SearchScope, SortOrder};

/// How people sign in. Chosen in the first-run wizard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    /// Several accounts, each with username and password.
    MultiUser,
    /// One account protected by a password.
    SingleUser,
    /// One account, no password.
    #[default]
    NoPassword,
}

/// Installation-wide settings stored in the database (`app_settings`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// `false` until the first-run wizard completes.
    pub initialized: bool,
    pub auth_mode: AuthMode,
    /// Lock modes users may choose from. Unknown names (e.g. from a newer version)
    /// are skipped instead of discarding the whole list.
    #[serde(deserialize_with = "known_lock_modes")]
    pub enabled_lock_modes: Vec<LockMode>,
    /// Allow anyone at the login screen to create an account (multi-user only).
    pub allow_registration: bool,
    /// Run the local MCP server while the app is open, so AI clients can use Cardfile
    /// with an API key.
    pub mcp_enabled: bool,
    /// TCP port of the MCP server on 127.0.0.1.
    pub mcp_port: u16,
}

/// Default port of the local MCP server.
pub const DEFAULT_MCP_PORT: u16 = 7337;

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            initialized: false,
            auth_mode: AuthMode::default(),
            enabled_lock_modes: LockMode::ALL.to_vec(),
            allow_registration: false,
            mcp_enabled: false,
            mcp_port: DEFAULT_MCP_PORT,
        }
    }
}

/// Keep the lock modes this version knows.
fn known_lock_modes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<LockMode>, D::Error> {
    let raw = Vec::<serde_json::Value>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// Parse settings stored as a JSON object, field by field.
///
/// A field that does not parse (a value from a newer version, a `null` written for
/// a NaN float, …) falls back to its default instead of discarding every other
/// field: losing `initialized` would re-run the first-run wizard on an existing
/// database, and losing `ai_providers` would erase them on the next save. Only input
/// that is not a JSON object at all is an error.
fn parse_lenient<T>(json: &str) -> Result<T, serde_json::Error>
where
    T: Serialize + DeserializeOwned + Default,
{
    let serde_json::Value::Object(input) = serde_json::from_str::<serde_json::Value>(json)? else {
        return Err(serde::de::Error::custom("settings are not a JSON object"));
    };
    let serde_json::Value::Object(mut merged) = serde_json::to_value(T::default())? else {
        return Err(serde::de::Error::custom(
            "settings do not serialize to an object",
        ));
    };
    for (key, value) in input {
        let previous = merged.insert(key.clone(), value);
        if serde_json::from_value::<T>(serde_json::Value::Object(merged.clone())).is_err() {
            // Restore the default (or drop a field this version does not know).
            match previous {
                Some(p) => merged.insert(key, p),
                None => merged.remove(&key),
            };
        }
    }
    serde_json::from_value(serde_json::Value::Object(merged))
}

/// `value` when it is a finite number, `default` otherwise.
fn finite(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

impl AppSettings {
    /// Parse stored settings, tolerating unknown or invalid fields (see
    /// `parse_lenient`). Fails only when `json` is not a JSON object.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        parse_lenient(json)
    }

    pub fn lock_mode_enabled(&self, mode: LockMode) -> bool {
        mode == LockMode::None || self.enabled_lock_modes.contains(&mode)
    }
}

/// Per-user preferences stored as JSON on the user row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub theme: String,
    pub ui_font_size: f32,
    pub editor_font_size: f32,
    pub search_scope: SearchScope,
    pub sort: SortOrder,
    pub default_lock_mode: LockMode,
    /// Lock the session after this many idle seconds (0 = never).
    pub auto_lock_secs: u32,
    /// Re-lock an unlocked private card after this many seconds (0 = when deselected).
    pub relock_card_secs: u32,
    /// Visible leading characters of a masked private title.
    pub mask_visible_chars: usize,
    pub spellcheck: bool,
    pub spellcheck_language: String,
    pub markdown_shortcuts: bool,
    /// Snapshots kept per card (0 disables history).
    pub max_revisions: u32,
    pub check_for_updates: bool,
    pub confirm_delete: bool,
    pub language: String,
    /// Width of the card list column in logical pixels.
    pub list_width: f32,
    /// AI services the user configured (serialized `cardfile_ai::ProviderConfig`s).
    /// The assistant UI only appears when at least one is usable.
    pub ai_providers: Vec<serde_json::Value>,
    /// Id of the provider the chat uses.
    pub ai_active_provider: Option<String>,
    /// Let the in-app assistant read and edit private cards (never encrypted ones).
    pub ai_chat_allow_private: bool,
    /// Show the assistant panel.
    pub ai_panel_open: bool,
    /// Width of the assistant panel in logical pixels.
    pub ai_panel_width: f32,
}

impl UserSettings {
    /// Parse stored settings, tolerating unknown or invalid fields (see
    /// `parse_lenient`). Fails only when `json` is not a JSON object.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        parse_lenient(json)
    }

    /// A copy with non-finite sizes replaced by their defaults. JSON has no NaN or
    /// infinity (serde_json writes `null`), so they must not reach storage.
    pub fn sanitized(&self) -> Self {
        let d = Self::default();
        Self {
            ui_font_size: finite(self.ui_font_size, d.ui_font_size),
            editor_font_size: finite(self.editor_font_size, d.editor_font_size),
            list_width: finite(self.list_width, d.list_width),
            ai_panel_width: finite(self.ai_panel_width, d.ai_panel_width),
            ..self.clone()
        }
    }
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: "Midnight".into(),
            ui_font_size: 14.0,
            editor_font_size: 15.0,
            search_scope: SearchScope::default(),
            sort: SortOrder::default(),
            default_lock_mode: LockMode::None,
            auto_lock_secs: 0,
            relock_card_secs: 60,
            mask_visible_chars: 3,
            spellcheck: true,
            spellcheck_language: "en_US".into(),
            markdown_shortcuts: true,
            max_revisions: 50,
            check_for_updates: true,
            confirm_delete: true,
            language: "en".into(),
            list_width: 300.0,
            ai_providers: Vec::new(),
            ai_active_provider: None,
            ai_chat_allow_private: false,
            ai_panel_open: true,
            ai_panel_width: 360.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tolerate_missing_fields() {
        let s: UserSettings = serde_json::from_str(r#"{"theme":"Forest"}"#).unwrap();
        assert_eq!(s.theme, "Forest");
        assert_eq!(s.max_revisions, 50);
    }

    #[test]
    fn one_bad_field_keeps_the_others() {
        let json = r#"{"initialized":true,"auth_mode":"from_the_future","mcp_port":"x",
            "enabled_lock_modes":["private","quantum"],"unknown":1}"#;
        let s = AppSettings::from_json(json).unwrap();
        assert!(s.initialized);
        assert_eq!(s.auth_mode, AuthMode::default());
        assert_eq!(s.mcp_port, DEFAULT_MCP_PORT);
        assert_eq!(s.enabled_lock_modes, [LockMode::Private]);

        let json = r#"{"ui_font_size":null,"ai_providers":[{"id":"p"}],"theme":"Forest"}"#;
        let u = UserSettings::from_json(json).unwrap();
        assert_eq!(u.ui_font_size, 14.0);
        assert_eq!(u.ai_providers.len(), 1);
        assert_eq!(u.theme, "Forest");

        assert!(AppSettings::from_json("not json").is_err());
        assert!(UserSettings::from_json("[1]").is_err());
    }

    #[test]
    fn nan_sizes_are_sanitized() {
        let s = UserSettings {
            ui_font_size: f32::NAN,
            list_width: f32::INFINITY,
            ..Default::default()
        }
        .sanitized();
        assert_eq!(s.ui_font_size, 14.0);
        assert_eq!(s.list_width, 300.0);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(UserSettings::from_json(&json).unwrap(), s);
    }

    #[test]
    fn none_lock_mode_is_always_enabled() {
        let s = AppSettings {
            enabled_lock_modes: vec![],
            ..Default::default()
        };
        assert!(s.lock_mode_enabled(LockMode::None));
        assert!(!s.lock_mode_enabled(LockMode::Private));
    }
}
