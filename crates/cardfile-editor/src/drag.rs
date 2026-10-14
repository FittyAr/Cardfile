//! Dragging images and attachments: within a card to move them, or onto another
//! card in the list (handled by the host, which receives [`DraggedBlock`]).

use cardfile_doc::{Block, BlockKind};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, EntityId, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window,
    div, px,
};

use crate::editor::{EditorEvent, RichEditor};
use crate::history::EditKind;
use crate::pos::{Pos, Slot};

/// An image or attachment block being dragged out of an editor.
#[derive(Clone, Debug)]
pub struct DraggedBlock {
    /// The editor it comes from.
    pub source: EntityId,
    /// Its index in the source document when the drag started.
    pub index: usize,
    pub block: Block,
    pub label: SharedString,
}

impl DraggedBlock {
    pub(crate) fn new(source: EntityId, index: usize, block: &Block) -> Option<Self> {
        let label = match &block.kind {
            BlockKind::Image(img) if !img.alt.is_empty() => format!("🖼 {}", img.alt),
            BlockKind::Image(_) => "🖼".to_owned(),
            BlockKind::Attachment(att) => format!("📎 {}", att.file_name),
            _ => return None,
        };
        Some(Self {
            source,
            index,
            block: block.clone(),
            label: label.into(),
        })
    }
}

/// What follows the pointer while dragging.
pub struct DragPreview(pub SharedString);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().primary)
            .shadow_md()
            .text_size(px(13.))
            .child(self.0.clone())
    }
}

impl RichEditor {
    /// Drop a dragged block before block `target` (or at the end when `target` is the
    /// block count). From this editor the block moves, or is duplicated with `copy`;
    /// from elsewhere it is inserted.
    pub(crate) fn drop_block(
        &mut self,
        dragged: &DraggedBlock,
        target: usize,
        copy: bool,
        cx: &mut Context<Self>,
    ) {
        let own = dragged.source == cx.entity_id();
        let index = dragged.index;
        if own && !copy && (target == index || target == index + 1) {
            return;
        }
        let block = dragged.block.clone();
        self.edit_blocks(cx, move |blocks| {
            let mut at = target.min(blocks.len());
            if own && !copy {
                if blocks.get(index) != Some(&block) {
                    return None;
                }
                blocks.remove(index);
                if index < at {
                    at -= 1;
                }
            }
            blocks.insert(at, block);
            Some(at)
        });
    }

    /// Remove the block at `index` if it is still `expected` (it was moved to another
    /// card). Returns whether it was removed.
    pub fn take_block(&mut self, index: usize, expected: &Block, cx: &mut Context<Self>) -> bool {
        if self.is_read_only() || self.model.doc.blocks.get(index) != Some(expected) {
            return false;
        }
        let expected = expected.clone();
        self.edit_blocks(cx, move |blocks| {
            if blocks.get(index) != Some(&expected) {
                return None;
            }
            blocks.remove(index);
            Some(index.saturating_sub(1))
        });
        true
    }

    /// Apply a structural change to the block list as one undoable step; `f` returns
    /// the block to put the caret on, or `None` to cancel.
    fn edit_blocks(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Vec<Block>) -> Option<usize>,
    ) {
        if self.is_read_only() {
            cx.emit(EditorEvent::EditBlocked);
            return;
        }
        let mut blocks = self.model.doc.blocks.clone();
        let Some(caret) = f(&mut blocks) else {
            return;
        };
        self.edit(EditKind::Other, cx, move |m| {
            m.doc.blocks = blocks;
            m.doc.normalize();
            let caret = caret.min(m.doc.blocks.len().saturating_sub(1));
            m.set_caret(Pos::new(Slot::block(caret), 0));
        });
        self.layouts.clear();
    }
}
