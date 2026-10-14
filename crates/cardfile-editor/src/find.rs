//! Find and replace inside the current card.

use std::ops::Range;

use cardfile_i18n::{t, t_args};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _, h_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Styled as _, Subscription, Window, div, px,
};

use crate::editor::RichEditor;
use crate::pos::{Pos, Selection, Slot};

pub(crate) struct FindBar {
    query: Entity<InputState>,
    replacement: Entity<InputState>,
    show_replace: bool,
    case_sensitive: bool,
    pub(crate) matches: Vec<(Slot, Range<usize>)>,
    pub(crate) current: Option<usize>,
    _subs: Vec<Subscription>,
}

impl RichEditor {
    /// Open the find bar (or focus it), prefilled with the selected text.
    pub(crate) fn open_find(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.is_none() {
            let query =
                cx.new(|cx| InputState::new(window, cx).placeholder(t("editor.find.placeholder")));
            let replacement = cx.new(|cx| {
                InputState::new(window, cx).placeholder(t("editor.find.replace_placeholder"))
            });
            let subs = vec![
                cx.subscribe_in(
                    &query,
                    window,
                    |this, _, ev: &InputEvent, window, cx| match ev {
                        InputEvent::Change => this.refresh_matches(true, cx),
                        InputEvent::PressEnter { shift, .. } => this.find_step(!*shift, window, cx),
                        _ => {}
                    },
                ),
                cx.subscribe_in(
                    &replacement,
                    window,
                    |this, _, ev: &InputEvent, window, cx| {
                        if let InputEvent::PressEnter { .. } = ev {
                            this.replace_current(window, cx);
                        }
                    },
                ),
            ];
            self.find = Some(FindBar {
                query,
                replacement,
                show_replace: replace,
                case_sensitive: false,
                matches: Vec::new(),
                current: None,
                _subs: subs,
            });
        }
        let selected = (!self.model.sel.is_empty() && self.model.sel.is_single_slot())
            .then(|| self.model.selected_text())
            .filter(|s| !s.contains('\n'));
        if let Some(bar) = &mut self.find {
            bar.show_replace |= replace;
            let query = bar.query.clone();
            if let Some(text) = selected {
                query.update(cx, |s, cx| s.set_value(text, window, cx));
            }
            let focus = query.focus_handle(cx);
            focus.focus(window, cx);
        }
        self.refresh_matches(true, cx);
        cx.notify();
    }

    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    /// Recompute matches; with `select_nearest`, jump to the first match after the caret.
    pub(crate) fn refresh_matches(&mut self, select_nearest: bool, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let query = bar.query.read(cx).value().to_string();
        let matches = self.model.find_matches(&query, bar.case_sensitive);
        let caret = self.model.sel.start();
        let current = if matches.is_empty() {
            None
        } else if select_nearest {
            Some(
                matches
                    .iter()
                    .position(|(s, r)| Pos::new(*s, r.start) >= caret)
                    .unwrap_or(0),
            )
        } else {
            bar.current.map(|c| c.min(matches.len() - 1))
        };
        if let Some(bar) = &mut self.find {
            bar.matches = matches;
            bar.current = current;
        }
        if select_nearest {
            self.select_current_match(cx);
        }
        cx.notify();
    }

    fn select_current_match(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let Some((slot, range)) = bar.current.and_then(|i| bar.matches.get(i)).cloned() else {
            return;
        };
        self.model.set_selection(Selection {
            anchor: Pos::new(slot, range.start),
            head: Pos::new(slot, range.end),
        });
        self.request_autoscroll();
        cx.notify();
    }

    pub(crate) fn find_step(
        &mut self,
        forward: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bar) = &mut self.find else { return };
        let n = bar.matches.len();
        if n == 0 {
            return;
        }
        bar.current = Some(match bar.current {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None => 0,
        });
        self.select_current_match(cx);
    }

    fn replace_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let Some((slot, range)) = bar.current.and_then(|i| bar.matches.get(i)).cloned() else {
            return;
        };
        let replacement = bar.replacement.read(cx).value().to_string();
        let sel = Selection {
            anchor: Pos::new(slot, range.start),
            head: Pos::new(slot, range.end),
        };
        self.edit_with(cx, move |m| {
            m.set_selection(sel);
            let rules = m.input_rules;
            m.input_rules = false;
            if replacement.is_empty() {
                m.delete_selection();
            } else {
                m.insert_text(&replacement);
            }
            m.input_rules = rules;
        });
        self.refresh_matches(false, cx);
        self.find_step(true, window, cx);
    }

    fn replace_all_matches(&mut self, cx: &mut Context<Self>) {
        let Some(bar) = &self.find else { return };
        let query = bar.query.read(cx).value().to_string();
        let replacement = bar.replacement.read(cx).value().to_string();
        let case = bar.case_sensitive;
        self.edit_with(cx, move |m| {
            m.replace_all(&query, &replacement, case);
        });
        self.refresh_matches(false, cx);
    }

    /// Ranges in `slot` to highlight, and whether one of them is the current match.
    pub(crate) fn find_highlights(&self, slot: Slot) -> (Vec<Range<usize>>, Option<Range<usize>>) {
        let Some(bar) = &self.find else {
            return (Vec::new(), None);
        };
        let current = bar
            .current
            .and_then(|i| bar.matches.get(i))
            .filter(|(s, _)| *s == slot)
            .map(|(_, r)| r.clone());
        let ranges = bar
            .matches
            .iter()
            .filter(|(s, _)| *s == slot)
            .map(|(_, r)| r.clone())
            .collect();
        (ranges, current)
    }

    pub(crate) fn render_find_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let bar = self.find.as_ref()?;
        let th = cx.theme();
        let count = match (bar.current, bar.matches.len()) {
            (_, 0) if !bar.query.read(cx).value().is_empty() => t("editor.find.no_results"),
            (_, 0) => String::new(),
            (Some(i), n) => t_args(
                "editor.find.count",
                &[("i", &(i + 1).to_string()), ("n", &n.to_string())],
            ),
            (None, n) => n.to_string(),
        };
        let show_replace = bar.show_replace;
        let case = bar.case_sensitive;
        let read_only = self.is_read_only();
        let mut row = h_flex()
            .gap(px(4.))
            .child(div().w(px(220.)).child(Input::new(&bar.query).small()))
            .child(
                div()
                    .min_w(px(70.))
                    .text_size(px(12.))
                    .text_color(th.muted_foreground)
                    .child(count),
            )
            .child(
                Button::new("find-case")
                    .xsmall()
                    .ghost()
                    .icon(IconName::CaseSensitive)
                    .selected(case)
                    .tooltip(t("editor.find.case"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(bar) = &mut this.find {
                            bar.case_sensitive = !bar.case_sensitive;
                        }
                        this.refresh_matches(true, cx);
                    })),
            )
            .child(
                Button::new("find-prev")
                    .xsmall()
                    .ghost()
                    .icon(IconName::ChevronUp)
                    .tooltip(t("editor.find.previous"))
                    .on_click(cx.listener(|this, _, w, cx| this.find_step(false, w, cx))),
            )
            .child(
                Button::new("find-next")
                    .xsmall()
                    .ghost()
                    .icon(IconName::ChevronDown)
                    .tooltip(t("editor.find.next"))
                    .on_click(cx.listener(|this, _, w, cx| this.find_step(true, w, cx))),
            );
        if !read_only {
            row = row.child(
                Button::new("find-toggle-replace")
                    .xsmall()
                    .ghost()
                    .icon(IconName::Replace)
                    .selected(show_replace)
                    .tooltip(t("editor.find.replace"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(bar) = &mut this.find {
                            bar.show_replace = !bar.show_replace;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut col = gpui_kit::component::v_flex()
            .id("find-bar")
            .gap(px(4.))
            .px(px(10.))
            .py(px(6.))
            .border_b_1()
            .border_color(th.border)
            .bg(th.muted.opacity(0.5))
            .on_key_down(
                cx.listener(|this, ev: &gpui_kit::KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.close_find(window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .child(
                h_flex().justify_between().child(row).child(
                    Button::new("find-close")
                        .xsmall()
                        .ghost()
                        .icon(IconName::Close)
                        .on_click(cx.listener(|this, _, w, cx| this.close_find(w, cx))),
                ),
            );
        if show_replace && !read_only {
            col = col.child(
                h_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .w(px(220.))
                            .child(Input::new(&bar.replacement).small()),
                    )
                    .child(
                        Button::new("replace-one")
                            .xsmall()
                            .outline()
                            .label(t("editor.find.replace_one"))
                            .on_click(cx.listener(|this, _, w, cx| this.replace_current(w, cx))),
                    )
                    .child(
                        Button::new("replace-all")
                            .xsmall()
                            .outline()
                            .label(t("editor.find.replace_all"))
                            .on_click(cx.listener(|this, _, _, cx| this.replace_all_matches(cx))),
                    ),
            );
        }
        Some(col.into_any_element())
    }
}
