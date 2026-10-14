//! Command palette: search and run any application or editor command.

use cardfile_i18n::t;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Action, App, AppContext as _, Focusable as _, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::keymap::*;

fn commands(cx: &App) -> Vec<(String, Box<dyn Action>)> {
    use cardfile_editor::editor as ed;
    let mut v: Vec<(String, Box<dyn Action>)> = vec![
        (t("card.new"), Box::new(NewCard)),
        (t("card.delete"), Box::new(DeleteCard)),
        (t("card.pin"), Box::new(TogglePin)),
        (t("card.favorite"), Box::new(ToggleFavorite)),
        (t("card.history"), Box::new(ShowHistory)),
        (t("export.markdown"), Box::new(ExportCard)),
        (t("menu.export_all"), Box::new(ExportAll)),
        (t("nav.all"), Box::new(ShowAllCards)),
        (t("nav.favorites"), Box::new(ShowFavorites)),
        (t("nav.recycle_bin"), Box::new(ShowRecycleBin)),
        (t("recycle.empty"), Box::new(EmptyRecycleBin)),
        (t("search.focus"), Box::new(FocusSearch)),
        (t("menu.settings"), Box::new(OpenSettings)),
        (t("menu.keymap"), Box::new(OpenKeymap)),
        (t("menu.backup"), Box::new(BackupNow)),
        (t("menu.open_data_folder"), Box::new(OpenDataFolder)),
        (t("menu.check_updates"), Box::new(CheckForUpdates)),
        (t("menu.about"), Box::new(About)),
        (t("menu.lock"), Box::new(LockApp)),
        (t("menu.sign_out"), Box::new(SignOut)),
        (t("menu.quit"), Box::new(Quit)),
        (t("nav.shared"), Box::new(ShowShared)),
        (t("share.button"), Box::new(ShareCard)),
    ];
    if crate::ai_settings::active_provider(cx).is_some() {
        v.push((t("chat.toggle"), Box::new(ToggleAssistant)));
    }
    let editor: Vec<(&str, Box<dyn Action>)> = vec![
        ("editor.toolbar.heading1", Box::new(ed::Heading1)),
        ("editor.toolbar.heading2", Box::new(ed::Heading2)),
        ("editor.toolbar.heading3", Box::new(ed::Heading3)),
        ("editor.toolbar.paragraph", Box::new(ed::Paragraph)),
        ("editor.toolbar.bullet_list", Box::new(ed::BulletList)),
        ("editor.toolbar.numbered_list", Box::new(ed::NumberedList)),
        ("editor.toolbar.check_list", Box::new(ed::CheckList)),
        ("editor.toolbar.quote", Box::new(ed::Quote)),
        ("editor.toolbar.code_block", Box::new(ed::CodeBlock)),
        ("editor.toolbar.table", Box::new(ed::InsertTable)),
        ("editor.toolbar.divider", Box::new(ed::InsertDivider)),
        ("editor.toolbar.image", Box::new(ed::InsertImage)),
        ("editor.toolbar.attachment", Box::new(ed::InsertAttachment)),
        ("editor.toolbar.link", Box::new(ed::InsertLink)),
        (
            "editor.toolbar.clear_formatting",
            Box::new(ed::ClearFormatting),
        ),
    ];
    v.extend(editor.into_iter().map(|(k, a)| (t(k), a)));
    v
}

pub fn open(window: &mut Window, cx: &mut App) {
    // Remember where focus was so editor commands reach the editor.
    let previous = window.focused(cx);
    let query = cx.new(|cx| InputState::new(window, cx).placeholder(t("palette.placeholder")));
    let focus = query.focus_handle(cx);
    focus.focus(window, cx);
    let selected = cx.new(|_| 0usize);
    let run = std::rc::Rc::new(
        move |action: Box<dyn Action>, window: &mut Window, cx: &mut App| {
            window.close_dialog(cx);
            match &previous {
                Some(f) => f.dispatch_action(action.as_ref(), window, cx),
                None => window.dispatch_action(action, cx),
            }
        },
    );
    let sub = {
        let run = run.clone();
        let selected = selected.clone();
        let handle = window.window_handle();
        cx.subscribe(&query, move |state, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let q = state.read(cx).value().to_lowercase();
                let ix = *selected.read(cx);
                if let Some((_, a)) = commands(cx)
                    .into_iter()
                    .filter(|(n, _)| n.to_lowercase().contains(&q))
                    .nth(ix)
                {
                    let run = run.clone();
                    let _ = handle.update(cx, |_, window, cx| run(a, window, cx));
                }
            } else if let InputEvent::Change = ev {
                selected.update(cx, |s, cx| {
                    *s = 0;
                    cx.notify();
                });
            }
        })
    };
    let sub = std::rc::Rc::new(sub);
    window.open_dialog(cx, move |dialog, _, cx| {
        let _keep = sub.clone();
        let q = query.read(cx).value().to_lowercase();
        let sel = *selected.read(cx);
        let th = cx.theme();
        let mut list = v_flex().gap(px(1.)).max_h(px(360.)).overflow_hidden();
        for (i, (name, action)) in commands(cx)
            .into_iter()
            .filter(|(n, _)| n.to_lowercase().contains(&q))
            .take(14)
            .enumerate()
        {
            let run = run.clone();
            let action = std::cell::RefCell::new(Some(action));
            list = list.child(
                div()
                    .id(("cmd", i))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(i == sel, |d| d.bg(th.list_active))
                    .hover(|d| d.bg(th.list_hover))
                    .on_click(move |_, window, cx| {
                        if let Some(a) = action.borrow_mut().take() {
                            run(a, window, cx);
                        }
                    })
                    .child(name),
            );
        }
        dialog
            .w(px(520.))
            .child(v_flex().gap(px(8.)).child(Input::new(&query)).child(list))
    });
}
