//! Reusable dialogs: confirmation and password prompts.

use cardfile_i18n::t;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogButtonProps, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, v_flex};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, ParentElement as _, Styled as _, Window,
    div, px,
};
use zeroize::Zeroizing;

use crate::state::AppState;

/// Cancel and OK buttons for a dialog. A plain gpui-component `Dialog` draws no
/// buttons from `button_props` (those only back Enter and Escape), so every
/// dialog needs this footer. OK dispatches the dialog's confirm action, so it runs
/// the same `on_ok` as Enter.
pub fn footer(ok_label: String, danger: bool) -> DialogFooter {
    let ok = Button::new("dialog-ok").label(ok_label);
    let ok = if danger { ok.danger() } else { ok.primary() };
    DialogFooter::new()
        .child(DialogClose::new().trigger(|button| button.label(t("common.cancel"))))
        .child(DialogAction::new().child(ok))
}

/// Ask for confirmation of a destructive action; `on_confirm` runs when the user
/// accepts.
pub fn confirm(
    title: String,
    body: String,
    ok_label: String,
    window: &mut Window,
    cx: &mut App,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
) {
    let on_confirm = std::rc::Rc::new(on_confirm);
    window.open_dialog(cx, move |dialog, _, _| {
        let f = on_confirm.clone();
        dialog
            .title(title.clone())
            .child(div().max_w(px(420.)).child(body.clone()))
            .button_props(
                DialogButtonProps::default()
                    .ok_text(ok_label.clone())
                    .cancel_text(t("common.cancel"))
                    .show_cancel(true),
            )
            .footer(footer(ok_label.clone(), true))
            .on_ok(move |_, window, cx| {
                f(window, cx);
                true
            })
    });
}

/// Ask for the card lock password and verify it before calling `on_ok` with it.
pub fn ask_password<V: 'static>(
    title: String,
    body: String,
    view: Entity<V>,
    on_ok: impl Fn(&mut V, Zeroizing<String>, &mut Window, &mut Context<V>) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .masked(true)
            .placeholder(t("card.unlock.placeholder"))
    });
    let focus = input.focus_handle(cx);
    focus.focus(window, cx);
    let error = cx.new(|_| None::<String>);
    let on_ok = std::rc::Rc::new(on_ok);
    window.open_dialog(cx, move |dialog, _, cx| {
        let input = input.clone();
        let view = view.clone();
        let on_ok = on_ok.clone();
        let error_state = error.clone();
        let err = error.read(cx).clone();
        dialog
            .title(title.clone())
            .child(
                v_flex()
                    .gap(px(8.))
                    .w(px(360.))
                    .child(body.clone())
                    .child(Input::new(&input).mask_toggle())
                    .children(err.map(|e| div().text_color(gpui_kit::red()).child(e))),
            )
            .button_props(
                DialogButtonProps::default()
                    .ok_text(t("common.ok"))
                    .cancel_text(t("common.cancel"))
                    .show_cancel(true),
            )
            .footer(footer(t("common.ok"), false))
            .on_ok(move |_, window, cx| {
                let pw = Zeroizing::new(input.read(cx).value().to_string());
                let Some(hash) = AppState::lock_password_hash(cx) else {
                    return true;
                };
                // Argon2 takes hundreds of milliseconds: verify in the background and
                // close the dialog only once the password is known to be right.
                let check = {
                    let pw = pw.clone();
                    cx.background_spawn(async move { cardfile_crypto::verify_password(&hash, &pw) })
                };
                let f = on_ok.clone();
                let view = view.clone();
                let error_state = error_state.clone();
                window
                    .spawn(cx, async move |cx| {
                        let ok = check.await;
                        let _ = cx.update(|window, cx| {
                            if ok {
                                window.close_dialog(cx);
                                view.update(cx, |v, cx| f(v, pw, window, cx));
                            } else {
                                error_state.update(cx, |e, cx| {
                                    *e = Some(t("card.unlock.wrong"));
                                    cx.notify();
                                });
                            }
                        });
                    })
                    .detach();
                false
            })
    });
}
