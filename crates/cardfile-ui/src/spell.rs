//! Spell checking with Hunspell dictionaries via `spellbook`.
//!
//! Dictionaries (`<lang>.aff` + `<lang>.dic`) are looked up in the user's data folder
//! (`dictionaries/`) and next to the executable (`dictionaries/`, shipped by the
//! installers). Words the user adds go to `dictionaries/personal.txt`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cardfile_core::UserSettings;
use cardfile_editor::SpellChecker;
use gpui_kit::{App, AppContext as _, Global, Task};
use parking_lot::{Mutex, RwLock};

pub struct Speller {
    dict: RwLock<spellbook::Dictionary>,
    personal: Mutex<HashSet<String>>,
    personal_path: PathBuf,
}

impl Speller {
    pub fn new(aff: &str, dic: &str, personal_path: PathBuf) -> anyhow::Result<Self> {
        let mut dict = spellbook::Dictionary::new(aff, dic).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut personal = HashSet::new();
        if let Ok(text) = std::fs::read_to_string(&personal_path) {
            for word in text.lines().map(str::trim).filter(|w| !w.is_empty()) {
                let _ = dict.add(word);
                personal.insert(word.to_owned());
            }
        }
        Ok(Self {
            dict: RwLock::new(dict),
            personal: Mutex::new(personal),
            personal_path,
        })
    }
}

impl SpellChecker for Speller {
    fn check(&self, word: &str) -> bool {
        self.personal.lock().contains(word) || self.dict.read().check(word)
    }

    fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.read().suggest(word, &mut out);
        out
    }

    fn add_word(&self, word: &str) {
        let word = word.trim();
        if word.is_empty() || !self.personal.lock().insert(word.to_owned()) {
            return;
        }
        let _ = self.dict.write().add(word);
        let mut words: Vec<String> = self.personal.lock().iter().cloned().collect();
        words.sort();
        if let Some(dir) = self.personal_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.personal_path, words.join("\n") + "\n") {
            tracing::warn!("personal dictionary: {e}");
        }
    }
}

/// Loaded dictionaries, keyed by language (`None` = not available).
#[derive(Default)]
struct Dictionaries(HashMap<String, Option<Arc<Speller>>>);

impl Global for Dictionaries {}

fn search_dirs(cx: &App) -> Vec<PathBuf> {
    let mut dirs = vec![crate::state::AppState::get(cx).paths.dictionaries()];
    if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_owned))
    {
        dirs.push(exe.join("dictionaries"));
        // Linux packages install shared data here.
        dirs.push(exe.join("../share/cardfile/dictionaries"));
    }
    dirs.push(PathBuf::from("/usr/share/hunspell"));
    dirs
}

/// Languages with an installed dictionary.
pub fn available_languages(cx: &App) -> Vec<String> {
    let mut langs: Vec<String> = search_dirs(cx)
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|it| it.flatten())
        .filter_map(|e| {
            let p = e.path();
            (p.extension()? == "dic")
                .then(|| p.file_stem()?.to_str().map(str::to_owned))
                .flatten()
        })
        .collect();
    langs.sort();
    langs.dedup();
    langs
}

/// The spell checker for the user's settings. The first use of a language reads and
/// parses its dictionary (several MB) on the background executor.
pub fn checker(settings: &UserSettings, cx: &mut App) -> Task<Option<Arc<dyn SpellChecker>>> {
    if !settings.spellcheck {
        return Task::ready(None);
    }
    let lang = settings.spellcheck_language.clone();
    if !cx.has_global::<Dictionaries>() {
        cx.set_global(Dictionaries::default());
    }
    if let Some(cached) = cx.global::<Dictionaries>().0.get(&lang) {
        return Task::ready(cached.clone().map(|s| s as Arc<dyn SpellChecker>));
    }
    let personal = crate::state::AppState::get(cx)
        .paths
        .dictionaries()
        .join("personal.txt");
    let dirs = search_dirs(cx);
    let load = {
        let lang = lang.clone();
        cx.background_spawn(async move { load(&lang, dirs, personal) })
    };
    cx.spawn(async move |cx| {
        let loaded = load.await;
        cx.update(|cx| {
            cx.global_mut::<Dictionaries>()
                .0
                .insert(lang, loaded.clone());
        });
        loaded.map(|s| s as Arc<dyn SpellChecker>)
    })
}

fn load(lang: &str, dirs: Vec<PathBuf>, personal: PathBuf) -> Option<Arc<Speller>> {
    let loaded = dirs.into_iter().find_map(|dir| {
        let aff = std::fs::read_to_string(dir.join(format!("{lang}.aff"))).ok()?;
        let dic = std::fs::read_to_string(dir.join(format!("{lang}.dic"))).ok()?;
        match Speller::new(&aff, &dic, personal.clone()) {
            Ok(s) => Some(Arc::new(s)),
            Err(e) => {
                tracing::warn!("dictionary {lang} in {}: {e:#}", dir.display());
                None
            }
        }
    });
    if loaded.is_none() {
        tracing::info!("no spelling dictionary for {lang}");
    }
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\n";
    const DIC: &str = "3\nhello\nworld\ncard\n";

    #[test]
    fn checks_suggests_and_learns() {
        let dir = tempfile::tempdir().unwrap();
        let personal = dir.path().join("personal.txt");
        let s = Speller::new(AFF, DIC, personal.clone()).unwrap();
        assert!(s.check("hello"));
        assert!(!s.check("helo"));
        assert!(s.suggest("helo").contains(&"hello".to_owned()));
        s.add_word("Cardfile");
        assert!(s.check("Cardfile"));
        let again = Speller::new(AFF, DIC, personal).unwrap();
        assert!(again.check("Cardfile"));
    }
}
