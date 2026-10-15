//! Rendering of the document blocks.

use cardfile_doc::{Align, Block, BlockKind, RichText, TextSize};
use cardfile_i18n::t;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, CursorStyle, Entity, ExternalPaths, Hsla,
    ImageSource, InteractiveElement, IntoElement, MouseButton, ObjectFit, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, StyledImage, TextAlign, Window, canvas, div,
    img, px,
};

use crate::drag::{DragPreview, DraggedBlock};
use crate::editor::*;
use crate::element::{SlotElement, SlotStyle};
use crate::media::MediaColors;
use crate::pos::Slot;

/// Theme colors and fonts resolved once per frame.
#[derive(Clone)]
struct Palette {
    fg: Hsla,
    muted_fg: Hsla,
    link: Hsla,
    code_bg: Hsla,
    selection: Hsla,
    caret: Hsla,
    danger: Hsla,
    border: Hsla,
    primary: Hsla,
    header_bg: Hsla,
    found: Hsla,
    found_current: Hsla,
    font: SharedString,
    mono: SharedString,
}

impl Palette {
    fn from(cx: &App) -> Self {
        let th = cx.theme();
        Self {
            fg: th.foreground,
            muted_fg: th.muted_foreground,
            link: th.link,
            code_bg: th.muted,
            selection: th.selection,
            caret: th.caret,
            danger: th.danger,
            border: th.border,
            primary: th.primary,
            header_bg: th.muted,
            found: th.warning.opacity(0.3),
            found_current: th.warning.opacity(0.75),
            font: th.font_family.clone(),
            mono: th.mono_font_family.clone(),
        }
    }
}

fn text_align(a: Align) -> TextAlign {
    match a {
        Align::Left | Align::Justify => TextAlign::Left,
        Align::Center => TextAlign::Center,
        Align::Right => TextAlign::Right,
    }
}

const BULLETS: [&str; 3] = ["•", "◦", "▪"];
const INDENT_PX: f32 = 24.;

impl RichEditor {
    fn slot_style(&self, block: &Block, pal: &Palette, cell_header: bool) -> SlotStyle {
        let base = self.base_font_size;
        let (scale, bold, lh) = match block.kind {
            BlockKind::Heading(1) => (1.75, true, 1.3),
            BlockKind::Heading(2) => (1.4, true, 1.3),
            BlockKind::Heading(_) => (1.2, true, 1.35),
            BlockKind::Code { .. } => (0.92, false, 1.45),
            _ => (block.size.scale(), false, 1.55),
        };
        let checked = matches!(block.kind, BlockKind::CheckList { checked: true });
        let font_size = base * scale;
        SlotStyle {
            font_family: pal.font.clone(),
            mono_family: pal.mono.clone(),
            font_size,
            line_height: (font_size * lh).round(),
            color: if checked || matches!(block.kind, BlockKind::Quote) {
                pal.muted_fg
            } else {
                pal.fg
            },
            muted: pal.muted_fg,
            link: pal.link,
            code_bg: pal.code_bg,
            selection: pal.selection,
            caret: pal.caret,
            misspelled: pal.danger,
            found: pal.found,
            found_current: pal.found_current,
            bold: bold || cell_header || self.strong,
            italic: matches!(block.kind, BlockKind::Quote),
            mono: matches!(block.kind, BlockKind::Code { .. }),
            strike: checked,
            // Right-to-left paragraphs read from the right unless aligned explicitly.
            align: if block.align == Align::Left && crate::bidi::is_rtl_paragraph(&block.text.text)
            {
                TextAlign::Right
            } else {
                text_align(block.align)
            },
            placeholder: None,
        }
    }

    fn slot(
        &self,
        entity: &Entity<Self>,
        slot: Slot,
        text: &RichText,
        style: SlotStyle,
    ) -> AnyElement {
        SlotElement::new(entity.clone(), slot, text.clone(), style).into_any_element()
    }

    fn render_block(
        &mut self,
        ix: usize,
        block: &Block,
        number: Option<usize>,
        pal: &Palette,
        entity: &Entity<Self>,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.is_atom_selected(ix);
        let mut style = self.slot_style(block, pal, false);
        let indent = px(f32::from(block.indent) * INDENT_PX);
        match &block.kind {
            BlockKind::Paragraph | BlockKind::Heading(_) => {
                if ix == 0 && self.model.doc.is_empty() {
                    style.placeholder = self.placeholder.clone().map(Into::into);
                }
                let top = if matches!(block.kind, BlockKind::Heading(_)) && ix > 0 {
                    px(10.)
                } else {
                    px(0.)
                };
                div()
                    .pt(top)
                    .pl(indent)
                    .child(self.slot(entity, Slot::block(ix), &block.text, style))
                    .into_any_element()
            }
            BlockKind::BulletList | BlockKind::NumberedList | BlockKind::CheckList { .. } => {
                let lh = style.line_height;
                let marker: AnyElement = match &block.kind {
                    BlockKind::CheckList { checked } => {
                        let checked = *checked;
                        let ent = entity.clone();
                        div()
                            .id(("check", ix))
                            .size(px(15.))
                            .mt((lh - px(15.)) / 2.)
                            .rounded(px(3.))
                            .border_1()
                            .border_color(if checked { pal.primary } else { pal.muted_fg })
                            .when(checked, |d| d.bg(pal.primary))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(gpui_kit::white())
                            .text_size(px(11.))
                            .cursor(CursorStyle::PointingHand)
                            .when(checked, |d| d.child("✓"))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                ent.update(cx, |e, cx| e.toggle_check(ix, cx));
                            })
                            .into_any_element()
                    }
                    BlockKind::NumberedList => div()
                        .text_size(style.font_size)
                        .line_height(lh)
                        .text_color(pal.muted_fg)
                        .child(format!("{}.", number.unwrap_or(1)))
                        .into_any_element(),
                    _ => div()
                        .text_size(style.font_size)
                        .line_height(lh)
                        .text_color(pal.muted_fg)
                        .child(BULLETS[block.indent as usize % BULLETS.len()])
                        .into_any_element(),
                };
                h_flex()
                    .items_start()
                    .pl(indent)
                    .child(
                        div()
                            .w(px(26.))
                            .flex_shrink_0()
                            .flex()
                            .justify_end()
                            .pr(px(8.))
                            .child(marker),
                    )
                    .child(div().flex_1().min_w_0().child(self.slot(
                        entity,
                        Slot::block(ix),
                        &block.text,
                        style,
                    )))
                    .into_any_element()
            }
            BlockKind::Quote => div()
                .ml(indent)
                .border_l_3()
                .border_color(pal.border)
                .pl(px(12.))
                .child(self.slot(entity, Slot::block(ix), &block.text, style))
                .into_any_element(),
            BlockKind::Code { language } => v_flex()
                .ml(indent)
                .rounded(px(6.))
                .bg(pal.code_bg)
                .px(px(12.))
                .py(px(8.))
                .when(!language.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(11.))
                            .text_color(pal.muted_fg)
                            .child(language.clone()),
                    )
                })
                .child(self.slot(entity, Slot::block(ix), &block.text, style))
                .into_any_element(),
            BlockKind::Divider => {
                let ent = entity.clone();
                div()
                    .id(("divider", ix))
                    .py(px(8.))
                    .cursor(CursorStyle::PointingHand)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        ent.update(cx, |e, cx| e.select_atom(ix, window, cx));
                    })
                    .child(div().h(px(if selected { 2. } else { 1. })).bg(if selected {
                        pal.primary
                    } else {
                        pal.border
                    }))
                    .into_any_element()
            }
            BlockKind::Image(image) => {
                let ent = entity.clone();
                let dragged = DraggedBlock::new(entity.entity_id(), ix, block);
                let loaded = self.image_for(image);
                let (width, height) =
                    crate::image_resize::preview_size(self.image_drag.as_ref(), ix)
                        .map_or((image.width, image.height), |(w, h)| (Some(w), h));
                let body: AnyElement = match loaded {
                    Some(data) => {
                        let el = img(ImageSource::Image(data)).max_w_full();
                        match (width, height) {
                            // Stretched: show exactly the chosen box.
                            // An explicit ratio stops GPUI from imposing the image's own.
                            (Some(w), Some(h)) => el
                                .object_fit(ObjectFit::Fill)
                                .aspect_ratio(w / h.max(1.))
                                .w(px(w))
                                .h(px(h))
                                .into_any_element(),
                            (Some(w), None) => el
                                .object_fit(ObjectFit::Contain)
                                .w(px(w))
                                .into_any_element(),
                            _ => el
                                .object_fit(ObjectFit::Contain)
                                .w_full()
                                .into_any_element(),
                        }
                    }
                    None => div()
                        .p(px(16.))
                        .text_color(pal.muted_fg)
                        .child(format!("🖼 {}", image.alt))
                        .into_any_element(),
                };
                div()
                    .id(("image", ix))
                    .flex()
                    .when(block.align == Align::Center, |d| d.justify_center())
                    .when(block.align == Align::Right, |d| d.justify_end())
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        ent.update(cx, |e, cx| e.select_atom(ix, window, cx));
                    })
                    .child(
                        div()
                            .id(("image-drag", ix))
                            .border_2()
                            .rounded(px(4.))
                            .border_color(if selected {
                                pal.primary
                            } else {
                                gpui_kit::transparent_black()
                            })
                            .when_some(dragged, |d, dragged| {
                                d.on_drag(dragged, |d: &DraggedBlock, _, _, cx| {
                                    cx.new(|_| DragPreview(d.label.clone()))
                                })
                            })
                            .relative()
                            .child(body)
                            .child({
                                // Remember the painted size: resizing starts from it.
                                let bounds = self.image_bounds.clone();
                                canvas(
                                    move |b, _, _| {
                                        bounds.borrow_mut().insert(ix, b);
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full()
                            })
                            .when(selected && !self.is_read_only(), |d| {
                                d.children(Self::resize_handles(ix, pal.primary, entity))
                            }),
                    )
                    .into_any_element()
            }
            BlockKind::Attachment(att) => {
                let dragged = DraggedBlock::new(entity.entity_id(), ix, block);
                if let Some(kind) = crate::media::playable(att) {
                    let colors = MediaColors {
                        fg: pal.fg,
                        muted: pal.muted_fg,
                        primary: pal.primary,
                        border: pal.border,
                        track: pal.code_bg,
                    };
                    let player = self.render_media(ix, att, kind, selected, colors, entity);
                    let ent = entity.clone();
                    return div()
                        .id(("media", ix))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            ent.update(cx, |e, cx| e.select_atom(ix, window, cx));
                        })
                        .when_some(dragged, |d, dragged| {
                            d.on_drag(dragged, |d: &DraggedBlock, _, _, cx| {
                                cx.new(|_| DragPreview(d.label.clone()))
                            })
                        })
                        .child(player)
                        .into_any_element();
                }
                let ent = entity.clone();
                let ent2 = entity.clone();
                h_flex()
                    .id(("attachment", ix))
                    .when_some(dragged, |d, dragged| {
                        d.on_drag(dragged, |d: &DraggedBlock, _, _, cx| {
                            cx.new(|_| DragPreview(d.label.clone()))
                        })
                    })
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if selected { pal.primary } else { pal.border })
                    .cursor(CursorStyle::PointingHand)
                    .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                        cx.stop_propagation();
                        if ev.click_count >= 2 {
                            ent2.update(cx, |e, cx| e.open_attachment(ix, cx));
                        } else {
                            ent.update(cx, |e, cx| e.select_atom(ix, window, cx));
                        }
                    })
                    .child("📎")
                    .child(div().text_color(pal.fg).child(att.file_name.clone()))
                    .child(
                        div()
                            .text_color(pal.muted_fg)
                            .text_size(px(12.))
                            .child(crate::util::format_size(att.size)),
                    )
                    .into_any_element()
            }
            BlockKind::Table(table) => {
                let mut rows = v_flex()
                    .w_full()
                    .border_1()
                    .border_color(pal.border)
                    .rounded(px(4.));
                for (r, row) in table.rows.iter().enumerate() {
                    let header = r == 0 && table.header;
                    let mut line = h_flex().w_full().items_stretch();
                    for (c, cell) in row.iter().enumerate() {
                        let st = self.slot_style(&Block::paragraph(""), pal, header);
                        line = line.child(
                            div()
                                .flex_1()
                                .min_w(px(60.))
                                .px(px(8.))
                                .py(px(4.))
                                .when(c > 0, |d| d.border_l_1().border_color(pal.border))
                                .when(header, |d| d.bg(pal.header_bg))
                                .child(self.slot(entity, Slot::cell(ix, r, c), cell, st)),
                        );
                    }
                    rows = rows.child(
                        div()
                            .when(r > 0, |d| d.border_t_1().border_color(pal.border))
                            .child(line),
                    );
                }
                div().ml(indent).py(px(4.)).child(rows).into_any_element()
            }
        }
    }

    /// The three resize handles of a selected image.
    fn resize_handles(ix: usize, color: Hsla, entity: &Entity<Self>) -> Vec<AnyElement> {
        use crate::image_resize::{HANDLE, Handle};
        let half = HANDLE / 2.;
        [
            (Handle::Right, "resize-r", CursorStyle::ResizeLeftRight),
            (Handle::Bottom, "resize-b", CursorStyle::ResizeUpDown),
            (
                Handle::Corner,
                "resize-c",
                CursorStyle::ResizeUpLeftDownRight,
            ),
        ]
        .into_iter()
        .map(|(handle, id, cursor)| {
            let ent = entity.clone();
            let el = div()
                .id((id, ix))
                .absolute()
                .size(HANDLE)
                .rounded(px(2.))
                .bg(color)
                .border_1()
                .border_color(gpui_kit::white())
                .cursor(cursor)
                .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                    cx.stop_propagation();
                    ent.update(cx, |e, cx| e.start_image_drag(ix, handle, ev.position, cx));
                });
            match handle {
                Handle::Right => el.right(-half).top(gpui_kit::relative(0.5)).mt(-half),
                Handle::Bottom => el.bottom(-half).left(gpui_kit::relative(0.5)).ml(-half),
                Handle::Corner => el.right(-half).bottom(-half),
            }
            .into_any_element()
        })
        .collect()
    }

    /// Wrap a block so images and attachments can be dropped before it.
    fn drop_zone(ix: usize, el: AnyElement, color: Hsla, cx: &mut Context<Self>) -> AnyElement {
        div()
            .border_t_2()
            .border_color(gpui_kit::transparent_black())
            .drag_over::<DraggedBlock>(move |s, _, _, _| s.border_color(color))
            .on_drop(cx.listener(move |this, d: &DraggedBlock, window, cx| {
                let copy = window.modifiers().control || window.modifiers().alt;
                this.drop_block(d, ix, copy, cx);
            }))
            .child(el)
            .into_any_element()
    }

    fn context_menu(editor: &Entity<RichEditor>, menu: PopupMenu, cx: &App) -> PopupMenu {
        let e = editor.read(cx);
        let mut menu = menu.action_context(e.focus_handle.clone());
        if let Some((_, suggestions)) = e.spelling_suggestions() {
            if suggestions.is_empty() {
                menu = menu.label(t("editor.menu.no_suggestions"));
            }
            for s in suggestions {
                menu = menu.menu(s.clone(), Box::new(ReplaceWord(s)));
            }
            menu = menu
                .menu(
                    t("editor.menu.add_to_dictionary"),
                    Box::new(AddWordToDictionary),
                )
                .separator();
        }
        menu = menu
            .menu(t("editor.menu.cut"), Box::new(Cut))
            .menu(t("editor.menu.copy"), Box::new(Copy))
            .menu(t("editor.menu.paste"), Box::new(Paste))
            .menu(t("editor.menu.paste_plain"), Box::new(PastePlain))
            .separator()
            .menu(t("editor.menu.select_all"), Box::new(SelectAll));
        if e.link_at_caret().is_some() {
            menu = menu
                .separator()
                .menu(t("editor.menu.open_link"), Box::new(OpenLinkAtCaret))
                .menu(t("editor.toolbar.remove_link"), Box::new(RemoveLink));
        }
        if e.in_table() {
            menu = menu
                .separator()
                .menu(t("editor.table.row_above"), Box::new(TableInsertRowAbove))
                .menu(t("editor.table.row_below"), Box::new(TableInsertRowBelow))
                .menu(t("editor.table.col_left"), Box::new(TableInsertColumnLeft))
                .menu(
                    t("editor.table.col_right"),
                    Box::new(TableInsertColumnRight),
                )
                .menu(t("editor.table.delete_row"), Box::new(TableDeleteRow))
                .menu(t("editor.table.delete_col"), Box::new(TableDeleteColumn))
                .menu(t("editor.table.toggle_header"), Box::new(TableToggleHeader))
                .menu(t("editor.table.delete"), Box::new(TableDelete));
        }
        if e.selected_image() {
            menu = menu
                .separator()
                .menu(t("editor.image.smaller"), Box::new(ImageSmaller))
                .menu(t("editor.image.larger"), Box::new(ImageLarger))
                .menu(t("editor.image.fit"), Box::new(ImageFit))
                .menu(t("editor.image.keep_ratio"), Box::new(ImageKeepRatio));
        }
        menu
    }
}

impl Render for RichEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_spelling();
        // Drop layouts of slots that no longer exist; paint refills the rest.
        let blocks = self.model.doc.blocks.clone();
        self.layouts.retain(|s, _| s.block < blocks.len());
        let pal = Palette::from(cx);
        let entity = cx.entity();

        let mut numbers: Vec<usize> = Vec::new();
        let mut children = Vec::with_capacity(blocks.len());
        for (ix, block) in blocks.iter().enumerate() {
            let level = block.indent as usize;
            let number = if matches!(block.kind, BlockKind::NumberedList) {
                numbers.resize(level + 1, 0);
                numbers.truncate(level + 1);
                numbers[level] += 1;
                Some(numbers[level])
            } else {
                if !block.kind.is_list() {
                    numbers.clear();
                } else {
                    numbers.truncate(level);
                }
                None
            };
            let el = self.render_block(ix, block, number, &pal, &entity, cx);
            children.push(if self.single_line {
                el
            } else {
                Self::drop_zone(ix, el, pal.primary, cx)
            });
        }

        let focus = self.focus_handle.clone();
        let input_entity = entity.clone();
        let read_only = self.is_read_only();
        let input_handler = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                if !read_only {
                    window.handle_input(
                        &focus,
                        gpui_kit::ElementInputHandler::new(bounds, input_entity.clone()),
                        cx,
                    );
                }
            },
        )
        .absolute()
        .size_full();

        let content = if self.single_line {
            v_flex().w_full().px(px(2.)).py(px(4.)).children(children)
        } else {
            v_flex()
                .w_full()
                .px(px(28.))
                .py(px(20.))
                .gap(px(6.))
                .children(children)
                .child(Self::drop_zone(
                    blocks.len(),
                    div().h(px(80.)).into_any_element(),
                    pal.primary,
                    cx,
                ))
        };
        let scroll = div()
            .id("rich-editor-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(content);

        let root = div()
            .id("rich-editor")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .cursor(CursorStyle::IBeam)
            .text_color(pal.fg)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.insert_files(paths.paths().to_vec(), cx);
            }))
            .child(scroll)
            .child(input_handler);
        let root = root.children(self.render_slash(cx));
        let editor = entity.clone();
        let root = Self::bind_actions(root, cx)
            .context_menu(move |menu, _window, cx| Self::context_menu(&editor, menu, cx));
        v_flex()
            .size_full()
            .children(self.render_find_bar(cx))
            .child(div().flex_1().min_h_0().child(root))
    }
}

#[allow(dead_code)]
fn _size_scale_used(s: TextSize) -> f32 {
    s.scale()
}
