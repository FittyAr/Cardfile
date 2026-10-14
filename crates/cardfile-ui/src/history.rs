//! Version history dialog for a card.

use cardfile_core::{Card, LockMode};
use cardfile_crypto::CardKey;
use cardfile_doc::Document;
use cardfile_i18n::t;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::card_pane::CardPane;
use crate::state::{AppState, run_db};
use crate::widgets::format_date;

struct Entry {
    title: String,
    doc: Document,
    created_at: i64,
    /// Display name of whoever wrote this version.
    author: Option<String>,
}

pub fn open(
    card: Card,
    key: Option<CardKey>,
    pane: Entity<CardPane>,
    window: &mut Window,
    cx: &mut App,
) {
    let db = AppState::db(cx);
    let id = card.id;
    let lock_mode = card.lock_mode;
    let task = run_db(cx, move |_| {
        let revs = db.card_revisions(id)?;
        let mut names = std::collections::HashMap::new();
        Ok(revs
            .into_iter()
            .filter_map(|r| {
                let doc = match (&r.encrypted, lock_mode, &key) {
                    (Some(p), LockMode::Encrypted, Some(key)) => {
                        let json =
                            cardfile_crypto::decrypt_with_key(p, key, id.to_string().as_bytes())
                                .ok()?;
                        Document::from_json(&json).ok()?
                    }
                    (None, _, _) => Document::from_json(&r.content).ok()?,
                    _ => return None,
                };
                let author = r.author_id.and_then(|a| {
                    names
                        .entry(a)
                        .or_insert_with(|| db.user(a).ok().map(|u| u.display_name))
                        .clone()
                });
                Some(Entry {
                    title: r.title,
                    doc,
                    created_at: r.created_at,
                    author,
                })
            })
            .collect::<Vec<Entry>>())
    });
    window
        .spawn(cx, async move |cx| {
            let Ok(entries) = task.await else { return };
            let _ = cx.update(|window, cx| show(entries, pane, window, cx));
        })
        .detach();
}

fn show(entries: Vec<Entry>, pane: Entity<CardPane>, window: &mut Window, cx: &mut App) {
    let selected = cx.new(|_| 0usize);
    let entries = std::rc::Rc::new(entries);
    window.open_dialog(cx, move |dialog, _, cx| {
        let th = cx.theme();
        let sel = *selected.read(cx);
        let mut list = v_flex()
            .w(px(200.))
            .gap(px(2.))
            .h(px(380.))
            .overflow_hidden();
        for (i, e) in entries.iter().enumerate() {
            let selected = selected.clone();
            list = list.child(
                div()
                    .id(("rev", i))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(i == sel, |d| d.bg(th.list_active))
                    .hover(|d| d.bg(th.list_hover))
                    .on_click(move |_, _, cx| {
                        selected.update(cx, |s, cx| {
                            *s = i;
                            cx.notify();
                        })
                    })
                    .child(div().text_size(px(13.)).child(format_date(e.created_at)))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(th.muted_foreground)
                            .child(e.title.clone()),
                    )
                    .children(e.author.clone().map(|a| {
                        div()
                            .text_size(px(11.))
                            .text_color(th.muted_foreground)
                            .child(cardfile_i18n::t_args("history.by", &[("name", &a)]))
                    })),
            );
        }
        let preview = entries
            .get(sel)
            .map(|e| cardfile_doc::to_plain_text(&e.doc))
            .unwrap_or_else(|| t("history.empty"));
        let body = h_flex().gap(px(12.)).items_start().child(list).child(
            div()
                .id("rev-preview")
                .w(px(420.))
                .h(px(380.))
                .p(px(10.))
                .rounded(px(6.))
                .bg(th.muted)
                .text_size(px(13.))
                .overflow_y_scroll()
                .child(preview),
        );
        let entries2 = entries.clone();
        let pane = pane.clone();
        dialog
            .title(t("history.title"))
            .w(px(680.))
            .child(body)
            .footer(
                h_flex().justify_end().gap(px(8.)).child(
                    Button::new("restore-rev")
                        .primary()
                        .small()
                        .label(t("history.restore"))
                        .disabled_when(entries2.is_empty())
                        .on_click(move |_, window, cx| {
                            if let Some(e) = entries2.get(sel) {
                                let (title, doc) = (e.title.clone(), e.doc.clone());
                                pane.update(cx, |p, cx| p.restore_revision(title, doc, window, cx));
                            }
                            window.close_dialog(cx);
                        }),
                ),
            )
    });
}

trait DisabledWhen {
    fn disabled_when(self, v: bool) -> Self;
}

impl DisabledWhen for Button {
    fn disabled_when(self, v: bool) -> Self {
        use gpui_kit::component::Disableable as _;
        self.disabled(v)
    }
}

#[allow(dead_code)]
fn _unused(_: impl IntoElement) {}
