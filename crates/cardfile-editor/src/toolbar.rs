//! A Word-like formatting toolbar for [`RichEditor`].

use cardfile_doc::{Align, BlockKind, Mark};
use cardfile_i18n::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Action, AnyElement, App, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, px,
};

use crate::editor::*;

/// Text colors offered by the toolbar (`None` = theme default).
pub const TEXT_COLORS: &[(&str, Option<&str>)] = &[
    ("default", None),
    ("gray", Some("#6b7280")),
    ("red", Some("#dc2626")),
    ("orange", Some("#ea580c")),
    ("yellow", Some("#ca8a04")),
    ("green", Some("#16a34a")),
    ("teal", Some("#0d9488")),
    ("blue", Some("#2563eb")),
    ("purple", Some("#9333ea")),
    ("pink", Some("#db2777")),
];

pub const HIGHLIGHT_COLORS: &[(&str, Option<&str>)] = &[
    ("none", None),
    ("yellow", Some("#facc15")),
    ("green", Some("#4ade80")),
    ("cyan", Some("#22d3ee")),
    ("pink", Some("#f472b6")),
    ("orange", Some("#fb923c")),
    ("purple", Some("#c084fc")),
    ("gray", Some("#9ca3af")),
];

pub struct EditorToolbar {
    editor: Entity<RichEditor>,
    _sub: Subscription,
}

impl EditorToolbar {
    pub fn new(editor: Entity<RichEditor>, cx: &mut Context<Self>) -> Self {
        let sub = cx.observe(&editor, |_, _, cx| cx.notify());
        Self { editor, _sub: sub }
    }

    fn dispatch(
        editor: &Entity<RichEditor>,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = editor.read(cx).focus_handle.clone();
        focus.focus(window, cx);
        focus.dispatch_action(action, window, cx);
    }

    fn icon_button(
        &self,
        id: &'static str,
        icon: IconName,
        tip: String,
        active: bool,
        action: Box<dyn Action>,
    ) -> Button {
        let editor = self.editor.clone();
        Button::new(id)
            .icon(icon)
            .ghost()
            .small()
            .selected(active)
            .tooltip(tip)
            .on_click(move |_, window, cx| Self::dispatch(&editor, action.as_ref(), window, cx))
    }

    fn swatches(
        &self,
        id: &'static str,
        icon: IconName,
        tip: String,
        highlight: bool,
    ) -> impl IntoElement {
        let editor = self.editor.clone();
        let colors = if highlight {
            HIGHLIGHT_COLORS
        } else {
            TEXT_COLORS
        };
        Popover::new(id)
            .trigger(
                Button::new((id, 0usize))
                    .icon(icon)
                    .ghost()
                    .small()
                    .tooltip(tip),
            )
            .content(move |_, _window, cx| {
                let popover = cx.entity();
                let mut grid = h_flex().gap(px(4.)).p(px(4.)).flex_wrap().w(px(184.));
                for (i, (name, hex)) in colors.iter().enumerate() {
                    let editor = editor.clone();
                    let popover = popover.clone();
                    let value = hex.map(str::to_owned);
                    let fill = hex
                        .and_then(crate::element::parse_hex)
                        .unwrap_or_else(|| cx.theme().foreground);
                    let label = if hex.is_none() {
                        t(if highlight {
                            "editor.toolbar.no_highlight"
                        } else {
                            "editor.toolbar.default_color"
                        })
                    } else {
                        (*name).to_owned()
                    };
                    grid = grid.child(
                        div()
                            .id(("swatch", i))
                            .size(px(20.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .when(hex.is_some(), |d| d.bg(fill))
                            .when(hex.is_none(), |d| {
                                d.child(div().text_size(px(11.)).text_center().child("∅"))
                            })
                            .cursor_pointer()
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(label.clone())
                                    .build(window, cx)
                            })
                            .on_click(move |_, window, cx| {
                                popover.update(cx, |s, cx| s.dismiss(window, cx));
                                let action: Box<dyn Action> = if highlight {
                                    Box::new(SetHighlight(value.clone()))
                                } else {
                                    Box::new(SetColor(value.clone()))
                                };
                                Self::dispatch(&editor, action.as_ref(), window, cx);
                            }),
                    );
                }
                grid
            })
    }

    fn sep(cx: &App) -> AnyElement {
        div()
            .w(px(1.))
            .h(px(18.))
            .mx(px(4.))
            .bg(cx.theme().border)
            .into_any_element()
    }
}

fn block_label(kind: Option<&BlockKind>) -> String {
    t(match kind {
        Some(BlockKind::Heading(1)) => "editor.toolbar.heading1",
        Some(BlockKind::Heading(2)) => "editor.toolbar.heading2",
        Some(BlockKind::Heading(_)) => "editor.toolbar.heading3",
        Some(BlockKind::Quote) => "editor.toolbar.quote",
        Some(BlockKind::Code { .. }) => "editor.toolbar.code_block",
        Some(BlockKind::BulletList) => "editor.toolbar.bullet_list",
        Some(BlockKind::NumberedList) => "editor.toolbar.numbered_list",
        Some(BlockKind::CheckList { .. }) => "editor.toolbar.check_list",
        _ => "editor.toolbar.paragraph",
    })
}

impl Render for EditorToolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let e = self.editor.read(cx);
        let read_only = e.is_read_only();
        let kind = e.current_block_kind();
        let align = e.current_align();
        let (bold, italic, underline, strike, code) = (
            e.has_mark(Mark::Bold),
            e.has_mark(Mark::Italic),
            e.has_mark(Mark::Underline),
            e.has_mark(Mark::Strike),
            e.has_mark(Mark::Code),
        );
        let (can_undo, can_redo) = (e.can_undo(), e.can_redo());
        let focus = e.focus_handle.clone();
        let is = |k: fn(&BlockKind) -> bool| kind.as_ref().is_some_and(k);

        let style_menu = {
            let focus = focus.clone();
            Button::new("block-style")
                .label(SharedString::from(block_label(kind.as_ref())))
                .ghost()
                .small()
                .w(px(130.))
                .dropdown_caret(true)
                .tooltip(t("editor.toolbar.block_type"))
                .dropdown_menu(move |menu, _, _| {
                    menu.action_context(focus.clone())
                        .menu(t("editor.toolbar.paragraph"), Box::new(Paragraph))
                        .menu(t("editor.toolbar.heading1"), Box::new(Heading1))
                        .menu(t("editor.toolbar.heading2"), Box::new(Heading2))
                        .menu(t("editor.toolbar.heading3"), Box::new(Heading3))
                        .separator()
                        .menu(t("editor.toolbar.quote"), Box::new(Quote))
                        .menu(t("editor.toolbar.code_block"), Box::new(CodeBlock))
                        .separator()
                        .menu(t("editor.toolbar.size_small"), Box::new(TextSmall))
                        .menu(t("editor.toolbar.size_normal"), Box::new(TextNormal))
                        .menu(t("editor.toolbar.size_large"), Box::new(TextLarge))
                        .menu(t("editor.toolbar.size_huge"), Box::new(TextHuge))
                })
        };

        let align_icon = match align {
            Align::Left => IconName::TextAlignStart,
            Align::Center => IconName::TextAlignCenter,
            Align::Right => IconName::TextAlignEnd,
            Align::Justify => IconName::TextAlignJustify,
        };
        let align_menu = {
            let focus = focus.clone();
            Button::new("align")
                .icon(align_icon)
                .ghost()
                .small()
                .dropdown_caret(true)
                .tooltip(t("editor.toolbar.align"))
                .dropdown_menu(move |menu, _, _| {
                    menu.action_context(focus.clone())
                        .menu(t("editor.toolbar.align_left"), Box::new(AlignLeft))
                        .menu(t("editor.toolbar.align_center"), Box::new(AlignCenter))
                        .menu(t("editor.toolbar.align_right"), Box::new(AlignRight))
                        .menu(t("editor.toolbar.align_justify"), Box::new(AlignJustify))
                })
        };

        let insert_menu = {
            let focus = focus.clone();
            Button::new("insert")
                .icon(IconName::Plus)
                .label(t("editor.toolbar.insert"))
                .ghost()
                .small()
                .dropdown_caret(true)
                .dropdown_menu(move |menu, _, _| {
                    menu.action_context(focus.clone())
                        .menu(t("editor.toolbar.image"), Box::new(InsertImage))
                        .menu(t("editor.toolbar.attachment"), Box::new(InsertAttachment))
                        .menu(t("editor.toolbar.table"), Box::new(InsertTable))
                        .menu(t("editor.toolbar.divider"), Box::new(InsertDivider))
                        .separator()
                        .menu(t("editor.toolbar.link"), Box::new(InsertLink))
                })
        };

        let bar = h_flex()
            .id("editor-toolbar")
            .w_full()
            .flex_wrap()
            .gap(px(2.))
            .px(px(8.))
            .py(px(4.))
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                self.icon_button(
                    "undo",
                    IconName::Undo2,
                    t("editor.toolbar.undo"),
                    false,
                    Box::new(Undo),
                )
                .disabled(!can_undo),
            )
            .child(
                self.icon_button(
                    "redo",
                    IconName::Redo2,
                    t("editor.toolbar.redo"),
                    false,
                    Box::new(Redo),
                )
                .disabled(!can_redo),
            )
            .child(Self::sep(cx))
            .child(style_menu)
            .child(Self::sep(cx))
            .child(self.icon_button(
                "bold",
                IconName::Bold,
                t("editor.toolbar.bold"),
                bold,
                Box::new(Bold),
            ))
            .child(self.icon_button(
                "italic",
                IconName::Italic,
                t("editor.toolbar.italic"),
                italic,
                Box::new(Italic),
            ))
            .child(self.icon_button(
                "underline",
                IconName::Underline,
                t("editor.toolbar.underline"),
                underline,
                Box::new(Underline),
            ))
            .child(self.icon_button(
                "strike",
                IconName::Strikethrough,
                t("editor.toolbar.strikethrough"),
                strike,
                Box::new(Strikethrough),
            ))
            .child(self.icon_button(
                "code",
                IconName::Code,
                t("editor.toolbar.code"),
                code,
                Box::new(InlineCode),
            ))
            .child(self.swatches(
                "text-color",
                IconName::Baseline,
                t("editor.toolbar.text_color"),
                false,
            ))
            .child(self.swatches(
                "highlight",
                IconName::Highlighter,
                t("editor.toolbar.highlight"),
                true,
            ))
            .child(self.icon_button(
                "clear",
                IconName::RemoveFormatting,
                t("editor.toolbar.clear_formatting"),
                false,
                Box::new(ClearFormatting),
            ))
            .child(Self::sep(cx))
            .child(self.icon_button(
                "bullets",
                IconName::List,
                t("editor.toolbar.bullet_list"),
                is(|k| matches!(k, BlockKind::BulletList)),
                Box::new(BulletList),
            ))
            .child(self.icon_button(
                "numbers",
                IconName::ListOrdered,
                t("editor.toolbar.numbered_list"),
                is(|k| matches!(k, BlockKind::NumberedList)),
                Box::new(NumberedList),
            ))
            .child(self.icon_button(
                "checks",
                IconName::ListTodo,
                t("editor.toolbar.check_list"),
                is(|k| matches!(k, BlockKind::CheckList { .. })),
                Box::new(CheckList),
            ))
            .child(self.icon_button(
                "outdent",
                IconName::ListIndentDecrease,
                t("editor.toolbar.outdent"),
                false,
                Box::new(Outdent),
            ))
            .child(self.icon_button(
                "indent",
                IconName::ListIndentIncrease,
                t("editor.toolbar.indent"),
                false,
                Box::new(Indent),
            ))
            .child(align_menu)
            .child(Self::sep(cx))
            .child(self.icon_button(
                "link",
                IconName::Link,
                t("editor.toolbar.link"),
                false,
                Box::new(InsertLink),
            ))
            .child(insert_menu);
        if read_only {
            div().w_full().child(bar.opacity(0.45)).into_any_element()
        } else {
            bar.into_any_element()
        }
    }
}
