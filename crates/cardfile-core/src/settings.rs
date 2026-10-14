use serde::{Deserialize, Serialize};

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
    /// Lock modes users may choose from.
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

impl AppSettings {
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
    fn none_lock_mode_is_always_enabled() {
        let s = AppSettings {
            enabled_lock_modes: vec![],
            ..Default::default()
        };
        assert!(s.lock_mode_enabled(LockMode::None));
        assert!(!s.lock_mode_enabled(LockMode::Private));
    }
}
