//! Bundled and user themes.

use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::{App, SharedString};

use crate::paths::DataPaths;

const BUNDLED: &str = include_str!("../../../assets/themes/cardfile.json");
pub const DEFAULT_THEME: &str = "Midnight";

/// Register bundled themes and any `*.json` theme files in the user's theme folder.
pub fn init(paths: &DataPaths, cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    if let Err(e) = registry.load_themes_from_str(BUNDLED) {
        tracing::error!("bundled themes: {e:#}");
    }
    let dir = paths.themes();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(text) = std::fs::read_to_string(&path)
                && let Err(e) = ThemeRegistry::global_mut(cx).load_themes_from_str(&text)
            {
                tracing::warn!("theme {}: {e:#}", path.display());
            }
        }
    }
}

/// Theme names, bundled first then user themes, alphabetically.
pub fn names(cx: &App) -> Vec<SharedString> {
    let mut names: Vec<SharedString> = ThemeRegistry::global(cx).themes().keys().cloned().collect();
    names.sort();
    names
}

/// Apply a theme by name, falling back to the default.
pub fn apply(name: &str, cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let config = registry
        .themes()
        .get(name)
        .or_else(|| registry.themes().get(DEFAULT_THEME))
        .cloned();
    if let Some(config) = config {
        let mode = config.mode;
        Theme::update(cx, |t| t.apply_config(&config));
        Theme::change(mode, None, cx);
    }
}
