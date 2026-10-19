//! Creating and editing tags.

use cardfile_core::Tag;
use cardfile_i18n::t;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Focusable as _, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::state::{AppState, run_db};

pub const TAG_COLORS: &[&str] = &[
    "#ef4444", "#f97316", "#eab308", "#22c55e", "#14b8a6", "#3b82f6", "#8b5cf6", "#ec4899",
    "#6b7280",
];

/// Emitted globally (via a notification on the window) when tags change; views reload.
pub struct TagsChanged;

impl gpui_kit::Global for TagsChanged {}

pub fn new_tag_dialog(window: &mut Window, cx: &mut App) {
    edit_tag_dialog(None, window, cx);
}

/// Create (`tag = None`) or edit a tag.
pub fn edit_tag_dialog(tag: Option<Tag>, window: &mut Window, cx: &mut App) {
    let Some(owner) = AppState::session(cx).map(|s| s.user.id) else {
        return;
    };
    let name = cx.new(|cx| {
        let mut s = InputState::new(window, cx).placeholder(t("tags.name"));
        if let Some(tag) = &tag {
            s = s.default_value(tag.name.clone());
        }
        s
    });
    let focus = name.focus_handle(cx);
    focus.focus(window, cx);
    let color = cx.new(|_| {
        tag.as_ref()
            .map_or(TAG_COLORS[5].to_owned(), |t| t.color.clone())
    });
    let editing = tag.map(|t| t.id);
    window.open_dialog(cx, move |dialog, _, cx| {
        let current = color.read(cx).clone();
        let mut swatches = h_flex().gap(px(6.));
        for (i, c) in TAG_COLORS.iter().enumerate() {
            let color = color.clone();
            let fill = cardfile_editor::parse_hex(c).unwrap_or(cx.theme().primary);
            swatches = swatches.child(
                div()
                    .id(("tag-color", i))
                    .size(px(22.))
                    .rounded_full()
                    .bg(fill)
                    .cursor_pointer()
                    .when(current == *c, |d| {
                        d.border_2().border_color(cx.theme().foreground)
                    })
                    .on_click(move |_, _, cx| {
                        color.update(cx, |v, cx| {
                            *v = (*c).to_owned();
                            cx.notify();
                        })
                    }),
            );
        }
        let name = name.clone();
        let color = color.clone();
        dialog
            .title(if editing.is_some() {
                t("tags.edit")
            } else {
                t("tags.new")
            })
            .child(
                v_flex()
                    .gap(px(10.))
                    .w(px(320.))
                    .child(Input::new(&name))
                    .child(swatches),
            )
            .footer(crate::dialogs::footer(t("common.save"), false))
            .on_ok(move |_, window, cx| {
                let value = name.read(cx).value().trim().to_string();
                if value.is_empty() {
                    return false;
                }
                let c = color.read(cx).clone();
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| match editing {
                    Some(id) => db.update_tag(id, &value, &c),
                    None => db.create_tag(owner, &value, &c).map(|_| ()),
                });
                window
                    .spawn(cx, async move |cx| {
                        let result = task.await;
                        let _ = cx.update(|window, cx| match result {
                            Ok(()) => {
                                cx.set_global(TagsChanged);
                                window.refresh();
                            }
                            Err(e) => window.push_notification(
                                gpui_kit::component::notification::Notification::error(
                                    if e.to_string().contains("UNIQUE") {
                                        t("tags.exists")
                                    } else {
                                        e.to_string()
                                    },
                                ),
                                cx,
                            ),
                        });
                    })
                    .detach();
                true
            })
    });
}
