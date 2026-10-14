//! Application actions and key bindings, with user overrides from `keymap.json`.
//!
//! `keymap.json` is a list of bindings; later entries win:
//!
//! ```json
//! [
//!   { "keys": "ctrl-shift-n", "action": "cardfile::NewCard" },
//!   { "keys": "ctrl-b", "action": "rich_editor::Bold", "context": "RichEditor" }
//! ]
//! ```

use std::rc::Rc;

use gpui_kit::{App, KeyBinding, KeyBindingContextPredicate, actions};
use serde::Deserialize;

use crate::paths::DataPaths;

actions!(
    cardfile,
    [
        NewCard,
        DeleteCard,
        FocusSearch,
        OpenSettings,
        LockApp,
        SignOut,
        Quit,
        TogglePin,
        ToggleFavorite,
        ExportCard,
        ExportAll,
        ShowAllCards,
        ShowFavorites,
        ShowRecycleBin,
        ShowHistory,
        CommandPalette,
        OpenKeymap,
        OpenDataFolder,
        BackupNow,
        CheckForUpdates,
        About,
        EmptyRecycleBin,
        ToggleAssistant,
        ShareCard,
        ShowShared,
    ]
);

/// Key context of the main workspace.
pub const WORKSPACE: &str = "Workspace";

pub fn default_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-n", NewCard, None),
        KeyBinding::new("ctrl-f", FocusSearch, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("ctrl-shift-l", LockApp, None),
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("ctrl-shift-p", CommandPalette, None),
        KeyBinding::new("ctrl-shift-backspace", DeleteCard, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-e", ExportCard, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-h", ShowHistory, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-s", ToggleFavorite, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-t", TogglePin, Some(WORKSPACE)),
        KeyBinding::new("ctrl-shift-a", ToggleAssistant, Some(WORKSPACE)),
    ]
}

#[derive(Debug, Deserialize)]
struct UserBinding {
    keys: String,
    action: String,
    #[serde(default)]
    context: Option<String>,
}

/// Install default bindings, then the user's `keymap.json` on top.
pub fn init(paths: &DataPaths, cx: &mut App) {
    cx.bind_keys(default_bindings());
    cardfile_editor::init(cx);
    match load_user_bindings(&paths.keymap(), cx) {
        Ok(bindings) if !bindings.is_empty() => cx.bind_keys(bindings),
        Ok(_) => {}
        Err(e) => tracing::warn!("keymap.json: {e:#}"),
    }
}

fn load_user_bindings(path: &std::path::Path, cx: &App) -> anyhow::Result<Vec<KeyBinding>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(path)?;
    // Allow `//` comment lines.
    let text: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    let entries: Vec<UserBinding> = serde_json::from_str(&text)?;
    let mut out = Vec::new();
    for e in entries {
        let action = match cx.build_action(&e.action, None) {
            Ok(a) => a,
            Err(err) => {
                tracing::warn!("keymap: unknown action {}: {err}", e.action);
                continue;
            }
        };
        let predicate = match e
            .context
            .as_deref()
            .map(KeyBindingContextPredicate::parse)
            .transpose()
        {
            Ok(p) => p.map(Rc::new),
            Err(err) => {
                tracing::warn!("keymap: bad context {:?}: {err}", e.context);
                continue;
            }
        };
        match KeyBinding::load(
            &e.keys,
            action,
            predicate,
            false,
            None,
            cx.keyboard_mapper().as_ref(),
        ) {
            Ok(b) => out.push(b),
            Err(err) => tracing::warn!("keymap: bad keys {}: {err}", e.keys),
        }
    }
    Ok(out)
}

/// Write a starter `keymap.json` listing every action, if none exists.
pub fn ensure_keymap_file(paths: &DataPaths, cx: &App) -> std::io::Result<std::path::PathBuf> {
    let path = paths.keymap();
    if !path.exists() {
        std::fs::create_dir_all(&paths.root)?;
        let mut names: Vec<&str> = cx
            .all_action_names()
            .iter()
            .copied()
            .filter(|n| n.starts_with("cardfile::") || n.starts_with("rich_editor::"))
            .collect();
        names.sort_unstable();
        let mut text = String::from(
            "// Cardfile key bindings. Uncomment and edit entries; restart Cardfile to apply.\n\
             // Available actions:\n",
        );
        for n in names {
            text.push_str("//   ");
            text.push_str(n);
            text.push('\n');
        }
        text.push_str(
            "[\n  // { \"keys\": \"ctrl-shift-n\", \"action\": \"cardfile::NewCard\" }\n]\n",
        );
        std::fs::write(&path, text)?;
    }
    Ok(path)
}
