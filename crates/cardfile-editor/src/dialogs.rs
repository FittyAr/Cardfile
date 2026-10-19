//! Small dialogs opened by the editor.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::{AppContext as _, Entity, ParentElement as _, Styled as _, Window, px};

use crate::editor::RichEditor;

/// Ask for a link URL (and its text when nothing is selected).
///
/// `current` is the link under the caret and `needs_text` whether a display text must
/// be asked for. The caller passes them in because it is usually an editor action
/// handler, and reading the editor entity while it is being updated panics.
pub fn open_link_dialog(
    editor: Entity<RichEditor>,
    current: String,
    needs_text: bool,
    window: &mut Window,
    cx: &mut gpui_kit::App,
) {
    let url = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("https://…")
            .default_value(current)
    });
    let text = cx.new(|cx| {
        InputState::new(window, cx).placeholder(cardfile_i18n::t("editor.link.text_placeholder"))
    });
    url.update(cx, |s, cx| s.focus(window, cx));
    window.open_dialog(cx, move |dialog, _window, _cx| {
        let url_input = url.clone();
        let text_input = text.clone();
        let editor = editor.clone();
        let mut body = v_flex().gap_2().w(px(380.)).child(Input::new(&url_input));
        if needs_text {
            body = body.child(Input::new(&text_input));
        }
        dialog
            .title(cardfile_i18n::t("editor.link.title"))
            .child(body)
            // A plain Dialog draws no buttons of its own; OK runs `on_ok` like Enter.
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .trigger(|button| button.label(cardfile_i18n::t("common.cancel"))),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("link-ok")
                                .primary()
                                .label(cardfile_i18n::t("common.ok")),
                        ),
                    ),
            )
            .on_ok(move |_, window, cx| {
                let u = url_input.read(cx).value().to_string();
                let t = needs_text.then(|| text_input.read(cx).value().to_string());
                editor.update(cx, |e, cx| e.apply_link(u.clone(), t.clone(), cx));
                let focus = editor.read(cx).focus_handle.clone();
                focus.focus(window, cx);
                true
            })
    });
}
