//! The [`RichEditor`] GPUI entity: input handling on top of [`Model`].

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use cardfile_doc::{Align, Attachment, Block, BlockKind, Document, Image, Mark, TextSize};
use gpui_kit::{
    Action, App, Bounds, ClipboardEntry, ClipboardItem, Context, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PathPromptOptions, Pixels, Point, ScrollHandle, Task, UTF16Selection, Window, actions, point,
    px,
};

use crate::history::{EditKind, History};
use crate::layout::SlotLayout;
use crate::model::Model;
use crate::pos::{
    Pos, Selection, Slot, Slots, next_word, prev_word, utf8_to_utf16, utf16_to_utf8, word_range,
};

/// Key context for editor bindings.
pub const KEY_CONTEXT: &str = "RichEditor";

actions!(
    rich_editor,
    [
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        MoveWordLeft,
        MoveWordRight,
        MoveLineStart,
        MoveLineEnd,
        MoveDocStart,
        MoveDocEnd,
        MovePageUp,
        MovePageDown,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        SelectLineStart,
        SelectLineEnd,
        SelectDocStart,
        SelectDocEnd,
        SelectAll,
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Enter,
        SoftBreak,
        Tab,
        ShiftTab,
        Undo,
        Redo,
        Copy,
        Cut,
        Paste,
        PastePlain,
        Bold,
        Italic,
        Underline,
        Strikethrough,
        InlineCode,
        ClearFormatting,
        Paragraph,
        Heading1,
        Heading2,
        Heading3,
        BulletList,
        NumberedList,
        CheckList,
        Quote,
        CodeBlock,
        AlignLeft,
        AlignCenter,
        AlignRight,
        AlignJustify,
        InsertLink,
        RemoveLink,
        InsertDivider,
        InsertTable,
        InsertImage,
        InsertAttachment,
        TableInsertRowAbove,
        TableInsertRowBelow,
        TableInsertColumnLeft,
        TableInsertColumnRight,
        TableDeleteRow,
        TableDeleteColumn,
        TableDelete,
        TableToggleHeader,
        Indent,
        Outdent,
        ToggleCheck,
        TextSmall,
        TextNormal,
        TextLarge,
        TextHuge,
        ImageSmaller,
        ImageLarger,
        ImageFit,
        OpenLinkAtCaret,
        AddWordToDictionary,
        Find,
        FindReplace,
        FindNext,
        FindPrevious,
        Cancel,
    ]
);

/// Set the text color of the selection (`None` resets it).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = rich_editor, no_json)]
pub struct SetColor(pub Option<String>);

/// Set the highlight color of the selection (`None` removes it).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = rich_editor, no_json)]
pub struct SetHighlight(pub Option<String>);

/// Replace the word at the caret (spelling suggestion).
#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = rich_editor, no_json)]
pub struct ReplaceWord(pub String);

/// Storage for images and attachments. Implemented by the application on top of the DB.
pub trait BlobStore: Send + Sync + 'static {
    /// Store bytes, returning the blob id.
    fn put(&self, data: &[u8], mime: &str, file_name: Option<&str>) -> Option<String>;
    /// Load `(mime, bytes)`.
    fn get(&self, id: &str) -> Option<(String, Vec<u8>)>;
}

/// Spell checking backend.
pub trait SpellChecker: Send + Sync + 'static {
    fn check(&self, word: &str) -> bool;
    fn suggest(&self, word: &str) -> Vec<String>;
    fn add_word(&self, word: &str);
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorEvent {
    /// The document changed (debounce before saving).
    Changed,
    /// The caret or selection moved (refresh toolbars).
    SelectionChanged,
    /// The user tried to edit a read-only document.
    EditBlocked,
    OpenAttachment {
        blob: String,
        file_name: String,
    },
    OpenLink(String),
}

/// Default key bindings. The app may override them with a user keymap.
pub fn default_key_bindings() -> Vec<KeyBinding> {
    let c = Some(KEY_CONTEXT);
    vec![
        KeyBinding::new("left", MoveLeft, c),
        KeyBinding::new("right", MoveRight, c),
        KeyBinding::new("up", MoveUp, c),
        KeyBinding::new("down", MoveDown, c),
        KeyBinding::new("ctrl-left", MoveWordLeft, c),
        KeyBinding::new("ctrl-right", MoveWordRight, c),
        KeyBinding::new("home", MoveLineStart, c),
        KeyBinding::new("end", MoveLineEnd, c),
        KeyBinding::new("ctrl-home", MoveDocStart, c),
        KeyBinding::new("ctrl-end", MoveDocEnd, c),
        KeyBinding::new("pageup", MovePageUp, c),
        KeyBinding::new("pagedown", MovePageDown, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, c),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("ctrl-shift-home", SelectDocStart, c),
        KeyBinding::new("ctrl-shift-end", SelectDocEnd, c),
        KeyBinding::new("ctrl-a", SelectAll, c),
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, c),
        KeyBinding::new("ctrl-delete", DeleteWordRight, c),
        KeyBinding::new("enter", Enter, c),
        KeyBinding::new("shift-enter", SoftBreak, c),
        KeyBinding::new("tab", Tab, c),
        KeyBinding::new("shift-tab", ShiftTab, c),
        KeyBinding::new("ctrl-z", Undo, c),
        KeyBinding::new("ctrl-y", Redo, c),
        KeyBinding::new("ctrl-shift-z", Redo, c),
        KeyBinding::new("ctrl-c", Copy, c),
        KeyBinding::new("ctrl-insert", Copy, c),
        KeyBinding::new("ctrl-x", Cut, c),
        KeyBinding::new("shift-delete", Cut, c),
        KeyBinding::new("ctrl-v", Paste, c),
        KeyBinding::new("shift-insert", Paste, c),
        KeyBinding::new("ctrl-shift-v", PastePlain, c),
        KeyBinding::new("ctrl-b", Bold, c),
        KeyBinding::new("ctrl-i", Italic, c),
        KeyBinding::new("ctrl-u", Underline, c),
        KeyBinding::new("ctrl-shift-x", Strikethrough, c),
        KeyBinding::new("ctrl-`", InlineCode, c),
        KeyBinding::new("ctrl-space", ClearFormatting, c),
        KeyBinding::new("ctrl-alt-0", Paragraph, c),
        KeyBinding::new("ctrl-alt-1", Heading1, c),
        KeyBinding::new("ctrl-alt-2", Heading2, c),
        KeyBinding::new("ctrl-alt-3", Heading3, c),
        KeyBinding::new("ctrl-shift-8", BulletList, c),
        KeyBinding::new("ctrl-shift-7", NumberedList, c),
        KeyBinding::new("ctrl-shift-9", CheckList, c),
        KeyBinding::new("ctrl-shift-.", Quote, c),
        KeyBinding::new("ctrl-alt-c", CodeBlock, c),
        KeyBinding::new("ctrl-l", AlignLeft, c),
        KeyBinding::new("ctrl-e", AlignCenter, c),
        KeyBinding::new("ctrl-r", AlignRight, c),
        KeyBinding::new("ctrl-j", AlignJustify, c),
        KeyBinding::new("ctrl-k", InsertLink, c),
        KeyBinding::new("ctrl-]", Indent, c),
        KeyBinding::new("ctrl-[", Outdent, c),
        KeyBinding::new("ctrl-enter", ToggleCheck, c),
        KeyBinding::new("alt-enter", OpenLinkAtCaret, c),
        KeyBinding::new("ctrl-f", Find, c),
        KeyBinding::new("ctrl-h", FindReplace, c),
        KeyBinding::new("f3", FindNext, c),
        KeyBinding::new("shift-f3", FindPrevious, c),
        KeyBinding::new("escape", Cancel, c),
    ]
}

/// Bind the default keys. Call once at startup (after the app's own bindings).
pub fn init(cx: &mut App) {
    cx.bind_keys(default_key_bindings());
}

pub struct RichEditor {
    pub(crate) model: Model,
    history: History,
    pub(crate) focus_handle: FocusHandle,
    read_only: bool,
    pub(crate) layouts: HashMap<Slot, SlotLayout>,
    /// IME composition range (bytes) in the caret slot.
    marked: Option<Range<usize>>,
    selecting: bool,
    /// Preferred x when moving vertically.
    goal_x: Option<Pixels>,
    pub(crate) scroll: ScrollHandle,
    autoscroll: bool,
    blink_on: bool,
    blink_epoch: usize,
    _blink_task: Option<Task<()>>,
    pub(crate) blobs: Option<Arc<dyn BlobStore>>,
    pub(crate) image_cache: HashMap<String, Arc<gpui_kit::Image>>,
    spell: Option<Arc<dyn SpellChecker>>,
    spell_cache: HashMap<String, Vec<Range<usize>>>,
    pub(crate) base_font_size: Pixels,
    pub(crate) placeholder: Option<String>,
    revision: u64,
    pub(crate) find: Option<crate::find::FindBar>,
    pub(crate) slash: Option<crate::slash::SlashMenu>,
}

impl EventEmitter<EditorEvent> for RichEditor {}

impl Focusable for RichEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl RichEditor {
    pub fn new(doc: Document, cx: &mut Context<Self>) -> Self {
        Self {
            model: Model::new(doc),
            history: History::default(),
            focus_handle: cx.focus_handle(),
            read_only: false,
            layouts: HashMap::new(),
            marked: None,
            selecting: false,
            goal_x: None,
            scroll: ScrollHandle::new(),
            autoscroll: false,
            blink_on: true,
            blink_epoch: 0,
            _blink_task: None,
            blobs: None,
            image_cache: HashMap::new(),
            spell: None,
            spell_cache: HashMap::new(),
            base_font_size: px(15.),
            placeholder: None,
            revision: 0,
            find: None,
            slash: None,
        }
    }

    // ---------------------------------------------------------------------
    // Public API
    // ---------------------------------------------------------------------

    pub fn document(&self) -> &Document {
        &self.model.doc
    }

    /// Replace the document (e.g. when another card is opened). Clears undo history.
    pub fn set_document(&mut self, doc: Document, cx: &mut Context<Self>) {
        self.model = Model {
            input_rules: self.model.input_rules,
            ..Model::new(doc)
        };
        self.history.clear();
        self.layouts.clear();
        self.marked = None;
        self.slash = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.refresh_matches(false, cx);
        cx.notify();
    }

    /// Bump on every content change; the host compares it to know what was saved.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        cx.notify();
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn set_blob_store(&mut self, store: Arc<dyn BlobStore>) {
        self.blobs = Some(store);
        self.image_cache.clear();
    }

    pub fn set_spell_checker(
        &mut self,
        spell: Option<Arc<dyn SpellChecker>>,
        cx: &mut Context<Self>,
    ) {
        self.spell = spell;
        self.spell_cache.clear();
        cx.notify();
    }

    pub fn set_input_rules(&mut self, on: bool) {
        self.model.input_rules = on;
    }

    pub fn set_font_size(&mut self, size: Pixels, cx: &mut Context<Self>) {
        self.base_font_size = size;
        cx.notify();
    }

    pub fn set_placeholder(&mut self, text: Option<String>) {
        self.placeholder = text;
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn has_mark(&self, mark: Mark) -> bool {
        self.model.has_mark(mark)
    }

    pub fn current_block_kind(&self) -> Option<BlockKind> {
        self.model.current_block_kind()
    }

    pub fn current_align(&self) -> Align {
        self.model
            .doc
            .blocks
            .get(self.model.caret().slot.block)
            .map_or(Align::Left, |b| b.align)
    }

    /// The selected text as plain text.
    pub fn selected_text(&self) -> String {
        self.model.selected_text()
    }

    pub fn in_table(&self) -> bool {
        self.model.in_table()
    }

    pub fn selected_image(&self) -> bool {
        let slot = self.model.caret().slot;
        matches!(
            self.model.doc.blocks.get(slot.block).map(|b| &b.kind),
            Some(BlockKind::Image(_))
        )
    }

    // ---------------------------------------------------------------------
    // Paint-time helpers used by the elements
    // ---------------------------------------------------------------------

    pub(crate) fn store_layout(&mut self, slot: Slot, layout: SlotLayout) -> bool {
        let mut refresh = false;
        if self.autoscroll && slot == self.model.caret().slot {
            self.autoscroll = false;
            let caret = layout.position_for(self.model.caret().offset);
            let viewport = self.scroll.bounds();
            if viewport.size.height > px(0.) {
                let top = caret.y;
                let bottom = caret.y + layout.line_height;
                let mut offset = self.scroll.offset();
                let margin = layout.line_height;
                if top < viewport.top() + margin {
                    offset.y += viewport.top() + margin - top;
                    refresh = true;
                } else if bottom > viewport.bottom() - margin {
                    offset.y -= bottom - (viewport.bottom() - margin);
                    refresh = true;
                }
                if refresh {
                    offset.y = offset.y.min(px(0.));
                    self.scroll.set_offset(offset);
                }
            }
        }
        self.layouts.insert(slot, layout);
        refresh
    }

    pub(crate) fn selected_range_in(&self, slot: Slot, len: usize) -> Option<Range<usize>> {
        let sel = self.model.sel;
        if sel.is_empty() {
            return None;
        }
        let (s, e) = (sel.start(), sel.end());
        if slot < s.slot || slot > e.slot {
            return None;
        }
        let start = if slot == s.slot { s.offset } else { 0 };
        let end = if slot == e.slot { e.offset } else { len };
        // Mark a fully covered empty slot (or line break) as selected.
        let end = if slot != e.slot {
            end.max(start) + usize::from(len == 0)
        } else {
            end
        };
        Some(start..end)
    }

    pub(crate) fn is_atom_selected(&self, block: usize) -> bool {
        let sel = self.model.sel;
        let slot = Slot::block(block);
        if sel.is_empty() {
            return sel.head.slot == slot;
        }
        sel.start().slot <= slot && slot <= sel.end().slot
    }

    pub(crate) fn marked_range_in(&self, slot: Slot) -> Option<Range<usize>> {
        (slot == self.model.caret().slot)
            .then(|| self.marked.clone())
            .flatten()
    }

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
        let mut ranges = self.spell_cache.get(text).cloned().unwrap_or_default();
        // Don't flag the word being typed.
        let caret = self.model.caret();
        if caret.slot == slot {
            ranges.retain(|r| !(r.start <= caret.offset && caret.offset <= r.end));
        }
        ranges
    }

    /// Spell-check visible slots whose text changed (called from render).
    pub(crate) fn refresh_spelling(&mut self) {
        let Some(spell) = self.spell.clone() else {
            return;
        };
        let mut texts: Vec<String> = Vec::new();
        for (bi, block) in self.model.doc.blocks.iter().enumerate() {
            if matches!(block.kind, BlockKind::Code { .. }) {
                continue;
            }
            let _ = bi;
            match &block.kind {
                BlockKind::Table(t) => {
                    texts.extend(t.rows.iter().flatten().map(|c| c.text.clone()))
                }
                k if k.is_textual() => texts.push(block.text.text.clone()),
                _ => {}
            }
        }
        for text in texts {
            if self.spell_cache.contains_key(&text) {
                continue;
            }
            let ranges = crate::util::misspelled_ranges(&text, |w| spell.check(w));
            self.spell_cache.insert(text, ranges);
        }
        if self.spell_cache.len() > 2000 {
            self.spell_cache.clear();
        }
    }

    pub(crate) fn caret_visible(&self) -> bool {
        self.blink_on
    }

    fn reset_blink(&mut self, cx: &mut Context<Self>) {
        self.blink_on = true;
        self.blink_epoch += 1;
        let epoch = self.blink_epoch;
        self._blink_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let alive = this
                    .update(cx, |e, cx| {
                        if e.blink_epoch != epoch {
                            return false;
                        }
                        e.blink_on = !e.blink_on;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        }));
    }

    // ---------------------------------------------------------------------
    // Edit plumbing
    // ---------------------------------------------------------------------

    fn edit(&mut self, kind: EditKind, cx: &mut Context<Self>, f: impl FnOnce(&mut Model)) {
        if self.read_only {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        let before = self.model.doc.clone();
        let sel = self.model.sel;
        f(&mut self.model);
        if self.model.doc != before {
            self.history.record(&before, sel, kind);
            self.revision += 1;
            cx.emit(EditorEvent::Changed);
            if self.find.is_some() {
                self.refresh_matches(false, cx);
            }
        }
        self.validate_slash();
        self.after_move(cx);
    }

    pub(crate) fn edit_with(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Model)) {
        self.edit(EditKind::Other, cx, f);
    }

    pub(crate) fn request_autoscroll(&mut self) {
        self.autoscroll = true;
    }

    pub(crate) fn pick_files_public(&mut self, images: bool, cx: &mut Context<Self>) {
        self.pick_files(images, cx);
    }

    fn after_move(&mut self, cx: &mut Context<Self>) {
        self.autoscroll = true;
        self.reset_blink(cx);
        cx.emit(EditorEvent::SelectionChanged);
        cx.notify();
    }

    fn move_caret(&mut self, pos: Pos, extend: bool, cx: &mut Context<Self>) {
        let pos = self.model.doc.clamp_pos(pos);
        if extend {
            self.model.select_to(pos);
        } else {
            self.model.set_caret(pos);
        }
        self.history.seal();
        self.marked = None;
        self.validate_slash();
        self.after_move(cx);
    }

    // ---------------------------------------------------------------------
    // Horizontal movement
    // ---------------------------------------------------------------------

    fn pos_left(&self, pos: Pos, word: bool) -> Pos {
        let doc = &self.model.doc;
        if pos.offset > 0 {
            let text = doc.slot_text(pos.slot).map_or("", |t| t.text.as_str());
            let off = if word {
                prev_word(text, pos.offset)
            } else {
                crate::pos::prev_grapheme(text, pos.offset)
            };
            return Pos::new(pos.slot, off);
        }
        match doc.prev_slot(pos.slot) {
            Some(prev) => Pos::new(prev, doc.slot_len(prev)),
            None => pos,
        }
    }

    fn pos_right(&self, pos: Pos, word: bool) -> Pos {
        let doc = &self.model.doc;
        let len = doc.slot_len(pos.slot);
        if pos.offset < len {
            let text = doc.slot_text(pos.slot).map_or("", |t| t.text.as_str());
            let off = if word {
                next_word(text, pos.offset)
            } else {
                crate::pos::next_grapheme(text, pos.offset)
            };
            return Pos::new(pos.slot, off);
        }
        match doc.next_slot(pos.slot) {
            Some(next) => Pos::new(next, 0),
            None => pos,
        }
    }

    fn horizontal(&mut self, left: bool, word: bool, extend: bool, cx: &mut Context<Self>) {
        self.goal_x = None;
        let sel = self.model.sel;
        let target = if !extend && !sel.is_empty() && !word {
            if left { sel.start() } else { sel.end() }
        } else if left {
            self.pos_left(sel.head, word)
        } else {
            self.pos_right(sel.head, word)
        };
        self.move_caret(target, extend, cx);
    }

    // ---------------------------------------------------------------------
    // Vertical movement (layout based)
    // ---------------------------------------------------------------------

    fn caret_point(&self) -> Option<Point<Pixels>> {
        let pos = self.model.caret();
        self.layouts
            .get(&pos.slot)
            .map(|l| l.position_for(pos.offset))
    }

    /// The text position closest to a window-space point.
    pub(crate) fn pos_for_point(&self, p: Point<Pixels>) -> Option<Pos> {
        let mut best: Option<(Slot, f32)> = None;
        for (slot, layout) in &self.layouts {
            if slot.block >= self.model.doc.blocks.len() {
                continue;
            }
            let b = layout.bounds;
            let height = layout.height();
            let dy = if p.y < b.top() {
                (b.top() - p.y).as_f32()
            } else if p.y > b.top() + height {
                (p.y - (b.top() + height)).as_f32()
            } else {
                0.
            };
            let dx = if p.x < b.left() {
                (b.left() - p.x).as_f32()
            } else if p.x > b.right() {
                (p.x - b.right()).as_f32()
            } else {
                0.
            };
            // Vertical distance dominates; x disambiguates table cells on one row.
            let score = dy * 1000. + dx;
            if best.is_none_or(|(_, s)| score < s) {
                best = Some((*slot, score));
            }
        }
        let (slot, _) = best?;
        let layout = &self.layouts[&slot];
        Some(Pos::new(slot, layout.offset_for(p)))
    }

    fn vertical(&mut self, down: bool, pages: bool, extend: bool, cx: &mut Context<Self>) {
        let Some(caret) = self.caret_point() else {
            return;
        };
        let lh = self
            .layouts
            .get(&self.model.caret().slot)
            .map_or(px(20.), |l| l.line_height);
        let x = *self.goal_x.get_or_insert(caret.x);
        let step = if pages {
            self.scroll.bounds().size.height.max(lh * 3.) - lh
        } else {
            lh
        };
        let y = if down {
            caret.y + step + lh / 2.
        } else {
            caret.y - step + lh / 2.
        };
        let target = self.pos_for_point(point(x, y));
        let target = match target {
            Some(t) if t != self.model.caret() => t,
            // Already on the first/last line: jump to the document edge.
            _ if down => self.model.doc.doc_end(),
            _ => self.model.doc.doc_start(),
        };
        let goal = self.goal_x;
        self.move_caret(target, extend, cx);
        self.goal_x = goal;
    }

    fn line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        self.goal_x = None;
        let pos = self.model.caret();
        let (s, e) = self
            .layouts
            .get(&pos.slot)
            .map_or((0, self.model.doc.slot_len(pos.slot)), |l| {
                l.row_bounds(pos.offset)
            });
        // A soft wrap row ends just before the next row's first char; trim a trailing space.
        let text = self
            .model
            .doc
            .slot_text(pos.slot)
            .map_or("", |t| t.text.as_str());
        let e = if e < text.len() && text[..e].ends_with(' ') {
            e - 1
        } else {
            e
        };
        self.move_caret(Pos::new(pos.slot, if end { e } else { s }), extend, cx);
    }

    // ---------------------------------------------------------------------
    // Action handlers
    // ---------------------------------------------------------------------

    fn move_left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, false, false, cx);
    }
    fn move_right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, false, false, cx);
    }
    fn move_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.slash_key(crate::slash::SlashKey::Up, cx) {
            return;
        }
        self.vertical(false, false, false, cx);
    }
    fn move_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.slash_key(crate::slash::SlashKey::Down, cx) {
            return;
        }
        self.vertical(true, false, false, cx);
    }
    fn move_word_left(&mut self, _: &MoveWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, true, false, cx);
    }
    fn move_word_right(&mut self, _: &MoveWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, true, false, cx);
    }
    fn move_line_start(&mut self, _: &MoveLineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(false, false, cx);
    }
    fn move_line_end(&mut self, _: &MoveLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(true, false, cx);
    }
    fn move_doc_start(&mut self, _: &MoveDocStart, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.model.doc.doc_start();
        self.move_caret(p, false, cx);
    }
    fn move_doc_end(&mut self, _: &MoveDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.model.doc.doc_end();
        self.move_caret(p, false, cx);
    }
    fn move_page_up(&mut self, _: &MovePageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, true, false, cx);
    }
    fn move_page_down(&mut self, _: &MovePageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, true, false, cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, false, true, cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, false, true, cx);
    }
    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(false, false, true, cx);
    }
    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical(true, false, true, cx);
    }
    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, true, true, cx);
    }
    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, true, true, cx);
    }
    fn select_line_start(&mut self, _: &SelectLineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(false, true, cx);
    }
    fn select_line_end(&mut self, _: &SelectLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(true, true, cx);
    }
    fn select_doc_start(&mut self, _: &SelectDocStart, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.model.doc.doc_start();
        self.move_caret(p, true, cx);
    }
    fn select_doc_end(&mut self, _: &SelectDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let p = self.model.doc.doc_end();
        self.move_caret(p, true, cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.model.select_all();
        self.after_move(cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Deleting, cx, Model::backspace);
    }
    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Deleting, cx, Model::delete_forward);
    }
    fn delete_word_left(&mut self, _: &DeleteWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        if self.model.sel.is_empty() {
            let to = self.pos_left(self.model.caret(), true);
            self.model.select_to(to);
        }
        self.edit(EditKind::Other, cx, Model::backspace);
    }
    fn delete_word_right(&mut self, _: &DeleteWordRight, _: &mut Window, cx: &mut Context<Self>) {
        if self.model.sel.is_empty() {
            let to = self.pos_right(self.model.caret(), true);
            self.model.select_to(to);
        }
        self.edit(EditKind::Other, cx, Model::delete_forward);
    }
    fn enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        if self.slash_key(crate::slash::SlashKey::Enter, cx) {
            return;
        }
        self.marked = None;
        self.edit(EditKind::Other, cx, Model::enter);
    }
    fn soft_break(&mut self, _: &SoftBreak, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, Model::soft_break);
    }
    fn tab(&mut self, _: &Tab, _: &mut Window, cx: &mut Context<Self>) {
        if self.slash_key(crate::slash::SlashKey::Enter, cx) {
            return;
        }
        if self.model.in_table() {
            self.model.table_next_cell(false);
            // Adding a row at the end of a table is an edit.
            self.revision += 1;
            cx.emit(EditorEvent::Changed);
            self.after_move(cx);
            return;
        }
        let kind = self.model.current_block_kind();
        match kind {
            Some(k) if k.is_list() => self.edit(EditKind::Other, cx, |m| m.indent(1)),
            _ => self.edit(EditKind::Typing, cx, |m| m.insert_text("    ")),
        }
    }
    fn shift_tab(&mut self, _: &ShiftTab, _: &mut Window, cx: &mut Context<Self>) {
        if self.model.in_table() {
            self.model.table_next_cell(true);
            self.after_move(cx);
            return;
        }
        self.edit(EditKind::Other, cx, |m| m.indent(-1));
    }
    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some((doc, sel)) = self.history.undo(&self.model.doc, self.model.sel) {
            self.model.doc = doc;
            self.model.set_selection(sel);
            self.revision += 1;
            cx.emit(EditorEvent::Changed);
            self.after_move(cx);
        }
    }
    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some((doc, sel)) = self.history.redo(&self.model.doc, self.model.sel) {
            self.model.doc = doc;
            self.model.set_selection(sel);
            self.revision += 1;
            cx.emit(EditorEvent::Changed);
            self.after_move(cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(frag) = self.model.selected_fragment() {
            let text = cardfile_doc::to_plain_text(&frag);
            cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(text, frag));
        }
    }
    fn cut(&mut self, a: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            self.copy(&Copy, window, cx);
            return;
        }
        let _ = a;
        self.copy(&Copy, window, cx);
        self.edit(EditKind::Other, cx, |m| {
            m.delete_selection();
        });
    }
    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        self.paste_impl(false, cx);
    }
    fn paste_plain(&mut self, _: &PastePlain, _: &mut Window, cx: &mut Context<Self>) {
        self.paste_impl(true, cx);
    }

    fn paste_impl(&mut self, plain: bool, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        if let Some(entry) = item.entries().first() {
            match entry {
                ClipboardEntry::String(s) => {
                    if !plain && let Some(frag) = s.metadata_json::<Document>() {
                        self.edit(EditKind::Other, cx, |m| m.insert_fragment(&frag));
                        return;
                    }
                    let text = s.text().to_owned();
                    if !plain && cardfile_doc::looks_like_markdown(&text) && !self.model.in_table()
                    {
                        let frag = cardfile_doc::from_markdown(&text);
                        self.edit(EditKind::Other, cx, |m| m.insert_fragment(&frag));
                    } else {
                        self.edit(EditKind::Other, cx, |m| m.insert_text(&text));
                    }
                }
                ClipboardEntry::Image(img) => {
                    let mime = img.format.mime_type().to_owned();
                    let bytes = img.bytes.clone();
                    self.insert_image_bytes(&bytes, &mime, None, cx);
                }
                ClipboardEntry::ExternalPaths(paths) => {
                    let paths: Vec<_> = paths.paths().to_vec();
                    self.insert_files(paths, cx);
                }
            }
        }
    }

    /// Insert dropped or pasted files: images inline, anything else as attachments.
    pub fn insert_files(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        if self.read_only {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        for path in paths {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
            let mime = crate::util::mime_for_path(&path);
            if mime.starts_with("image/") {
                self.insert_image_bytes(&bytes, &mime, name.as_deref(), cx);
            } else {
                self.insert_attachment_bytes(&bytes, &mime, name.as_deref().unwrap_or("file"), cx);
            }
        }
    }

    pub fn insert_image_bytes(
        &mut self,
        bytes: &[u8],
        mime: &str,
        name: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.blobs.clone() else {
            return;
        };
        let Some(id) = store.put(bytes, mime, name) else {
            return;
        };
        let image = Image {
            blob: id,
            mime: mime.to_owned(),
            width: None,
            alt: name.unwrap_or_default().to_owned(),
        };
        self.edit(EditKind::Other, cx, |m| {
            m.insert_block(Block::new(BlockKind::Image(image), ""))
        });
    }

    pub fn insert_attachment_bytes(
        &mut self,
        bytes: &[u8],
        mime: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.blobs.clone() else {
            return;
        };
        let Some(id) = store.put(bytes, mime, Some(name)) else {
            return;
        };
        let att = Attachment {
            blob: id,
            file_name: name.to_owned(),
            mime: mime.to_owned(),
            size: bytes.len() as u64,
        };
        self.edit(EditKind::Other, cx, |m| {
            m.insert_block(Block::new(BlockKind::Attachment(att), ""))
        });
    }

    fn mark(&mut self, mark: Mark, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.toggle_mark(mark));
        // Toggling with an empty selection only changes the pending style.
        cx.emit(EditorEvent::SelectionChanged);
        cx.notify();
    }
    fn bold(&mut self, _: &Bold, _: &mut Window, cx: &mut Context<Self>) {
        self.mark(Mark::Bold, cx);
    }
    fn italic(&mut self, _: &Italic, _: &mut Window, cx: &mut Context<Self>) {
        self.mark(Mark::Italic, cx);
    }
    fn underline(&mut self, _: &Underline, _: &mut Window, cx: &mut Context<Self>) {
        self.mark(Mark::Underline, cx);
    }
    fn strikethrough(&mut self, _: &Strikethrough, _: &mut Window, cx: &mut Context<Self>) {
        self.mark(Mark::Strike, cx);
    }
    fn inline_code(&mut self, _: &InlineCode, _: &mut Window, cx: &mut Context<Self>) {
        self.mark(Mark::Code, cx);
    }
    fn clear_formatting(&mut self, _: &ClearFormatting, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, Model::clear_formatting);
    }
    fn set_color(&mut self, a: &SetColor, _: &mut Window, cx: &mut Context<Self>) {
        let c = a.0.clone();
        self.edit(EditKind::Other, cx, |m| m.set_color(c));
    }
    fn set_highlight(&mut self, a: &SetHighlight, _: &mut Window, cx: &mut Context<Self>) {
        let c = a.0.clone();
        self.edit(EditKind::Other, cx, |m| m.set_highlight(c));
    }

    fn block_kind(&mut self, kind: BlockKind, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.set_block_kind(kind));
    }
    fn paragraph(&mut self, _: &Paragraph, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| {
            for b in m.selected_blocks() {
                if let Some(block) = m.doc.blocks.get_mut(b)
                    && block.kind.is_textual()
                {
                    block.kind = BlockKind::Paragraph;
                }
            }
        });
    }
    fn heading1(&mut self, _: &Heading1, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::Heading(1), cx);
    }
    fn heading2(&mut self, _: &Heading2, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::Heading(2), cx);
    }
    fn heading3(&mut self, _: &Heading3, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::Heading(3), cx);
    }
    fn bullet_list(&mut self, _: &BulletList, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::BulletList, cx);
    }
    fn numbered_list(&mut self, _: &NumberedList, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::NumberedList, cx);
    }
    fn check_list(&mut self, _: &CheckList, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::CheckList { checked: false }, cx);
    }
    fn quote(&mut self, _: &Quote, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(BlockKind::Quote, cx);
    }
    fn code_block(&mut self, _: &CodeBlock, _: &mut Window, cx: &mut Context<Self>) {
        self.block_kind(
            BlockKind::Code {
                language: String::new(),
            },
            cx,
        );
    }
    fn align(&mut self, a: Align, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.set_align(a));
    }
    fn align_left(&mut self, _: &AlignLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Align::Left, cx);
    }
    fn align_center(&mut self, _: &AlignCenter, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Align::Center, cx);
    }
    fn align_right(&mut self, _: &AlignRight, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Align::Right, cx);
    }
    fn align_justify(&mut self, _: &AlignJustify, _: &mut Window, cx: &mut Context<Self>) {
        self.align(Align::Justify, cx);
    }
    fn size(&mut self, s: TextSize, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.set_size(s));
    }
    fn text_small(&mut self, _: &TextSmall, _: &mut Window, cx: &mut Context<Self>) {
        self.size(TextSize::Small, cx);
    }
    fn text_normal(&mut self, _: &TextNormal, _: &mut Window, cx: &mut Context<Self>) {
        self.size(TextSize::Normal, cx);
    }
    fn text_large(&mut self, _: &TextLarge, _: &mut Window, cx: &mut Context<Self>) {
        self.size(TextSize::Large, cx);
    }
    fn text_huge(&mut self, _: &TextHuge, _: &mut Window, cx: &mut Context<Self>) {
        self.size(TextSize::Huge, cx);
    }
    fn indent(&mut self, _: &Indent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.indent(1));
    }
    fn outdent(&mut self, _: &Outdent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.indent(-1));
    }
    fn toggle_check_action(&mut self, _: &ToggleCheck, _: &mut Window, cx: &mut Context<Self>) {
        let block = self.model.caret().slot.block;
        self.toggle_check(block, cx);
    }
    pub(crate) fn toggle_check(&mut self, block: usize, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.toggle_check(block));
    }

    fn insert_divider(&mut self, _: &InsertDivider, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| {
            m.insert_block(Block::new(BlockKind::Divider, ""))
        });
    }
    fn insert_table(&mut self, _: &InsertTable, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.insert_table(3, 3));
    }
    fn table_op(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Model)) {
        self.edit(EditKind::Other, cx, f);
    }
    fn table_row_above(&mut self, _: &TableInsertRowAbove, _: &mut Window, cx: &mut Context<Self>) {
        self.table_op(cx, |m| m.table_insert_row(false));
    }
    fn table_row_below(&mut self, _: &TableInsertRowBelow, _: &mut Window, cx: &mut Context<Self>) {
        self.table_op(cx, |m| m.table_insert_row(true));
    }
    fn table_col_left(
        &mut self,
        _: &TableInsertColumnLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.table_op(cx, |m| m.table_insert_col(false));
    }
    fn table_col_right(
        &mut self,
        _: &TableInsertColumnRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.table_op(cx, |m| m.table_insert_col(true));
    }
    fn table_delete_row(&mut self, _: &TableDeleteRow, _: &mut Window, cx: &mut Context<Self>) {
        self.table_op(cx, Model::table_delete_row);
    }
    fn table_delete_col(&mut self, _: &TableDeleteColumn, _: &mut Window, cx: &mut Context<Self>) {
        self.table_op(cx, Model::table_delete_col);
    }
    fn table_delete(&mut self, _: &TableDelete, _: &mut Window, cx: &mut Context<Self>) {
        self.table_op(cx, Model::table_delete);
    }
    fn table_toggle_header(
        &mut self,
        _: &TableToggleHeader,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.table_op(cx, Model::table_toggle_header);
    }

    fn resize_image(&mut self, f: impl Fn(Option<f32>) -> Option<f32>, cx: &mut Context<Self>) {
        let block = self.model.caret().slot.block;
        self.edit(EditKind::Other, cx, |m| {
            if let Some(Block {
                kind: BlockKind::Image(img),
                ..
            }) = m.doc.blocks.get_mut(block)
            {
                img.width = f(img.width);
            }
        });
    }
    fn image_smaller(&mut self, _: &ImageSmaller, _: &mut Window, cx: &mut Context<Self>) {
        self.resize_image(|w| Some((w.unwrap_or(640.) * 0.8).max(48.)), cx);
    }
    fn image_larger(&mut self, _: &ImageLarger, _: &mut Window, cx: &mut Context<Self>) {
        self.resize_image(|w| Some((w.unwrap_or(400.) * 1.25).min(4000.)), cx);
    }
    fn image_fit(&mut self, _: &ImageFit, _: &mut Window, cx: &mut Context<Self>) {
        self.resize_image(|_| None, cx);
    }

    fn insert_image(&mut self, _: &InsertImage, _: &mut Window, cx: &mut Context<Self>) {
        self.pick_files(true, cx);
    }
    fn insert_attachment(&mut self, _: &InsertAttachment, _: &mut Window, cx: &mut Context<Self>) {
        self.pick_files(false, cx);
    }

    fn pick_files(&mut self, images: bool, cx: &mut Context<Self>) {
        if self.read_only {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let paths: Vec<_> = if images {
                paths
                    .into_iter()
                    .filter(|p| crate::util::mime_for_path(p).starts_with("image/"))
                    .collect()
            } else {
                paths
            };
            let _ = this.update(cx, |e, cx| e.insert_files(paths, cx));
        })
        .detach();
    }

    fn insert_link(&mut self, _: &InsertLink, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        crate::dialogs::open_link_dialog(cx.entity(), window, cx);
    }
    fn remove_link(&mut self, _: &RemoveLink, _: &mut Window, cx: &mut Context<Self>) {
        self.edit(EditKind::Other, cx, |m| m.set_link(None, None));
    }
    pub(crate) fn apply_link(&mut self, url: String, text: Option<String>, cx: &mut Context<Self>) {
        let url = normalize_url(&url);
        let display = text
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| url_preview(&url));
        self.edit(EditKind::Other, cx, |m| {
            if url.is_empty() {
                m.set_link(None, None);
            } else {
                m.set_link(Some(url.clone()), Some(&display));
            }
        });
    }
    pub(crate) fn link_at_caret(&self) -> Option<String> {
        self.model.link_at_caret()
    }
    pub(crate) fn selection_is_empty(&self) -> bool {
        self.model.sel.is_empty()
    }
    fn open_link_at_caret(&mut self, _: &OpenLinkAtCaret, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(url) = self.model.link_at_caret() {
            cx.emit(EditorEvent::OpenLink(url));
        }
    }

    fn replace_word(&mut self, a: &ReplaceWord, _: &mut Window, cx: &mut Context<Self>) {
        let pos = self.model.caret();
        let Some(text) = self.model.doc.slot_text(pos.slot) else {
            return;
        };
        let range = word_range(&text.text, pos.offset);
        let word = a.0.clone();
        self.model.set_selection(Selection {
            anchor: Pos::new(pos.slot, range.start),
            head: Pos::new(pos.slot, range.end),
        });
        self.edit(EditKind::Other, cx, |m| m.insert_text(&word));
    }
    fn add_word_to_dictionary(
        &mut self,
        _: &AddWordToDictionary,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let (Some(spell), Some(word)) = (self.spell.clone(), self.word_at_caret()) {
            spell.add_word(&word);
            self.spell_cache.clear();
            cx.notify();
        }
    }

    pub(crate) fn word_at_caret(&self) -> Option<String> {
        let pos = self.model.caret();
        let text = self.model.doc.slot_text(pos.slot)?;
        let r = word_range(&text.text, pos.offset);
        let w = text.text.get(r)?.trim();
        (!w.is_empty() && w.chars().any(char::is_alphabetic)).then(|| w.to_owned())
    }

    /// Suggestions for the misspelled word at the caret, if it is misspelled.
    pub(crate) fn spelling_suggestions(&self) -> Option<(String, Vec<String>)> {
        let spell = self.spell.as_ref()?;
        let word = self.word_at_caret()?;
        if spell.check(&word) {
            return None;
        }
        Some((
            word.clone(),
            spell.suggest(&word).into_iter().take(6).collect(),
        ))
    }

    // ---------------------------------------------------------------------
    // Mouse
    // ---------------------------------------------------------------------

    pub(crate) fn on_mouse_down(
        &mut self,
        ev: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        let Some(pos) = self.pos_for_point(ev.position) else {
            return;
        };
        self.goal_x = None;
        self.marked = None;
        if ev.button == MouseButton::Right {
            // Keep a selection the user right-clicked into; otherwise move the caret.
            let sel = self.model.sel;
            if sel.is_empty() || pos < sel.start() || pos > sel.end() {
                self.move_caret(pos, false, cx);
            }
            return;
        }
        if ev.modifiers.control
            && let Some(url) = self.link_at(pos)
        {
            cx.emit(EditorEvent::OpenLink(url));
            return;
        }
        match ev.click_count {
            2 => {
                let text = self
                    .model
                    .doc
                    .slot_text(pos.slot)
                    .map_or("", |t| t.text.as_str());
                let r = word_range(text, pos.offset);
                self.model.set_selection(Selection {
                    anchor: Pos::new(pos.slot, r.start),
                    head: Pos::new(pos.slot, r.end),
                });
                self.after_move(cx);
            }
            n if n >= 3 => {
                let len = self.model.doc.slot_len(pos.slot);
                self.model.set_selection(Selection {
                    anchor: Pos::new(pos.slot, 0),
                    head: Pos::new(pos.slot, len),
                });
                self.after_move(cx);
            }
            _ => {
                self.selecting = true;
                self.move_caret(pos, ev.modifiers.shift, cx);
            }
        }
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        ev: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.selecting || ev.pressed_button != Some(MouseButton::Left) {
            return;
        }
        if let Some(pos) = self.pos_for_point(ev.position)
            && pos != self.model.sel.head
        {
            self.model.select_to(pos);
            self.after_move(cx);
        }
    }

    pub(crate) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn link_at(&self, pos: Pos) -> Option<String> {
        let t = self.model.doc.slot_text(pos.slot)?;
        t.spans()
            .find(|(r, s)| s.link.is_some() && r.start <= pos.offset && pos.offset < r.end)
            .and_then(|(_, s)| s.link.clone())
    }

    /// Clicking an atom (divider, image, attachment) selects it.
    pub(crate) fn select_atom(
        &mut self,
        block: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        self.move_caret(Pos::new(Slot::block(block), 0), false, cx);
    }

    pub(crate) fn open_attachment(&mut self, block: usize, cx: &mut Context<Self>) {
        if let Some(Block {
            kind: BlockKind::Attachment(a),
            ..
        }) = self.model.doc.blocks.get(block)
        {
            cx.emit(EditorEvent::OpenAttachment {
                blob: a.blob.clone(),
                file_name: a.file_name.clone(),
            });
        }
    }

    pub(crate) fn image_for(&mut self, img: &Image) -> Option<Arc<gpui_kit::Image>> {
        if let Some(i) = self.image_cache.get(&img.blob) {
            return Some(i.clone());
        }
        let (mime, bytes) = self.blobs.as_ref()?.get(&img.blob)?;
        let format = gpui_kit::ImageFormat::from_mime_type(&mime)
            .or_else(|| gpui_kit::ImageFormat::from_mime_type(&img.mime))?;
        let image = Arc::new(gpui_kit::Image::from_bytes(format, bytes));
        self.image_cache.insert(img.blob.clone(), image.clone());
        Some(image)
    }

    fn find_action(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        self.open_find(false, window, cx);
    }
    fn find_replace(&mut self, _: &FindReplace, window: &mut Window, cx: &mut Context<Self>) {
        let replace = !self.read_only;
        self.open_find(replace, window, cx);
    }
    fn find_next(&mut self, _: &FindNext, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            self.open_find(false, window, cx);
        }
        self.find_step(true, window, cx);
    }
    fn find_previous(&mut self, _: &FindPrevious, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            self.open_find(false, window, cx);
        }
        self.find_step(false, window, cx);
    }
    fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.slash_key(crate::slash::SlashKey::Escape, cx) {
            return;
        }
        if self.find.is_some() {
            self.close_find(window, cx);
        } else if !self.model.sel.is_empty() {
            let head = self.model.sel.head;
            self.move_caret(head, false, cx);
        }
    }

    /// Register action handlers on the editor root element.
    pub(crate) fn bind_actions(
        div: gpui_kit::Stateful<gpui_kit::Div>,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<gpui_kit::Div> {
        use gpui_kit::InteractiveElement as _;
        div.on_action(cx.listener(Self::move_left))
            .on_action(cx.listener(Self::move_right))
            .on_action(cx.listener(Self::move_up))
            .on_action(cx.listener(Self::move_down))
            .on_action(cx.listener(Self::move_word_left))
            .on_action(cx.listener(Self::move_word_right))
            .on_action(cx.listener(Self::move_line_start))
            .on_action(cx.listener(Self::move_line_end))
            .on_action(cx.listener(Self::move_doc_start))
            .on_action(cx.listener(Self::move_doc_end))
            .on_action(cx.listener(Self::move_page_up))
            .on_action(cx.listener(Self::move_page_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_line_start))
            .on_action(cx.listener(Self::select_line_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::soft_break))
            .on_action(cx.listener(Self::tab))
            .on_action(cx.listener(Self::shift_tab))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::paste_plain))
            .on_action(cx.listener(Self::bold))
            .on_action(cx.listener(Self::italic))
            .on_action(cx.listener(Self::underline))
            .on_action(cx.listener(Self::strikethrough))
            .on_action(cx.listener(Self::inline_code))
            .on_action(cx.listener(Self::clear_formatting))
            .on_action(cx.listener(Self::set_color))
            .on_action(cx.listener(Self::set_highlight))
            .on_action(cx.listener(Self::paragraph))
            .on_action(cx.listener(Self::heading1))
            .on_action(cx.listener(Self::heading2))
            .on_action(cx.listener(Self::heading3))
            .on_action(cx.listener(Self::bullet_list))
            .on_action(cx.listener(Self::numbered_list))
            .on_action(cx.listener(Self::check_list))
            .on_action(cx.listener(Self::quote))
            .on_action(cx.listener(Self::code_block))
            .on_action(cx.listener(Self::align_left))
            .on_action(cx.listener(Self::align_center))
            .on_action(cx.listener(Self::align_right))
            .on_action(cx.listener(Self::align_justify))
            .on_action(cx.listener(Self::text_small))
            .on_action(cx.listener(Self::text_normal))
            .on_action(cx.listener(Self::text_large))
            .on_action(cx.listener(Self::text_huge))
            .on_action(cx.listener(Self::indent))
            .on_action(cx.listener(Self::outdent))
            .on_action(cx.listener(Self::toggle_check_action))
            .on_action(cx.listener(Self::insert_divider))
            .on_action(cx.listener(Self::insert_table))
            .on_action(cx.listener(Self::insert_image))
            .on_action(cx.listener(Self::insert_attachment))
            .on_action(cx.listener(Self::insert_link))
            .on_action(cx.listener(Self::remove_link))
            .on_action(cx.listener(Self::open_link_at_caret))
            .on_action(cx.listener(Self::table_row_above))
            .on_action(cx.listener(Self::table_row_below))
            .on_action(cx.listener(Self::table_col_left))
            .on_action(cx.listener(Self::table_col_right))
            .on_action(cx.listener(Self::table_delete_row))
            .on_action(cx.listener(Self::table_delete_col))
            .on_action(cx.listener(Self::table_delete))
            .on_action(cx.listener(Self::table_toggle_header))
            .on_action(cx.listener(Self::image_smaller))
            .on_action(cx.listener(Self::image_larger))
            .on_action(cx.listener(Self::image_fit))
            .on_action(cx.listener(Self::replace_word))
            .on_action(cx.listener(Self::add_word_to_dictionary))
            .on_action(cx.listener(Self::find_action))
            .on_action(cx.listener(Self::find_replace))
            .on_action(cx.listener(Self::find_next))
            .on_action(cx.listener(Self::find_previous))
            .on_action(cx.listener(Self::cancel))
    }
}

fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        url.to_owned()
    } else if url.contains('@') && !url.contains('/') {
        format!("mailto:{url}")
    } else {
        format!("https://{url}")
    }
}

fn url_preview(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("mailto:")
        .to_owned()
}

// -------------------------------------------------------------------------
// IME / text input
// -------------------------------------------------------------------------

impl EntityInputHandler for RichEditor {
    fn text_for_range(
        &mut self,
        range16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let pos = self.model.caret();
        let text = &self.model.doc.slot_text(pos.slot)?.text;
        let s = utf16_to_utf8(text, range16.start);
        let e = utf16_to_utf8(text, range16.end);
        adjusted.replace(utf8_to_utf16(text, s)..utf8_to_utf16(text, e));
        text.get(s..e).map(str::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let sel = self.model.sel;
        let slot = sel.head.slot;
        let text = self
            .model
            .doc
            .slot_text(slot)
            .map_or("", |t| t.text.as_str());
        let range = if sel.is_single_slot() {
            let (s, e) = (sel.start().offset, sel.end().offset);
            utf8_to_utf16(text, s)..utf8_to_utf16(text, e)
        } else {
            let c = utf8_to_utf16(text, sel.head.offset);
            c..c
        };
        Some(UTF16Selection {
            range,
            reversed: sel.is_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let r = self.marked.clone()?;
        let text = self.model.doc.slot_text(self.model.caret().slot)?;
        Some(utf8_to_utf16(&text.text, r.start)..utf8_to_utf16(&text.text, r.end))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range16: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        let slot = self.model.caret().slot;
        let replace = range16
            .map(|r| {
                let t = self
                    .model
                    .doc
                    .slot_text(slot)
                    .map_or("", |t| t.text.as_str());
                utf16_to_utf8(t, r.start)..utf16_to_utf8(t, r.end)
            })
            .or_else(|| self.marked.clone());
        if let Some(r) = replace {
            self.model.set_selection(Selection {
                anchor: Pos::new(slot, r.start),
                head: Pos::new(slot, r.end),
            });
        }
        let had_marked = self.marked.take().is_some();
        if text == "\n" || text == "\r" {
            self.edit(EditKind::Other, cx, Model::enter);
            return;
        }
        let pending = self.model.pending_style.clone();
        let kind = if had_marked {
            EditKind::Other
        } else {
            EditKind::Typing
        };
        self.edit(kind, cx, |m| {
            m.pending_style = pending;
            m.insert_text(text);
        });
        self.maybe_open_slash(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range16: Option<Range<usize>>,
        new_text: &str,
        new_selected16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let slot = self.model.caret().slot;
        let replace = range16
            .map(|r| {
                let t = self
                    .model
                    .doc
                    .slot_text(slot)
                    .map_or("", |t| t.text.as_str());
                utf16_to_utf8(t, r.start)..utf16_to_utf8(t, r.end)
            })
            .or_else(|| self.marked.clone());
        if let Some(r) = replace {
            self.model.set_selection(Selection {
                anchor: Pos::new(slot, r.start),
                head: Pos::new(slot, r.end),
            });
        }
        let rules = self.model.input_rules;
        self.model.input_rules = false;
        self.edit(EditKind::Typing, cx, |m| {
            if new_text.is_empty() {
                m.delete_selection();
            } else {
                m.insert_text(new_text);
            }
        });
        self.model.input_rules = rules;
        let caret = self.model.caret();
        let start = caret.offset - new_text.len().min(caret.offset);
        self.marked = (!new_text.is_empty()).then_some(start..caret.offset);
        if let Some(sel16) = new_selected16 {
            let t = self
                .model
                .doc
                .slot_text(caret.slot)
                .map_or("", |t| t.text.as_str());
            let base16 = utf8_to_utf16(t, start);
            let s = utf16_to_utf8(t, base16 + sel16.start);
            let e = utf16_to_utf8(t, base16 + sel16.end);
            self.model.sel = Selection {
                anchor: Pos::new(caret.slot, s),
                head: Pos::new(caret.slot, e),
            };
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let slot = self.model.caret().slot;
        let layout = self.layouts.get(&slot)?;
        let text = self.model.doc.slot_text(slot)?;
        let s = layout.position_for(utf16_to_utf8(&text.text, range16.start));
        let e = layout.position_for(utf16_to_utf8(&text.text, range16.end));
        Some(Bounds::from_corners(
            s,
            point(e.x.max(s.x + px(1.)), s.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let pos = self.pos_for_point(p)?;
        (pos.slot == self.model.caret().slot).then(|| {
            let text = self
                .model
                .doc
                .slot_text(pos.slot)
                .map_or("", |t| t.text.as_str());
            utf8_to_utf16(text, pos.offset)
        })
    }
}

/// Name of an editor action for keymap files, e.g. `rich_editor::Bold`.
pub fn action_name(action: &dyn Action) -> &'static str {
    action.name()
}
