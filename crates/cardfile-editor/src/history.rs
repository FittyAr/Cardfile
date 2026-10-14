//! Snapshot-based undo/redo. Cards are small, so storing whole documents is simpler
//! and more robust than inverting operations.

use std::time::{Duration, Instant};

use cardfile_doc::Document;

use crate::pos::Selection;

const MAX_ENTRIES: usize = 300;
/// Consecutive edits of the same kind within this window are undone together.
const COALESCE: Duration = Duration::from_millis(800);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Typing,
    Deleting,
    Other,
}

#[derive(Clone, Debug)]
struct Entry {
    doc: Document,
    sel: Selection,
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    last: Option<(EditKind, Instant)>,
}

impl History {
    /// Record the state *before* an edit.
    pub fn record(&mut self, doc: &Document, sel: Selection, kind: EditKind) {
        let now = Instant::now();
        let coalesce = matches!(self.last, Some((k, t)) if k == kind && kind != EditKind::Other && now - t < COALESCE);
        self.last = Some((kind, now));
        self.redo.clear();
        if coalesce {
            return;
        }
        self.undo.push(Entry {
            doc: doc.clone(),
            sel,
        });
        if self.undo.len() > MAX_ENTRIES {
            self.undo.remove(0);
        }
    }

    /// Break coalescing so the next edit starts a new undo step (e.g. after the caret moves).
    pub fn seal(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, doc: &Document, sel: Selection) -> Option<(Document, Selection)> {
        let entry = self.undo.pop()?;
        self.redo.push(Entry {
            doc: doc.clone(),
            sel,
        });
        self.last = None;
        Some((entry.doc, entry.sel))
    }

    pub fn redo(&mut self, doc: &Document, sel: Selection) -> Option<(Document, Selection)> {
        let entry = self.redo.pop()?;
        self.undo.push(Entry {
            doc: doc.clone(),
            sel,
        });
        self.last = None;
        Some((entry.doc, entry.sel))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_typing_and_redoes() {
        let mut h = History::default();
        let d0 = Document::from_plain_text("a");
        let d1 = Document::from_plain_text("ab");
        let d2 = Document::from_plain_text("abc");
        let sel = Selection::default();
        h.record(&d0, sel, EditKind::Typing);
        h.record(&d1, sel, EditKind::Typing);
        let (back, _) = h.undo(&d2, sel).unwrap();
        assert_eq!(back, d0);
        assert!(!h.can_undo());
        let (fwd, _) = h.redo(&back, sel).unwrap();
        assert_eq!(fwd, d2);
        h.seal();
        h.record(&d2, sel, EditKind::Typing);
        h.record(&d2, sel, EditKind::Other);
        assert_eq!(h.undo.len(), 3);
    }
}
