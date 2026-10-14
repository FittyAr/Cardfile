//! Small dialogs opened by the editor.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::{AppContext as _, Entity, ParentElement as _, Styled as _, Window, px};

use crate::editor::RichEditor;

/// Ask for a link URL (and its text when nothing is selected).
pub fn open_link_dialog(editor: Entity<RichEditor>, window: &mut Window, cx: &mut gpui_kit::App) {
    let (current, needs_text) = {
        let e = editor.read(cx);
        (
            e.link_at_caret().unwrap_or_default(),
            e.selection_is_empty() && e.link_at_caret().is_none(),
        )
    };
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
