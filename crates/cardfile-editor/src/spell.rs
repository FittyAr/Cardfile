//! Spell-check results for the editor's slots.
//!
//! Results are kept per slot, for the slot's latest text only, so memory follows the
//! document's size. Word verdicts are cached too (bounded), so re-checking a block
//! after a keystroke only asks the checker about new words. Large blocks are
//! checked on the background executor; meanwhile their previous results are shown.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use cardfile_doc::BlockKind;
use gpui_kit::{AppContext as _, Context};

use crate::editor::{RichEditor, SpellChecker};
use crate::pos::Slot;

/// Blocks longer than this (bytes) are checked off the UI thread.
const BACKGROUND_THRESHOLD: usize = 4 * 1024;
/// Word verdicts kept before the cache starts over.
const MAX_WORDS: usize = 50_000;

/// One slot's results, for the text they were computed from.
struct Entry {
    text: String,
    ranges: Vec<Range<usize>>,
}

/// Verdicts of the spell checker by word, shared with background checks.
#[derive(Default)]
struct WordCache(HashMap<String, bool>);

#[derive(Default)]
pub(crate) struct SpellState {
    entries: HashMap<Slot, Entry>,
    words: Arc<Mutex<WordCache>>,
    /// Slots with a background check in flight.
    pending: HashSet<Slot>,
    /// Bumped when the dictionary or document changes, so results computed
    /// before are dropped.
    generation: u64,
}

impl SpellState {
    /// Forget everything (another checker, or a word was added to the dictionary).
    pub(crate) fn clear(&mut self) {
        self.clear_slots();
        lock(&self.words).0.clear();
    }

    /// Forget the per-slot results (another document is shown).
    pub(crate) fn clear_slots(&mut self) {
        self.entries.clear();
        self.pending.clear();
        self.generation += 1;
    }
}

fn lock(words: &Mutex<WordCache>) -> std::sync::MutexGuard<'_, WordCache> {
    // A panic while holding the lock leaves only a cache behind: keep using it.
    words
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Misspelled ranges of `text`, asking `spell` only about words not seen before.
fn check_text(text: &str, spell: &dyn SpellChecker, words: &Mutex<WordCache>) -> Vec<Range<usize>> {
    crate::util::misspelled_ranges(text, |w| {
        if let Some(ok) = lock(words).0.get(w) {
            return *ok;
        }
        // Not holding the lock while the checker runs keeps the UI thread free.
        let ok = spell.check(w);
        let mut cache = lock(words);
        if cache.0.len() >= MAX_WORDS {
            cache.0.clear();
        }
        cache.0.insert(w.to_owned(), ok);
        ok
    })
}

/// Whether `r` is a valid range in `text` (for results of an older text).
fn fits(text: &str, r: &Range<usize>) -> bool {
    r.end <= text.len() && text.is_char_boundary(r.start) && text.is_char_boundary(r.end)
}

impl RichEditor {
    /// Misspelled ranges to underline in `slot`, whose current text is `text`.
    pub(crate) fn misspelled_in(&self, slot: Slot, text: &str) -> Vec<Range<usize>> {
        if self.spell.is_none()
            || self
                .model
                .doc
                .blocks
                .get(slot.block)
                .is_some_and(|b| matches!(b.kind, BlockKind::Code { .. }))
        {
            return Vec::new();
        }
        let Some(entry) = self.spelling.entries.get(&slot) else {
            return Vec::new();
        };
        let mut ranges = if entry.text == text {
            entry.ranges.clone()
        } else {
            // A background check is catching up: keep the old marks that still fit.
            entry
                .ranges
                .iter()
                .filter(|r| fits(text, r))
                .cloned()
                .collect()
        };
        // Don't flag the word being typed.
        let caret = self.model.caret();
        if caret.slot == slot {
            ranges.retain(|r| !(r.start <= caret.offset && caret.offset <= r.end));
        }
        ranges
    }

    /// Spell-check the slots whose text changed (called from render).
    pub(crate) fn refresh_spelling(&mut self, cx: &mut Context<Self>) {
        let Some(spell) = self.spell.clone() else {
            return;
        };
        let mut seen: HashSet<Slot> = HashSet::new();
        let mut changed: Vec<(Slot, String)> = Vec::new();
        for (bi, block) in self.model.doc.blocks.iter().enumerate() {
            let mut visit = |slot: Slot, text: &str| {
                seen.insert(slot);
                let fresh = self
                    .spelling
                    .entries
                    .get(&slot)
                    .is_some_and(|e| e.text == text);
                if !fresh {
                    changed.push((slot, text.to_owned()));
                }
            };
            match &block.kind {
                BlockKind::Code { .. } => {}
                BlockKind::Table(t) => {
                    for (r, row) in t.rows.iter().enumerate() {
                        for (c, cell) in row.iter().enumerate() {
                            visit(Slot::cell(bi, r, c), &cell.text);
                        }
                    }
                }
                k if k.is_textual() => visit(Slot::block(bi), &block.text.text),
                _ => {}
            }
        }
        // Slots that no longer exist take their results with them.
        self.spelling.entries.retain(|s, _| seen.contains(s));
        self.spelling.pending.retain(|s| seen.contains(s));

        for (slot, text) in changed {
            if text.len() <= BACKGROUND_THRESHOLD {
                let ranges = check_text(&text, spell.as_ref(), &self.spelling.words);
                self.spelling.entries.insert(slot, Entry { text, ranges });
                continue;
            }
            if !self.spelling.pending.insert(slot) {
                // Already running; render checks again when it lands.
                continue;
            }
            let generation = self.spelling.generation;
            let words = self.spelling.words.clone();
            let spell = spell.clone();
            let task = cx.background_spawn(async move {
                let ranges = check_text(&text, spell.as_ref(), &words);
                (text, ranges)
            });
            cx.spawn(async move |this, cx| {
                let (text, ranges) = task.await;
                let _ = this.update(cx, |this, cx| {
                    if this.spelling.generation != generation
                        || !this.spelling.pending.remove(&slot)
                    {
                        return;
                    }
                    this.spelling.entries.insert(slot, Entry { text, ranges });
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counting(AtomicUsize);

    impl SpellChecker for Counting {
        fn check(&self, word: &str) -> bool {
            self.0.fetch_add(1, Ordering::Relaxed);
            word != "helo"
        }
        fn suggest(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
        fn add_word(&self, _: &str) {}
    }

    #[test]
    fn rechecks_only_new_words() {
        let spell = Counting(AtomicUsize::new(0));
        let words = Mutex::new(WordCache::default());
        let r = check_text("helo world helo", &spell, &words);
        assert_eq!(r, vec![0..4, 11..15]);
        assert_eq!(spell.0.load(Ordering::Relaxed), 2);
        let r = check_text("helo world helo again", &spell, &words);
        assert_eq!(r.len(), 2);
        assert_eq!(spell.0.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn stale_ranges_must_fit() {
        assert!(fits("héllo", &(0..1)));
        assert!(!fits("héllo", &(0..2)));
        assert!(!fits("ab", &(0..3)));
    }
}
