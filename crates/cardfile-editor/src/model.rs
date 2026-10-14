//! Pure editing operations: a [`Document`] plus a [`Selection`].
//!
//! Nothing here knows about GPUI, which keeps the behaviour unit-testable. The view
//! layer translates keys and mouse input into calls on [`Model`].

use cardfile_doc::rules::{block_rule_on_enter, block_rule_on_space, inline_rule};
use cardfile_doc::{
    Align, Block, BlockKind, Document, MAX_INDENT, Mark, RichText, Style, Table, TextSize,
};

use crate::pos::{Pos, Selection, Slot, Slots, next_grapheme, prev_grapheme};

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub doc: Document,
    pub sel: Selection,
    /// Style for the next typed text when the caret moved or a mark was toggled with
    /// an empty selection. Cleared when the caret moves.
    pub pending_style: Option<Style>,
    /// Apply Markdown-style input rules while typing.
    pub input_rules: bool,
}

impl Model {
    pub fn new(mut doc: Document) -> Self {
        doc.normalize();
        let sel = Selection::caret(doc.doc_start());
        Self {
            doc,
            sel,
            pending_style: None,
            input_rules: true,
        }
    }

    pub fn caret(&self) -> Pos {
        self.sel.head
    }

    pub fn set_selection(&mut self, sel: Selection) {
        self.sel = Selection {
            anchor: self.doc.clamp_pos(sel.anchor),
            head: self.doc.clamp_pos(sel.head),
        };
        self.pending_style = None;
    }

    pub fn set_caret(&mut self, pos: Pos) {
        self.set_selection(Selection::caret(pos));
    }

    /// Extend the selection's head to `pos`.
    pub fn select_to(&mut self, pos: Pos) {
        let anchor = self.sel.anchor;
        self.set_selection(Selection { anchor, head: pos });
    }

    pub fn select_all(&mut self) {
        self.set_selection(Selection {
            anchor: self.doc.doc_start(),
            head: self.doc.doc_end(),
        });
    }

    fn block_kind(&self, block: usize) -> Option<&BlockKind> {
        self.doc.blocks.get(block).map(|b| &b.kind)
    }

    /// Style at the caret: the pending style, or the style of the preceding character.
    pub fn caret_style(&self) -> Style {
        if let Some(s) = &self.pending_style {
            return s.clone();
        }
        let pos = self.sel.start();
        self.doc
            .slot_text(pos.slot)
            .map(|t| t.style_at(pos.offset))
            .unwrap_or_default()
    }

    /// Whether `mark` is active for the toolbar.
    pub fn has_mark(&self, mark: Mark) -> bool {
        if self.sel.is_empty() || !self.sel.is_single_slot() {
            return mark.get(&self.caret_style());
        }
        let (s, e) = (self.sel.start(), self.sel.end());
        self.doc
            .slot_text(s.slot)
            .is_some_and(|t| t.has_mark(s.offset..e.offset, mark))
    }

    // ---------------------------------------------------------------------
    // Text insertion and deletion
    // ---------------------------------------------------------------------

    /// Delete the selected content. Returns `true` if anything changed.
    pub fn delete_selection(&mut self) -> bool {
        if self.sel.is_empty() {
            return false;
        }
        let (start, end) = (self.sel.start(), self.sel.end());
        if start.slot == end.slot {
            if let Some(t) = self.doc.slot_text_mut(start.slot) {
                t.delete(start.offset..end.offset);
            } else if self.doc.is_atom(start.slot) {
                self.remove_block(start.slot.block);
                return true;
            }
            self.sel = Selection::caret(start);
            return true;
        }

        // Trim the first and last slots, clear or remove what lies between.
        let slots = self.doc.slots_between(start.slot, end.slot);
        let mut remove_blocks: Vec<usize> = Vec::new();
        for slot in &slots {
            let is_first = *slot == start.slot;
            let is_last = *slot == end.slot;
            if self.doc.is_atom(*slot) {
                remove_blocks.push(slot.block);
                continue;
            }
            let Some(text) = self.doc.slot_text_mut(*slot) else {
                continue;
            };
            if is_first {
                let len = text.len();
                text.delete(start.offset..len);
            } else if is_last {
                text.delete(0..end.offset);
            } else {
                let len = text.len();
                text.delete(0..len);
            }
        }
        // Whole blocks strictly inside the selection disappear (tables included when
        // every cell was covered).
        for b in (start.slot.block + 1)..end.slot.block {
            if !remove_blocks.contains(&b) {
                remove_blocks.push(b);
            }
        }
        let merge = !self.doc.is_cell(start.slot)
            && !self.doc.is_cell(end.slot)
            && !self.doc.is_atom(start.slot)
            && !self.doc.is_atom(end.slot)
            && start.slot.block != end.slot.block;
        if merge {
            let tail = self.doc.blocks[end.slot.block].text.clone();
            self.doc.blocks[start.slot.block].text.append(&tail);
            remove_blocks.push(end.slot.block);
        }
        remove_blocks.sort_unstable();
        remove_blocks.dedup();
        let mut caret = start;
        let start_removed = remove_blocks.contains(&start.slot.block);
        let end_removed = remove_blocks.contains(&end.slot.block);
        let removed_before_end = remove_blocks
            .iter()
            .filter(|b| **b < end.slot.block)
            .count();
        for b in remove_blocks.into_iter().rev() {
            self.doc.blocks.remove(b);
        }
        if start_removed {
            if end_removed {
                // Nothing left of the selection: put an empty paragraph in its place.
                let at = start.slot.block.min(self.doc.blocks.len());
                self.doc.blocks.insert(at, Block::paragraph(""));
                caret = Pos::new(Slot::block(at), 0);
            } else {
                // The trimmed end slot now takes the selection's place.
                let block = end.slot.block - removed_before_end;
                caret = Pos::new(Slot { block, ..end.slot }, 0);
            }
        }
        self.ensure_not_empty();
        self.sel = Selection::caret(self.doc.clamp_pos(caret));
        true
    }

    fn ensure_not_empty(&mut self) {
        if self.doc.blocks.is_empty() {
            self.doc.blocks.push(Block::paragraph(""));
        }
    }

    fn remove_block(&mut self, block: usize) {
        self.doc.blocks.remove(block);
        self.ensure_not_empty();
        let target = block.min(self.doc.blocks.len() - 1);
        let pos = if block < self.doc.blocks.len() {
            Pos::new(self.doc.first_slot(target), 0)
        } else {
            let slot = self.doc.last_slot(target);
            Pos::new(slot, self.doc.slot_len(slot))
        };
        self.sel = Selection::caret(pos);
    }

    /// Insert typed or pasted plain text at the selection.
    ///
    /// Newlines split blocks, except in code blocks and table cells where they stay
    /// literal. Input rules fire for single typed characters.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let style = self.caret_style();
        self.delete_selection();
        let pos = self.sel.head;
        if self.doc.is_atom(pos.slot) {
            // Typing on a selected atom: continue in a new paragraph after it.
            let at = pos.slot.block + 1;
            self.doc.blocks.insert(at, Block::paragraph(""));
            self.sel = Selection::caret(Pos::new(Slot::block(at), 0));
        }
        let literal_newlines = self.doc.is_cell(self.sel.head.slot)
            || matches!(
                self.block_kind(self.sel.head.slot.block),
                Some(BlockKind::Code { .. })
            );
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut first = true;
        for line in normalized.split('\n') {
            if !first {
                if literal_newlines {
                    self.insert_raw("\n", &style);
                } else {
                    self.split_block();
                }
            }
            first = false;
            self.insert_raw(line, &style);
        }
        let single_char = text.chars().count() == 1;
        if self.input_rules && single_char {
            self.apply_input_rules(text);
        }
    }

    fn insert_raw(&mut self, text: &str, style: &Style) {
        if text.is_empty() {
            return;
        }
        let pos = self.sel.head;
        let allows_marks = self
            .block_kind(pos.slot.block)
            .is_none_or(|k| k.allows_marks() || self.doc.is_cell(pos.slot));
        let style = if allows_marks {
            style.clone()
        } else {
            Style::default()
        };
        if let Some(t) = self.doc.slot_text_mut(pos.slot) {
            t.insert(pos.offset, text, &style);
            let caret = Pos::new(pos.slot, pos.offset + text.len());
            self.sel = Selection::caret(caret);
        }
    }

    fn apply_input_rules(&mut self, typed: &str) {
        let pos = self.sel.head;
        if self.doc.is_cell(pos.slot) {
            return;
        }
        let Some(block) = self.doc.blocks.get(pos.slot.block) else {
            return;
        };
        if matches!(block.kind, BlockKind::Code { .. }) {
            return;
        }
        if typed == " " && matches!(block.kind, BlockKind::Paragraph) {
            let before = &block.text.text[..pos.offset];
            if let Some(rule) = block_rule_on_space(before) {
                let block = &mut self.doc.blocks[pos.slot.block];
                block.text.delete(0..rule.strip);
                block.kind = rule.kind;
                self.sel = Selection::caret(Pos::new(pos.slot, pos.offset - rule.strip));
                return;
            }
        }
        if let Some(rule) = inline_rule(&block.text.text, pos.offset) {
            let block = &mut self.doc.blocks[pos.slot.block];
            block.text.delete(rule.close.clone());
            block.text.delete(rule.open.clone());
            block
                .text
                .update_style(rule.content.clone(), |s| rule.mark.set(s, true));
            // Typing continues without the mark.
            let mut style = block.text.style_at(rule.content.end);
            rule.mark.set(&mut style, false);
            self.sel = Selection::caret(Pos::new(pos.slot, rule.content.end));
            self.pending_style = Some(style);
        }
    }

    /// Enter: split the block at the caret, or leave an empty list item.
    pub fn enter(&mut self) {
        self.delete_selection();
        let pos = self.sel.head;
        if self.doc.is_cell(pos.slot) {
            self.insert_raw("\n", &self.caret_style());
            return;
        }
        if self.doc.is_atom(pos.slot) {
            let at = pos.slot.block + 1;
            self.doc.blocks.insert(at, Block::paragraph(""));
            self.sel = Selection::caret(Pos::new(Slot::block(at), 0));
            return;
        }
        let block = &self.doc.blocks[pos.slot.block];
        if let BlockKind::Code { .. } = block.kind {
            // Enter on an empty last line leaves the code block.
            if block.text.text.ends_with('\n') && pos.offset == block.text.len() {
                let len = block.text.len();
                self.doc.blocks[pos.slot.block].text.delete(len - 1..len);
                let at = pos.slot.block + 1;
                self.doc.blocks.insert(at, Block::paragraph(""));
                self.sel = Selection::caret(Pos::new(Slot::block(at), 0));
            } else {
                self.insert_raw("\n", &Style::default());
            }
            return;
        }
        if block.kind.is_list() && block.text.is_empty() {
            let block = &mut self.doc.blocks[pos.slot.block];
            if block.indent > 0 {
                block.indent -= 1;
            } else {
                block.kind = BlockKind::Paragraph;
            }
            return;
        }
        if self.input_rules
            && matches!(block.kind, BlockKind::Paragraph)
            && pos.offset == block.text.len()
            && let Some(rule) = block_rule_on_enter(&block.text.text)
        {
            let is_divider = matches!(rule.kind, BlockKind::Divider);
            let block = &mut self.doc.blocks[pos.slot.block];
            block.text = RichText::default();
            block.kind = rule.kind;
            if is_divider {
                let at = pos.slot.block + 1;
                self.doc.blocks.insert(at, Block::paragraph(""));
                self.sel = Selection::caret(Pos::new(Slot::block(at), 0));
            } else {
                self.sel = Selection::caret(Pos::new(pos.slot, 0));
            }
            return;
        }
        self.split_block();
    }

    /// Shift+Enter: line break inside the block.
    pub fn soft_break(&mut self) {
        let style = self.caret_style();
        self.delete_selection();
        if !self.doc.is_atom(self.sel.head.slot) {
            self.insert_raw("\n", &style);
        }
    }

    fn split_block(&mut self) {
        let pos = self.sel.head;
        if self.doc.is_cell(pos.slot) || self.doc.is_atom(pos.slot) {
            return;
        }
        let block = &mut self.doc.blocks[pos.slot.block];
        let tail = block.text.split_off(pos.offset);
        let mut new = Block::new(block.kind.clone(), tail);
        new.indent = block.indent;
        new.align = block.align;
        new.size = block.size;
        match &mut new.kind {
            BlockKind::Heading(_) => {
                // Splitting at the very start keeps the heading below; otherwise the
                // new line is body text.
                if pos.offset > 0 {
                    new.kind = BlockKind::Paragraph;
                    new.size = TextSize::Normal;
                }
            }
            BlockKind::CheckList { checked } => *checked = false,
            _ => {}
        }
        if pos.offset == 0 && !new.text.is_empty() {
            // Caret at start: the current block becomes the empty one above.
            let kind = self.doc.blocks[pos.slot.block].kind.clone();
            new.kind = kind;
        }
        let at = pos.slot.block + 1;
        self.doc.blocks.insert(at, new);
        self.sel = Selection::caret(Pos::new(Slot::block(at), 0));
    }

    /// Backspace.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let pos = self.sel.head;
        if self.doc.is_atom(pos.slot) {
            self.remove_block(pos.slot.block);
            return;
        }
        if pos.offset > 0 {
            if let Some(t) = self.doc.slot_text_mut(pos.slot) {
                let prev = prev_grapheme(&t.text, pos.offset);
                t.delete(prev..pos.offset);
                self.sel = Selection::caret(Pos::new(pos.slot, prev));
            }
            return;
        }
        if self.doc.is_cell(pos.slot) {
            return;
        }
        // At the start of a block.
        let block = &mut self.doc.blocks[pos.slot.block];
        if block.indent > 0 {
            block.indent -= 1;
            return;
        }
        if !matches!(block.kind, BlockKind::Paragraph) {
            block.kind = BlockKind::Paragraph;
            block.size = TextSize::Normal;
            return;
        }
        if pos.slot.block == 0 {
            return;
        }
        let prev = pos.slot.block - 1;
        match &self.doc.blocks[prev].kind {
            BlockKind::Divider | BlockKind::Image(_) | BlockKind::Attachment(_) => {
                self.doc.blocks.remove(prev);
                self.sel = Selection::caret(Pos::new(Slot::block(prev), 0));
            }
            BlockKind::Table(_) => {
                if self.doc.blocks[pos.slot.block].text.is_empty() {
                    self.doc.blocks.remove(pos.slot.block);
                }
                let slot = self.doc.last_slot(prev);
                self.sel = Selection::caret(Pos::new(slot, self.doc.slot_len(slot)));
            }
            _ => {
                let text = self.doc.blocks.remove(pos.slot.block).text;
                let target = &mut self.doc.blocks[prev].text;
                let offset = target.len();
                target.append(&text);
                self.sel = Selection::caret(Pos::new(Slot::block(prev), offset));
            }
        }
    }

    /// Delete (forward).
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let pos = self.sel.head;
        if self.doc.is_atom(pos.slot) {
            self.remove_block(pos.slot.block);
            return;
        }
        let len = self.doc.slot_len(pos.slot);
        if pos.offset < len {
            if let Some(t) = self.doc.slot_text_mut(pos.slot) {
                let next = next_grapheme(&t.text, pos.offset);
                t.delete(pos.offset..next);
            }
            return;
        }
        if self.doc.is_cell(pos.slot) {
            return;
        }
        let next = pos.slot.block + 1;
        let Some(next_block) = self.doc.blocks.get(next) else {
            return;
        };
        match next_block.kind {
            BlockKind::Divider | BlockKind::Image(_) | BlockKind::Attachment(_) => {
                self.doc.blocks.remove(next);
            }
            BlockKind::Table(_) => {}
            _ => {
                let text = self.doc.blocks.remove(next).text;
                self.doc.blocks[pos.slot.block].text.append(&text);
            }
        }
    }

    // ---------------------------------------------------------------------
    // Formatting
    // ---------------------------------------------------------------------

    /// Apply `f` to the style of the selection, or to the pending style when empty.
    pub fn update_style(&mut self, f: impl Fn(&mut Style)) {
        if self.sel.is_empty() {
            let mut style = self.caret_style();
            f(&mut style);
            self.pending_style = Some(style);
            return;
        }
        let (start, end) = (self.sel.start(), self.sel.end());
        for slot in self.doc.slots_between(start.slot, end.slot) {
            if matches!(self.block_kind(slot.block), Some(BlockKind::Code { .. })) {
                continue;
            }
            let Some(t) = self.doc.slot_text_mut(slot) else {
                continue;
            };
            let s = if slot == start.slot { start.offset } else { 0 };
            let e = if slot == end.slot {
                end.offset
            } else {
                t.len()
            };
            t.update_style(s..e, &f);
        }
    }

    pub fn toggle_mark(&mut self, mark: Mark) {
        let on = !self.has_mark(mark);
        self.update_style(|s| mark.set(s, on));
    }

    pub fn set_color(&mut self, color: Option<String>) {
        self.update_style(|s| s.color = color.clone());
    }

    pub fn set_highlight(&mut self, color: Option<String>) {
        self.update_style(|s| s.highlight = color.clone());
    }

    /// Set or remove a link over the selection. With an empty selection, the link text
    /// itself is inserted.
    pub fn set_link(&mut self, url: Option<String>, text_if_empty: Option<&str>) {
        if self.sel.is_empty() {
            if let (Some(url), Some(text)) = (&url, text_if_empty) {
                let mut style = self.caret_style();
                style.link = Some(url.clone());
                let pos = self.sel.head;
                if let Some(t) = self.doc.slot_text_mut(pos.slot) {
                    t.insert(pos.offset, text, &style);
                    self.sel = Selection {
                        anchor: pos,
                        head: Pos::new(pos.slot, pos.offset + text.len()),
                    };
                }
                return;
            }
            // Caret inside a link: remove the whole link run.
            if url.is_none() {
                let pos = self.sel.head;
                let range = self
                    .doc
                    .slot_text(pos.slot)
                    .and_then(|t| link_extent(t, pos.offset));
                if let (Some(range), Some(t)) = (range, self.doc.slot_text_mut(pos.slot)) {
                    t.update_style(range, |s| s.link = None);
                }
                return;
            }
        }
        self.update_style(|s| s.link = url.clone());
    }

    /// The link under the caret, if any.
    pub fn link_at_caret(&self) -> Option<String> {
        let pos = self.sel.head;
        let t = self.doc.slot_text(pos.slot)?;
        t.spans()
            .find(|(r, s)| {
                s.link.is_some() && r.start <= pos.offset && pos.offset < r.end.max(r.start + 1)
            })
            .and_then(|(_, s)| s.link.clone())
    }

    pub fn clear_formatting(&mut self) {
        self.update_style(|s| *s = Style::default());
    }

    /// Indices of the blocks touched by the selection.
    pub fn selected_blocks(&self) -> std::ops::RangeInclusive<usize> {
        self.sel.start().slot.block..=self.sel.end().slot.block
    }

    /// Change the kind of every selected textual block. Choosing the kind they already
    /// all have turns them back into paragraphs.
    pub fn set_block_kind(&mut self, kind: BlockKind) {
        let range = self.selected_blocks();
        let all_same = range.clone().all(|b| {
            self.doc.blocks.get(b).is_none_or(|blk| {
                !blk.kind.is_textual()
                    || std::mem::discriminant(&blk.kind) == std::mem::discriminant(&kind)
                        && match (&blk.kind, &kind) {
                            (BlockKind::Heading(a), BlockKind::Heading(b)) => a == b,
                            _ => true,
                        }
            })
        });
        let target = if all_same { BlockKind::Paragraph } else { kind };
        for b in range {
            let Some(block) = self.doc.blocks.get_mut(b) else {
                continue;
            };
            if !block.kind.is_textual() {
                continue;
            }
            if matches!(target, BlockKind::Code { .. }) {
                // Code blocks hold plain text.
                block.text = RichText::plain(&block.text.text);
            }
            if matches!(target, BlockKind::Heading(_)) {
                block.size = TextSize::Normal;
            }
            block.kind = target.clone();
        }
        self.sel = Selection {
            anchor: self.doc.clamp_pos(self.sel.anchor),
            head: self.doc.clamp_pos(self.sel.head),
        };
    }

    pub fn current_block_kind(&self) -> Option<BlockKind> {
        let pos = self.sel.head;
        if self.doc.is_cell(pos.slot) {
            return None;
        }
        self.block_kind(pos.slot.block).cloned()
    }

    pub fn set_align(&mut self, align: Align) {
        for b in self.selected_blocks() {
            if let Some(block) = self.doc.blocks.get_mut(b) {
                block.align = align;
            }
        }
    }

    pub fn set_size(&mut self, size: TextSize) {
        for b in self.selected_blocks() {
            if let Some(block) = self.doc.blocks.get_mut(b)
                && block.kind.is_textual()
                && !matches!(block.kind, BlockKind::Heading(_) | BlockKind::Code { .. })
            {
                block.size = size;
            }
        }
    }

    /// Indent (`delta > 0`) or outdent the selected blocks.
    pub fn indent(&mut self, delta: i8) {
        for b in self.selected_blocks() {
            if let Some(block) = self.doc.blocks.get_mut(b)
                && block.kind.is_textual()
            {
                let v = i16::from(block.indent) + i16::from(delta);
                block.indent = u8::try_from(v.clamp(0, i16::from(MAX_INDENT))).unwrap_or(0);
            }
        }
    }

    pub fn toggle_check(&mut self, block: usize) {
        if let Some(Block {
            kind: BlockKind::CheckList { checked },
            ..
        }) = self.doc.blocks.get_mut(block)
        {
            *checked = !*checked;
        }
    }

    // ---------------------------------------------------------------------
    // Block insertion
    // ---------------------------------------------------------------------

    /// Insert a non-text block (divider, image, table, attachment) after the caret's
    /// block, replacing it if it is an empty paragraph. The caret moves after it.
    pub fn insert_block(&mut self, block: Block) {
        self.delete_selection();
        let pos = self.sel.head;
        let current = pos.slot.block;
        let replace = !self.doc.is_cell(pos.slot)
            && self
                .doc
                .blocks
                .get(current)
                .is_some_and(|b| matches!(b.kind, BlockKind::Paragraph) && b.text.is_empty());
        let at = if replace {
            self.doc.blocks[current] = block;
            current
        } else {
            if !self.doc.is_cell(pos.slot)
                && !self.doc.is_atom(pos.slot)
                && pos.offset > 0
                && pos.offset < self.doc.slot_len(pos.slot)
            {
                self.split_block();
                self.doc.blocks.insert(current + 1, block);
                self.sel = Selection::caret(Pos::new(Slot::block(current + 2), 0));
                return;
            }
            let at = if pos.offset == 0
                && !self.doc.is_cell(pos.slot)
                && !self.doc.is_atom(pos.slot)
                && self.doc.slot_len(pos.slot) > 0
            {
                current
            } else {
                current + 1
            };
            self.doc.blocks.insert(at, block);
            at
        };
        let is_table = matches!(self.doc.blocks[at].kind, BlockKind::Table(_));
        if is_table {
            self.sel = Selection::caret(Pos::new(Slot::cell(at, 0, 0), 0));
            if at + 1 >= self.doc.blocks.len() {
                self.doc.blocks.push(Block::paragraph(""));
            }
            return;
        }
        if at + 1 >= self.doc.blocks.len() || !self.doc.blocks[at + 1].kind.is_textual() {
            self.doc.blocks.insert(at + 1, Block::paragraph(""));
        }
        self.sel = Selection::caret(Pos::new(Slot::block(at + 1), 0));
    }

    pub fn insert_table(&mut self, rows: usize, cols: usize) {
        self.insert_block(Block::new(BlockKind::Table(Table::new(rows, cols)), ""));
    }

    // ---------------------------------------------------------------------
    // Tables
    // ---------------------------------------------------------------------

    fn table_at_caret(&mut self) -> Option<(Slot, &mut Table)> {
        let slot = self.sel.head.slot;
        match &mut self.doc.blocks.get_mut(slot.block)?.kind {
            BlockKind::Table(t) => Some((slot, t)),
            _ => None,
        }
    }

    pub fn in_table(&self) -> bool {
        self.doc.is_cell(self.sel.head.slot)
    }

    pub fn table_insert_row(&mut self, below: bool) {
        let Some((slot, t)) = self.table_at_caret() else {
            return;
        };
        let at = if below { slot.row + 1 } else { slot.row };
        let cols = t.cols();
        t.rows.insert(at, vec![RichText::default(); cols]);
        self.set_caret(Pos::new(Slot::cell(slot.block, at, slot.col), 0));
    }

    pub fn table_insert_col(&mut self, right: bool) {
        let Some((slot, t)) = self.table_at_caret() else {
            return;
        };
        let at = if right { slot.col + 1 } else { slot.col };
        for row in &mut t.rows {
            row.insert(at.min(row.len()), RichText::default());
        }
        self.set_caret(Pos::new(Slot::cell(slot.block, slot.row, at), 0));
    }

    pub fn table_delete_row(&mut self) {
        let Some((slot, t)) = self.table_at_caret() else {
            return;
        };
        if t.rows.len() <= 1 {
            self.table_delete();
            return;
        }
        t.rows.remove(slot.row);
        let row = slot.row.min(t.rows.len() - 1);
        self.set_caret(Pos::new(Slot::cell(slot.block, row, slot.col), 0));
    }

    pub fn table_delete_col(&mut self) {
        let Some((slot, t)) = self.table_at_caret() else {
            return;
        };
        if t.cols() <= 1 {
            self.table_delete();
            return;
        }
        for row in &mut t.rows {
            if slot.col < row.len() {
                row.remove(slot.col);
            }
        }
        let col = slot.col.min(t.cols() - 1);
        self.set_caret(Pos::new(Slot::cell(slot.block, slot.row, col), 0));
    }

    pub fn table_toggle_header(&mut self) {
        if let Some((_, t)) = self.table_at_caret() {
            t.header = !t.header;
        }
    }

    pub fn table_delete(&mut self) {
        let block = self.sel.head.slot.block;
        if self.doc.is_cell(Slot::block(block)) {
            self.remove_block(block);
        }
    }

    /// Whether Tab can move to another cell without adding a row.
    pub fn has_next_cell(&self) -> bool {
        let slot = self.sel.head.slot;
        self.doc.is_cell(slot)
            && self
                .doc
                .next_slot(slot)
                .is_some_and(|t| t.block == slot.block)
    }

    /// Tab inside a table moves to the next cell, adding a row at the end.
    pub fn table_next_cell(&mut self, backwards: bool) -> bool {
        let slot = self.sel.head.slot;
        if !self.doc.is_cell(slot) {
            return false;
        }
        let target = if backwards {
            self.doc.prev_slot(slot)
        } else {
            self.doc.next_slot(slot)
        };
        match target {
            Some(t) if t.block == slot.block => {
                let len = self.doc.slot_len(t);
                self.set_selection(Selection {
                    anchor: Pos::new(t, 0),
                    head: Pos::new(t, len),
                });
            }
            _ if !backwards => {
                self.table_insert_row(true);
                let slot = self.sel.head.slot;
                self.set_caret(Pos::new(Slot::cell(slot.block, slot.row, 0), 0));
            }
            _ => {}
        }
        true
    }

    // ---------------------------------------------------------------------
    // Clipboard fragments
    // ---------------------------------------------------------------------

    /// The selection as a standalone document.
    pub fn selected_fragment(&self) -> Option<Document> {
        if self.sel.is_empty() {
            return None;
        }
        let (start, end) = (self.sel.start(), self.sel.end());
        let mut blocks = Vec::new();
        if start.slot == end.slot {
            let block = &self.doc.blocks[start.slot.block];
            if self.doc.is_atom(start.slot) {
                blocks.push(block.clone());
            } else if let Some(t) = self.doc.slot_text(start.slot) {
                let mut b = if self.doc.is_cell(start.slot) {
                    Block::paragraph("")
                } else {
                    Block {
                        text: RichText::default(),
                        ..block.clone()
                    }
                };
                b.text = t.slice(start.offset..end.offset);
                blocks.push(b);
            }
        } else {
            for b in start.slot.block..=end.slot.block {
                let block = &self.doc.blocks[b];
                let inner = b > start.slot.block && b < end.slot.block;
                match &block.kind {
                    BlockKind::Table(_) if inner => blocks.push(block.clone()),
                    BlockKind::Table(t) => {
                        // Partially selected table: copy covered cells as paragraphs.
                        for (r, row) in t.rows.iter().enumerate() {
                            for (c, cell) in row.iter().enumerate() {
                                let slot = Slot::cell(b, r, c);
                                if slot < start.slot || slot > end.slot {
                                    continue;
                                }
                                let s = if slot == start.slot { start.offset } else { 0 };
                                let e = if slot == end.slot {
                                    end.offset
                                } else {
                                    cell.len()
                                };
                                blocks.push(Block::new(BlockKind::Paragraph, cell.slice(s..e)));
                            }
                        }
                    }
                    k if !k.is_textual() => blocks.push(block.clone()),
                    _ => {
                        let s = if b == start.slot.block {
                            start.offset
                        } else {
                            0
                        };
                        let e = if b == end.slot.block {
                            end.offset
                        } else {
                            block.text.len()
                        };
                        blocks.push(Block {
                            text: block.text.slice(s..e),
                            ..block.clone()
                        });
                    }
                }
            }
        }
        Some(Document {
            version: cardfile_doc::DOC_VERSION,
            blocks,
        })
    }

    /// Plain text of the selection.
    pub fn selected_text(&self) -> String {
        self.selected_fragment()
            .map(|d| cardfile_doc::to_plain_text(&d))
            .unwrap_or_default()
    }

    /// Paste a document fragment at the selection.
    pub fn insert_fragment(&mut self, frag: &Document) {
        if frag.blocks.is_empty() {
            return;
        }
        self.delete_selection();
        let pos = self.sel.head;
        let into_plain_slot = self.doc.is_cell(pos.slot)
            || matches!(
                self.block_kind(pos.slot.block),
                Some(BlockKind::Code { .. })
            );
        let single_inline = frag.blocks.len() == 1 && frag.blocks[0].kind.is_textual();
        if into_plain_slot || single_inline {
            if single_inline && !into_plain_slot {
                let text = frag.blocks[0].text.clone();
                if let Some(t) = self.doc.slot_text_mut(pos.slot) {
                    let tail = t.split_off(pos.offset);
                    t.append(&text);
                    let caret = t.len();
                    t.append(&tail);
                    self.sel = Selection::caret(Pos::new(pos.slot, caret));
                }
            } else {
                let text = cardfile_doc::to_plain_text(frag);
                self.insert_text(&text);
            }
            return;
        }
        if self.doc.is_atom(pos.slot) {
            let at = pos.slot.block + 1;
            self.doc.blocks.splice(at..at, frag.blocks.iter().cloned());
            let last = at + frag.blocks.len() - 1;
            let slot = self.doc.last_slot(last);
            self.sel = Selection::caret(Pos::new(slot, self.doc.slot_len(slot)));
            return;
        }
        // Split the current block and splice the fragment in between.
        let cur = pos.slot.block;
        let tail = self.doc.blocks[cur].text.split_off(pos.offset);
        let mut blocks = frag.blocks.clone();
        let mut insert_at = cur + 1;
        if blocks[0].kind.is_textual() {
            let first = blocks.remove(0);
            let head = &mut self.doc.blocks[cur];
            if head.text.is_empty() {
                head.kind = first.kind.clone();
                head.indent = first.indent;
                head.align = first.align;
                head.size = first.size;
            }
            head.text.append(&first.text);
        }
        let n = blocks.len();
        self.doc.blocks.splice(insert_at..insert_at, blocks);
        insert_at += n;
        let last = insert_at - 1;
        let last_textual = self.doc.blocks[last].kind.is_textual()
            && !matches!(self.doc.blocks[last].kind, BlockKind::Table(_));
        if last_textual {
            let caret = self.doc.blocks[last].text.len();
            self.doc.blocks[last].text.append(&tail);
            self.sel = Selection::caret(Pos::new(Slot::block(last), caret));
        } else {
            let tail_kind = self.doc.blocks[cur].kind.clone();
            let mut b = Block::new(
                if tail_kind.is_textual() {
                    tail_kind
                } else {
                    BlockKind::Paragraph
                },
                tail,
            );
            b.indent = self.doc.blocks[cur].indent;
            self.doc.blocks.insert(insert_at, b);
            self.sel = Selection::caret(Pos::new(Slot::block(insert_at), 0));
        }
    }
}

impl Model {
    /// Every occurrence of `query`, in document order.
    pub fn find_matches(
        &self,
        query: &str,
        case_sensitive: bool,
    ) -> Vec<(Slot, std::ops::Range<usize>)> {
        let mut out = Vec::new();
        if query.is_empty() {
            return out;
        }
        let needle = if case_sensitive {
            query.to_owned()
        } else {
            fold_case(query).0
        };
        let mut slot = Some(self.doc.first_slot(0));
        while let Some(s) = slot {
            if let Some(text) = self.doc.slot_text(s) {
                let hay = &text.text;
                if case_sensitive {
                    out.extend(hay.match_indices(query).map(|(i, m)| (s, i..i + m.len())));
                } else {
                    // Lower-casing can change byte lengths ('İ' → "i̇"), so map offsets
                    // in the folded text back to the original.
                    let (folded, orig) = fold_case(hay);
                    for (i, m) in folded.match_indices(&needle) {
                        let (start, end) = (orig[i], orig[i + m.len()]);
                        if start != usize::MAX && end != usize::MAX {
                            out.push((s, start..end));
                        }
                    }
                }
            }
            slot = self.doc.next_slot(s);
        }
        out
    }

    /// Replace every match; returns how many were replaced.
    pub fn replace_all(&mut self, query: &str, replacement: &str, case_sensitive: bool) -> usize {
        let matches = self.find_matches(query, case_sensitive);
        let rules = self.input_rules;
        self.input_rules = false;
        for (slot, range) in matches.iter().rev() {
            self.sel = Selection {
                anchor: Pos::new(*slot, range.start),
                head: Pos::new(*slot, range.end),
            };
            if replacement.is_empty() {
                self.delete_selection();
            } else {
                // Keep the style of the replaced text.
                self.pending_style = self
                    .doc
                    .slot_text(*slot)
                    .map(|t| t.style_at(range.start + 1));
                self.insert_text(replacement);
            }
        }
        self.input_rules = rules;
        self.pending_style = None;
        matches.len()
    }
}

/// Byte range of the link touching `offset` (contiguous runs sharing one URL).
pub fn link_extent(text: &RichText, offset: usize) -> Option<std::ops::Range<usize>> {
    let spans: Vec<_> = text.spans().map(|(r, s)| (r, s.link.clone())).collect();
    let idx = spans
        .iter()
        .position(|(r, l)| l.is_some() && r.start <= offset && offset <= r.end)?;
    let url = spans[idx].1.clone();
    let mut start = spans[idx].0.start;
    let mut end = spans[idx].0.end;
    for (r, l) in spans[..idx].iter().rev() {
        if *l != url {
            break;
        }
        start = r.start;
    }
    for (r, l) in &spans[idx + 1..] {
        if *l != url {
            break;
        }
        end = r.end;
    }
    Some(start..end)
}

/// Lower-case `text` one char at a time. Returns the folded text and, for every byte
/// offset in it, the matching offset in `text` (`usize::MAX` inside a folded char).
fn fold_case(text: &str) -> (String, Vec<usize>) {
    let mut folded = String::with_capacity(text.len());
    let mut orig = Vec::with_capacity(text.len() + 1);
    for (i, ch) in text.char_indices() {
        let start = folded.len();
        folded.extend(ch.to_lowercase());
        orig.push(i);
        orig.resize(orig.len() + folded.len() - start - 1, usize::MAX);
    }
    orig.push(text.len());
    (folded, orig)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(paragraphs: &[&str]) -> Model {
        Model::new(Document {
            version: 1,
            blocks: paragraphs.iter().map(|p| Block::paragraph(p)).collect(),
        })
    }

    fn texts(m: &Model) -> Vec<String> {
        m.doc.blocks.iter().map(|b| b.text.text.clone()).collect()
    }

    fn at(block: usize, offset: usize) -> Pos {
        Pos::new(Slot::block(block), offset)
    }

    #[test]
    fn typing_and_enter() {
        let mut m = model(&[""]);
        m.insert_text("hello");
        m.enter();
        m.insert_text("world");
        assert_eq!(texts(&m), ["hello", "world"]);
        m.set_caret(at(0, 2));
        m.enter();
        assert_eq!(texts(&m), ["he", "llo", "world"]);
        assert_eq!(m.caret(), at(1, 0));
    }

    #[test]
    fn backspace_merges_and_converts() {
        let mut m = model(&["ab", "cd"]);
        m.set_caret(at(1, 0));
        m.backspace();
        assert_eq!(texts(&m), ["abcd"]);
        assert_eq!(m.caret(), at(0, 2));
        m.doc.blocks[0].kind = BlockKind::BulletList;
        m.doc.blocks[0].indent = 1;
        m.set_caret(at(0, 0));
        m.backspace();
        assert_eq!(m.doc.blocks[0].indent, 0);
        m.backspace();
        assert_eq!(m.doc.blocks[0].kind, BlockKind::Paragraph);
    }

    #[test]
    fn delete_multi_block_selection() {
        let mut m = model(&["hello", "middle", "world"]);
        m.set_selection(Selection {
            anchor: at(0, 2),
            head: at(2, 3),
        });
        m.backspace();
        assert_eq!(texts(&m), ["held"]);
        assert_eq!(m.caret(), at(0, 2));
    }

    #[test]
    fn typing_replaces_selection_and_keeps_style() {
        let mut m = model(&["hello world"]);
        m.set_selection(Selection {
            anchor: at(0, 0),
            head: at(0, 5),
        });
        m.toggle_mark(Mark::Bold);
        m.insert_text("HEY");
        assert_eq!(texts(&m), ["HEY world"]);
        assert!(m.doc.blocks[0].text.has_mark(0..3, Mark::Bold));
    }

    #[test]
    fn pending_style_with_empty_selection() {
        let mut m = model(&["ab"]);
        m.set_caret(at(0, 1));
        m.toggle_mark(Mark::Italic);
        assert!(m.has_mark(Mark::Italic));
        m.insert_text("X");
        assert!(m.doc.blocks[0].text.has_mark(1..2, Mark::Italic));
        assert!(!m.doc.blocks[0].text.has_mark(0..1, Mark::Italic));
    }

    #[test]
    fn markdown_block_shortcuts() {
        let mut m = model(&[""]);
        for ch in ["#", "#", " "] {
            m.insert_text(ch);
        }
        assert_eq!(m.doc.blocks[0].kind, BlockKind::Heading(2));
        assert_eq!(texts(&m), [""]);
        m.insert_text("T");
        m.enter();
        assert_eq!(m.doc.blocks[1].kind, BlockKind::Paragraph);
        for ch in ["-", " ", "a"] {
            m.insert_text(ch);
        }
        assert_eq!(m.doc.blocks[1].kind, BlockKind::BulletList);
        m.enter();
        assert_eq!(m.doc.blocks[2].kind, BlockKind::BulletList);
        // Enter on an empty item leaves the list.
        m.enter();
        assert_eq!(m.doc.blocks[2].kind, BlockKind::Paragraph);
        for ch in "---".chars() {
            m.insert_text(&ch.to_string());
        }
        m.enter();
        assert_eq!(m.doc.blocks[2].kind, BlockKind::Divider);
        assert_eq!(m.caret(), at(3, 0));
    }

    #[test]
    fn markdown_inline_shortcut() {
        let mut m = model(&[""]);
        for ch in "a **bold**".chars() {
            m.insert_text(&ch.to_string());
        }
        assert_eq!(texts(&m), ["a bold"]);
        assert!(m.doc.blocks[0].text.has_mark(2..6, Mark::Bold));
        m.insert_text("x");
        assert!(!m.doc.blocks[0].text.has_mark(6..7, Mark::Bold));
    }

    #[test]
    fn input_rules_can_be_disabled() {
        let mut m = model(&[""]);
        m.input_rules = false;
        for ch in ["#", " "] {
            m.insert_text(ch);
        }
        assert_eq!(m.doc.blocks[0].kind, BlockKind::Paragraph);
    }

    #[test]
    fn multiline_paste_splits_blocks() {
        let mut m = model(&["ab"]);
        m.set_caret(at(0, 1));
        m.insert_text("1\n2\n3");
        assert_eq!(texts(&m), ["a1", "2", "3b"]);
    }

    #[test]
    fn block_kind_toggle() {
        let mut m = model(&["a", "b"]);
        m.set_selection(Selection {
            anchor: at(0, 0),
            head: at(1, 1),
        });
        m.set_block_kind(BlockKind::NumberedList);
        assert!(
            m.doc
                .blocks
                .iter()
                .all(|b| b.kind == BlockKind::NumberedList)
        );
        m.set_block_kind(BlockKind::NumberedList);
        assert!(m.doc.blocks.iter().all(|b| b.kind == BlockKind::Paragraph));
    }

    #[test]
    fn tables() {
        let mut m = model(&["x"]);
        m.set_caret(at(0, 1));
        m.insert_table(2, 2);
        assert_eq!(m.caret().slot, Slot::cell(1, 0, 0));
        m.insert_text("A");
        m.enter();
        assert_eq!(m.doc.slot_text(Slot::cell(1, 0, 0)).unwrap().text, "A\n");
        assert!(m.table_next_cell(false));
        assert_eq!(m.caret().slot, Slot::cell(1, 0, 1));
        m.set_caret(Pos::new(Slot::cell(1, 1, 1), 0));
        m.table_next_cell(false);
        let BlockKind::Table(t) = &m.doc.blocks[1].kind else {
            panic!()
        };
        assert_eq!(t.rows.len(), 3);
        m.table_insert_col(true);
        m.table_delete_row();
        let BlockKind::Table(t) = &m.doc.blocks[1].kind else {
            panic!()
        };
        assert_eq!((t.rows.len(), t.cols()), (2, 3));
        // A trailing paragraph exists after the table.
        assert!(matches!(
            m.doc.blocks.last().unwrap().kind,
            BlockKind::Paragraph
        ));
    }

    #[test]
    fn atoms_insert_and_delete() {
        let mut m = model(&[""]);
        m.insert_block(Block::new(BlockKind::Divider, ""));
        assert_eq!(m.doc.blocks.len(), 2);
        assert_eq!(m.caret(), at(1, 0));
        m.backspace();
        assert_eq!(m.doc.blocks.len(), 1);
        assert_eq!(m.doc.blocks[0].kind, BlockKind::Paragraph);
    }

    #[test]
    fn fragment_copy_paste() {
        let mut m = model(&["hello", "big", "world"]);
        m.set_selection(Selection {
            anchor: at(0, 3),
            head: at(2, 2),
        });
        let frag = m.selected_fragment().unwrap();
        assert_eq!(cardfile_doc::to_plain_text(&frag), "lo\nbig\nwo");
        let mut n = model(&["AB"]);
        n.set_caret(at(0, 1));
        n.insert_fragment(&frag);
        assert_eq!(texts(&n), ["Alo", "big", "woB"]);
        assert_eq!(n.caret(), at(2, 2));
        // Inline fragment.
        let mut k = model(&["AB"]);
        k.set_caret(at(0, 1));
        k.insert_fragment(&Document::from_plain_text("x"));
        assert_eq!(texts(&k), ["AxB"]);
        assert_eq!(k.caret(), at(0, 2));
    }

    #[test]
    fn case_insensitive_find_across_length_changing_folds() {
        // 'İ' lower-cases to two chars (3 bytes) and the Kelvin sign 'K' to 'k'.
        let m = model(&["İstanbul \u{212A}elvin KELVIN"]);
        let found: Vec<_> = m
            .find_matches("kelvin", false)
            .into_iter()
            .map(|(_, r)| r)
            .collect();
        let text = &m.doc.blocks[0].text.text;
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|r| text[r.clone()].to_lowercase() == "kelvin")
        );
        let ist = m.find_matches("i̇stanbul", false);
        assert_eq!(ist.len(), 1);
        assert_eq!(&text[ist[0].1.clone()], "İstanbul");
        // A match ending inside a folded char is rejected.
        assert!(m.find_matches("i", false).iter().all(|(_, r)| r.start != 0));
    }

    #[test]
    fn find_and_replace() {
        let mut m = model(&["Hello world", "hello again"]);
        assert_eq!(m.find_matches("hello", false).len(), 2);
        assert_eq!(m.find_matches("hello", true).len(), 1);
        m.doc.blocks[0].text.toggle_mark(0..5, Mark::Bold);
        assert_eq!(m.replace_all("hello", "Hi", false), 2);
        assert_eq!(texts(&m), ["Hi world", "Hi again"]);
        assert!(m.doc.blocks[0].text.has_mark(0..2, Mark::Bold));
        assert_eq!(m.replace_all("missing", "x", false), 0);
    }

    #[test]
    fn links() {
        let mut m = model(&[""]);
        m.set_link(Some("https://a.b".into()), Some("site"));
        assert_eq!(texts(&m), ["site"]);
        m.set_caret(at(0, 2));
        assert_eq!(m.link_at_caret().as_deref(), Some("https://a.b"));
        m.set_link(None, None);
        assert_eq!(m.link_at_caret(), None);
    }

    #[test]
    fn select_all_delete_leaves_one_paragraph() {
        let mut m = model(&["a", "b"]);
        m.insert_block(Block::new(BlockKind::Divider, ""));
        m.select_all();
        m.backspace();
        assert_eq!(m.doc.blocks.len(), 1);
        assert_eq!(texts(&m), [""]);
    }
}
