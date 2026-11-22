//! The "/" command menu: typing `/` in an empty paragraph offers block types.

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _,
    anchored, deferred, div, point, px,
};
use notechest_doc::{Block, BlockKind};
use notechest_i18n::t;

use crate::editor::RichEditor;
use crate::pos::{Pos, Selection, Slot, Slots};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlashItem {
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Bullets,
    Numbers,
    Checklist,
    Quote,
    Code,
    Table,
    Divider,
    Image,
    Attachment,
}

const ITEMS: &[(SlashItem, &str, &str)] = &[
    (
        SlashItem::Paragraph,
        "editor.toolbar.paragraph",
        "text paragraph normal",
    ),
    (
        SlashItem::Heading1,
        "editor.toolbar.heading1",
        "h1 title heading",
    ),
    (
        SlashItem::Heading2,
        "editor.toolbar.heading2",
        "h2 heading subtitle",
    ),
    (SlashItem::Heading3, "editor.toolbar.heading3", "h3 heading"),
    (
        SlashItem::Bullets,
        "editor.toolbar.bullet_list",
        "list bullet ul",
    ),
    (
        SlashItem::Numbers,
        "editor.toolbar.numbered_list",
        "list number ol",
    ),
    (
        SlashItem::Checklist,
        "editor.toolbar.check_list",
        "todo task check",
    ),
    (SlashItem::Quote, "editor.toolbar.quote", "quote blockquote"),
    (SlashItem::Code, "editor.toolbar.code_block", "code snippet"),
    (SlashItem::Table, "editor.toolbar.table", "table grid"),
    (
        SlashItem::Divider,
        "editor.toolbar.divider",
        "line rule hr divider",
    ),
    (
        SlashItem::Image,
        "editor.toolbar.image",
        "image picture photo",
    ),
    (
        SlashItem::Attachment,
        "editor.toolbar.attachment",
        "file attachment",
    ),
];

#[derive(Clone, Debug)]
pub(crate) struct SlashMenu {
    pub slot: Slot,
    /// Offset of the `/`.
    pub start: usize,
    pub selected: usize,
}

/// The lower-cased query typed after the `/` at byte `start`, up to the caret at
/// byte `caret`. `None` when the caret is not past the `/`, the `/` is gone or the
/// query holds a space (the user moved on to normal text).
fn query_at(text: &str, start: usize, caret: usize) -> Option<String> {
    if caret <= start || !text.get(start..)?.starts_with('/') {
        return None;
    }
    let q = text.get(start + 1..caret)?;
    (!q.contains(' ')).then(|| q.to_lowercase())
}

/// Whether an item with this translated `label` and search `words` matches the
/// lower-cased query `q`. An empty query matches everything.
fn item_matches(q: &str, label: &str, words: &str) -> bool {
    q.is_empty() || label.to_lowercase().contains(q) || words.contains(q)
}

impl RichEditor {
    /// Called after text input: open the menu on `/` typed in an empty paragraph.
    pub(crate) fn maybe_open_slash(&mut self, typed: &str) {
        if typed != "/" || self.slash.is_some() || self.single_line {
            return;
        }
        let caret = self.model.caret();
        let Some(block) = self.model.doc.blocks.get(caret.slot.block) else {
            return;
        };
        if self.model.doc.is_cell(caret.slot)
            || !matches!(block.kind, BlockKind::Paragraph)
            || block.text.text != "/"
        {
            return;
        }
        self.slash = Some(SlashMenu {
            slot: caret.slot,
            start: 0,
            selected: 0,
        });
    }

    fn slash_query(&self) -> Option<String> {
        let menu = self.slash.as_ref()?;
        let caret = self.model.caret();
        let text = self.model.doc.slot_text(menu.slot)?;
        if caret.slot != menu.slot {
            return None;
        }
        query_at(&text.text, menu.start, caret.offset)
    }

    fn slash_items(&self) -> Vec<(SlashItem, String)> {
        let q = self.slash_query().unwrap_or_default();
        ITEMS
            .iter()
            .map(|(item, key, words)| (*item, t(key), *words))
            .filter(|(_, label, words)| item_matches(&q, label, words))
            .map(|(item, label, _)| (item, label))
            .collect()
    }

    /// Close the menu if the caret left it or nothing matches.
    pub(crate) fn validate_slash(&mut self) {
        if self.slash.is_some() && (self.slash_query().is_none() || self.slash_items().is_empty()) {
            self.slash = None;
        }
    }

    /// Handle navigation keys while the menu is open. Returns `true` if consumed.
    pub(crate) fn slash_key(&mut self, key: SlashKey, cx: &mut Context<Self>) -> bool {
        if self.slash.is_none() {
            return false;
        }
        let n = self.slash_items().len();
        match key {
            SlashKey::Up | SlashKey::Down => {
                if let Some(m) = &mut self.slash {
                    m.selected = crate::util::cycle(m.selected, n, key == SlashKey::Down);
                }
                cx.notify();
            }
            SlashKey::Enter => {
                let selected = self.slash.as_ref().map_or(0, |m| m.selected);
                if let Some((item, _)) = self.slash_items().get(selected).cloned() {
                    self.apply_slash(item, cx);
                }
            }
            SlashKey::Escape => {
                self.slash = None;
                cx.notify();
            }
        }
        true
    }

    fn apply_slash(&mut self, item: SlashItem, cx: &mut Context<Self>) {
        let Some(menu) = self.slash.take() else {
            return;
        };
        let caret = self.model.caret();
        let (slot, start, end) = (menu.slot, menu.start, caret.offset);
        let wants_files = matches!(item, SlashItem::Image | SlashItem::Attachment);
        self.edit_with(cx, move |m| {
            // Remove the "/query" text.
            m.set_selection(Selection {
                anchor: Pos::new(slot, start),
                head: Pos::new(slot, end),
            });
            m.delete_selection();
            match item {
                SlashItem::Paragraph => {}
                SlashItem::Heading1 => m.set_block_kind(BlockKind::Heading(1)),
                SlashItem::Heading2 => m.set_block_kind(BlockKind::Heading(2)),
                SlashItem::Heading3 => m.set_block_kind(BlockKind::Heading(3)),
                SlashItem::Bullets => m.set_block_kind(BlockKind::BulletList),
                SlashItem::Numbers => m.set_block_kind(BlockKind::NumberedList),
                SlashItem::Checklist => m.set_block_kind(BlockKind::CheckList { checked: false }),
                SlashItem::Quote => m.set_block_kind(BlockKind::Quote),
                SlashItem::Code => m.set_block_kind(BlockKind::Code {
                    language: String::new(),
                }),
                SlashItem::Table => m.insert_table(3, 3),
                SlashItem::Divider => m.insert_block(Block::new(BlockKind::Divider, "")),
                SlashItem::Image | SlashItem::Attachment => {}
            }
        });
        if wants_files {
            self.pick_files_public(item == SlashItem::Image, cx);
        }
    }

    pub(crate) fn render_slash(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.slash.clone()?;
        let layout = self.layouts.get(&menu.slot)?;
        let at = layout.position_for(menu.start);
        let lh = layout.line_height;
        let items = self.slash_items();
        let th = cx.theme();
        let mut list = v_flex().w(px(220.)).p(px(4.)).gap(px(1.));
        for (i, (item, label)) in items.into_iter().enumerate() {
            let selected = i == menu.selected;
            list = list.child(
                h_flex()
                    .id(("slash", i))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(4.))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(th.list_active))
                    .hover(|d| d.bg(th.list_hover))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.apply_slash(item, cx);
                        }),
                    )
                    .child(label),
            );
        }
        Some(
            deferred(
                anchored()
                    .position(point(at.x, at.y + lh))
                    .snap_to_window()
                    .child(
                        div()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(th.border)
                            .bg(th.popover)
                            .shadow_lg()
                            .child(list),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlashKey {
    Up,
    Down,
    Enter,
    Escape,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_the_text_between_slash_and_caret() {
        assert_eq!(query_at("/", 0, 1).as_deref(), Some(""));
        assert_eq!(query_at("/Head", 0, 5).as_deref(), Some("head"));
        assert_eq!(query_at("/heading", 0, 3).as_deref(), Some("he"));
    }

    #[test]
    fn query_needs_the_caret_after_the_slash() {
        assert_eq!(query_at("/h", 0, 0), None);
        assert_eq!(query_at("x/h", 1, 1), None);
        assert_eq!(query_at("x/h", 1, 3).as_deref(), Some("h"));
    }

    #[test]
    fn query_ends_when_the_slash_is_gone_or_a_space_is_typed() {
        assert_eq!(query_at("h", 0, 1), None);
        assert_eq!(query_at("/a b", 0, 4), None);
        assert_eq!(query_at("", 0, 1), None);
        // The menu start past the end of the text (the text was cut).
        assert_eq!(query_at("/", 3, 4), None);
    }

    #[test]
    fn query_out_of_char_bounds_is_rejected_not_a_panic() {
        // 'é' is two bytes: offset 2 falls inside it.
        assert_eq!(query_at("/é", 0, 2), None);
        assert_eq!(query_at("/é", 0, 3).as_deref(), Some("é"));
        assert_eq!(query_at("/ÉCOLE", 0, 7).as_deref(), Some("école"));
    }

    #[test]
    fn items_match_by_label_or_keywords() {
        assert!(item_matches("", "Heading 1", "h1 title heading"));
        assert!(item_matches("head", "Heading 1", "h1 title heading"));
        assert!(item_matches("h1", "Heading 1", "h1 title heading"));
        assert!(item_matches("título", "Título 1", "h1 title heading"));
        assert!(!item_matches("table", "Heading 1", "h1 title heading"));
    }
}
