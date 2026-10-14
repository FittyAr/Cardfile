//! Caret positions and selections inside a [`Document`].
//!
//! Every place that holds editable text is a [`Slot`]: one per non-table block and one
//! per table cell. Slots are totally ordered (block, row, col), so a selection is simply
//! a pair of positions. Atom blocks (divider, image, attachment) are slots of length 0.

use cardfile_doc::{BlockKind, Document, RichText};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slot {
    pub block: usize,
    pub row: usize,
    pub col: usize,
}

impl Slot {
    pub const fn block(block: usize) -> Self {
        Self {
            block,
            row: 0,
            col: 0,
        }
    }

    pub const fn cell(block: usize, row: usize, col: usize) -> Self {
        Self { block, row, col }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub slot: Slot,
    /// Byte offset in the slot's text.
    pub offset: usize,
}

impl Pos {
    pub const fn new(slot: Slot, offset: usize) -> Self {
        Self { slot, offset }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Pos,
    pub head: Pos,
}

impl Selection {
    pub const fn caret(pos: Pos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn start(&self) -> Pos {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> Pos {
        self.anchor.max(self.head)
    }

    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }

    /// Whether the selection lies inside one slot.
    pub fn is_single_slot(&self) -> bool {
        self.anchor.slot == self.head.slot
    }
}

/// Read-only helpers over a document's slots.
pub trait Slots {
    fn slot_text(&self, slot: Slot) -> Option<&RichText>;
    fn slot_text_mut(&mut self, slot: Slot) -> Option<&mut RichText>;
    fn slot_len(&self, slot: Slot) -> usize;
    fn is_atom(&self, slot: Slot) -> bool;
    fn is_cell(&self, slot: Slot) -> bool;
    fn first_slot(&self, block: usize) -> Slot;
    fn last_slot(&self, block: usize) -> Slot;
    fn next_slot(&self, slot: Slot) -> Option<Slot>;
    fn prev_slot(&self, slot: Slot) -> Option<Slot>;
    fn slots_between(&self, start: Slot, end: Slot) -> Vec<Slot>;
    fn clamp_pos(&self, pos: Pos) -> Pos;
    fn doc_start(&self) -> Pos;
    fn doc_end(&self) -> Pos;
}

impl Slots for Document {
    fn slot_text(&self, slot: Slot) -> Option<&RichText> {
        let block = self.blocks.get(slot.block)?;
        match &block.kind {
            BlockKind::Table(t) => t.rows.get(slot.row)?.get(slot.col),
            k if k.is_textual() => Some(&block.text),
            _ => None,
        }
    }

    fn slot_text_mut(&mut self, slot: Slot) -> Option<&mut RichText> {
        let block = self.blocks.get_mut(slot.block)?;
        match &mut block.kind {
            BlockKind::Table(t) => t.rows.get_mut(slot.row)?.get_mut(slot.col),
            k if k.is_textual() => Some(&mut block.text),
            _ => None,
        }
    }

    fn slot_len(&self, slot: Slot) -> usize {
        self.slot_text(slot).map_or(0, RichText::len)
    }

    fn is_atom(&self, slot: Slot) -> bool {
        self.blocks.get(slot.block).is_some_and(|b| {
            matches!(
                b.kind,
                BlockKind::Divider | BlockKind::Image(_) | BlockKind::Attachment(_)
            )
        })
    }

    fn is_cell(&self, slot: Slot) -> bool {
        self.blocks
            .get(slot.block)
            .is_some_and(|b| matches!(b.kind, BlockKind::Table(_)))
    }

    fn first_slot(&self, block: usize) -> Slot {
        Slot::block(block)
    }

    fn last_slot(&self, block: usize) -> Slot {
        match self.blocks.get(block).map(|b| &b.kind) {
            Some(BlockKind::Table(t)) => Slot::cell(
                block,
                t.rows.len().saturating_sub(1),
                t.cols().saturating_sub(1),
            ),
            _ => Slot::block(block),
        }
    }

    fn next_slot(&self, slot: Slot) -> Option<Slot> {
        if let Some(BlockKind::Table(t)) = self.blocks.get(slot.block).map(|b| &b.kind) {
            if slot.col + 1 < t.cols() {
                return Some(Slot::cell(slot.block, slot.row, slot.col + 1));
            }
            if slot.row + 1 < t.rows.len() {
                return Some(Slot::cell(slot.block, slot.row + 1, 0));
            }
        }
        (slot.block + 1 < self.blocks.len()).then(|| self.first_slot(slot.block + 1))
    }

    fn prev_slot(&self, slot: Slot) -> Option<Slot> {
        if let Some(BlockKind::Table(t)) = self.blocks.get(slot.block).map(|b| &b.kind) {
            if slot.col > 0 {
                return Some(Slot::cell(slot.block, slot.row, slot.col - 1));
            }
            if slot.row > 0 {
                return Some(Slot::cell(
                    slot.block,
                    slot.row - 1,
                    t.cols().saturating_sub(1),
                ));
            }
        }
        (slot.block > 0).then(|| self.last_slot(slot.block - 1))
    }

    fn slots_between(&self, start: Slot, end: Slot) -> Vec<Slot> {
        let mut out = vec![start];
        let mut cur = start;
        while cur < end {
            match self.next_slot(cur) {
                Some(next) if next <= end => {
                    out.push(next);
                    cur = next;
                }
                _ => break,
            }
        }
        out
    }

    fn clamp_pos(&self, pos: Pos) -> Pos {
        if self.blocks.is_empty() {
            return Pos::default();
        }
        let block = pos.slot.block.min(self.blocks.len() - 1);
        let slot = match &self.blocks[block].kind {
            BlockKind::Table(t) => {
                let row = pos.slot.row.min(t.rows.len().saturating_sub(1));
                let col = pos.slot.col.min(t.cols().saturating_sub(1));
                Slot::cell(block, row, col)
            }
            _ => Slot::block(block),
        };
        let text = self.slot_text(slot);
        let mut offset = pos.offset.min(text.map_or(0, RichText::len));
        if let Some(t) = text {
            while offset > 0 && !t.text.is_char_boundary(offset) {
                offset -= 1;
            }
        }
        Pos::new(slot, offset)
    }

    fn doc_start(&self) -> Pos {
        Pos::new(self.first_slot(0), 0)
    }

    fn doc_end(&self) -> Pos {
        let slot = self.last_slot(self.blocks.len().saturating_sub(1));
        Pos::new(slot, self.slot_len(slot))
    }
}

/// Byte offset of the grapheme boundary before `offset`.
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(i, _)| (i < offset).then_some(i))
        .unwrap_or(0)
}

/// Byte offset of the grapheme boundary after `offset`.
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(i, _)| (i > offset).then_some(i))
        .unwrap_or(text.len())
}

/// Start of the word at or before `offset` (for Ctrl+Left).
pub fn prev_word(text: &str, offset: usize) -> usize {
    let mut last = 0;
    for (i, w) in text.split_word_bound_indices() {
        if i >= offset {
            break;
        }
        if w.chars().any(char::is_alphanumeric) {
            last = i;
        }
    }
    last
}

/// End of the word at or after `offset` (for Ctrl+Right).
pub fn next_word(text: &str, offset: usize) -> usize {
    for (i, w) in text.split_word_bound_indices() {
        let end = i + w.len();
        if end > offset && w.chars().any(char::is_alphanumeric) {
            return end;
        }
    }
    text.len()
}

/// Byte range of the word containing `offset` (for double-click).
pub fn word_range(text: &str, offset: usize) -> std::ops::Range<usize> {
    for (i, w) in text.split_word_bound_indices() {
        if offset >= i && offset < i + w.len() {
            return i..i + w.len();
        }
    }
    let end = text.len();
    prev_word(text, end)..end
}

pub fn utf8_to_utf16(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .chars()
        .map(char::len_utf16)
        .sum()
}

pub fn utf16_to_utf8(text: &str, offset16: usize) -> usize {
    let mut units = 0;
    for (i, ch) in text.char_indices() {
        if units >= offset16 {
            return i;
        }
        units += ch.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use cardfile_doc::{Block, Table};

    use super::*;

    fn doc() -> Document {
        Document {
            version: 1,
            blocks: vec![
                Block::paragraph("one"),
                Block::new(BlockKind::Table(Table::new(2, 2)), ""),
                Block::new(BlockKind::Divider, ""),
                Block::paragraph("end"),
            ],
        }
    }

    #[test]
    fn slot_navigation() {
        let d = doc();
        assert_eq!(d.next_slot(Slot::block(0)), Some(Slot::cell(1, 0, 0)));
        assert_eq!(d.next_slot(Slot::cell(1, 0, 1)), Some(Slot::cell(1, 1, 0)));
        assert_eq!(d.next_slot(Slot::cell(1, 1, 1)), Some(Slot::block(2)));
        assert_eq!(d.prev_slot(Slot::block(2)), Some(Slot::cell(1, 1, 1)));
        assert_eq!(d.prev_slot(Slot::block(0)), None);
        assert!(d.is_atom(Slot::block(2)));
        assert_eq!(d.slots_between(Slot::block(0), Slot::block(2)).len(), 6);
        assert_eq!(d.doc_end(), Pos::new(Slot::block(3), 3));
    }

    #[test]
    fn clamps_positions() {
        let d = doc();
        assert_eq!(
            d.clamp_pos(Pos::new(Slot::block(0), 99)),
            Pos::new(Slot::block(0), 3)
        );
        assert_eq!(
            d.clamp_pos(Pos::new(Slot::block(1), 0)).slot,
            Slot::cell(1, 0, 0)
        );
        assert_eq!(
            d.clamp_pos(Pos::new(Slot::block(9), 0)).slot,
            Slot::block(3)
        );
    }

    #[test]
    fn text_boundaries() {
        assert_eq!(prev_grapheme("ae\u{301}b", 4), 1);
        assert_eq!(next_grapheme("ae\u{301}b", 1), 4);
        assert_eq!(prev_word("hello big world", 15), 10);
        assert_eq!(next_word("hello big world", 5), 9);
        assert_eq!(word_range("hello big world", 7), 6..9);
        assert_eq!(utf8_to_utf16("añ😀b", 7), 4);
        assert_eq!(utf16_to_utf8("añ😀b", 4), 7);
    }
}
