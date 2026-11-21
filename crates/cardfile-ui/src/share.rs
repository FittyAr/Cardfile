//! Share a card with other users: no access, can view, or can edit.

use cardfile_core::{Card, CardId, CardShare, LockMode, SharePermission, User, UserId};
use cardfile_crypto::CardKey;
use cardfile_db::{Database, KeyRotation};
use cardfile_i18n::{t, t_args};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Entity, ParentElement as _, SharedString, Styled as _, Window, div, px,
};

use crate::state::{AppState, run_db};

struct State {
    users: Vec<User>,
    shares: Vec<CardShare>,
    busy: bool,
    error: Option<String>,
}

/// Open the share dialog. `key` is the card key when an encrypted card is unlocked;
/// it is needed to give someone else access.
pub fn open(
    card: Card,
    key: Option<CardKey>,
    on_change: impl Fn(&mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(me) = AppState::session(cx).map(|s| s.user.id) else {
        return;
    };
    let state = cx.new(|_| State {
        users: Vec::new(),
        shares: Vec::new(),
        busy: true,
        error: None,
    });
    reload(&state, card.id, me, cx);
    let on_change = std::rc::Rc::new(on_change);
    window.open_dialog(cx, move |dialog, _, cx| {
        let th = cx.theme();
        let st = state.read(cx);
        let mut list = v_flex().gap(px(6.)).w(px(440.));
        if card.lock_mode == LockMode::Encrypted && key.is_none() {
            list = list.child(
                div()
                    .text_size(px(12.))
                    .text_color(th.warning)
                    .child(t("share.unlock_encrypted")),
            );
        }
        let others: Vec<&User> = st.users.iter().filter(|u| u.id != me).collect();
        if others.is_empty() && !st.busy {
            list = list.child(
                div()
                    .text_color(th.muted_foreground)
                    .child(t("share.no_users")),
            );
        }
        for user in others {
            let current = st
                .shares
                .iter()
                .find(|s| s.user_id == user.id)
                .map(|s| s.permission);
            let mut buttons = h_flex().gap(px(4.));
            for (choice, label) in [
                (None, t("share.none")),
                (Some(SharePermission::View), t("share.view")),
                (Some(SharePermission::Edit), t("share.edit")),
            ] {
                let selected = current == choice;
                let (state, card, key, uid, on_change) = (
                    state.clone(),
                    card.clone(),
                    key.clone(),
                    user.id,
                    on_change.clone(),
                );
                let id = SharedString::from(format!(
                    "share-{}-{}",
                    user.id,
                    choice.map(|c| c.as_str()).unwrap_or("none")
                ));
                buttons = buttons.child(
                    Button::new(id)
                        .xsmall()
                        .label(label)
                        .when(selected, |b| b.primary())
                        .when(!selected, |b| b.outline())
                        .on_click(move |_, window, cx| {
                            if !selected {
                                set(
                                    &state,
                                    &card,
                                    key.clone(),
                                    uid,
                                    choice,
                                    on_change.clone(),
                                    window,
                                    cx,
                                );
                            }
                        }),
                );
            }
            list = list.child(
                h_flex()
                    .justify_between()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(th.muted.opacity(0.5))
                    .child(
                        v_flex()
                            .child(div().text_size(px(14.)).child(user.display_name.clone()))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(th.muted_foreground)
                                    .child(user.username.clone()),
                            ),
                    )
                    .child(buttons),
            );
        }
        if let Some(e) = &st.error {
            list = list.child(
                div()
                    .text_size(px(12.))
                    .text_color(th.danger)
                    .child(e.clone()),
            );
        }
        dialog
            .title(t_args("share.title", &[("title", &card.title)]))
            .child(list)
    });
}

fn reload(state: &Entity<State>, card: cardfile_core::CardId, _me: UserId, cx: &mut App) {
    let db = AppState::db(cx);
    let task = run_db(cx, move |_| Ok((db.list_users()?, db.card_shares(card)?)));
    let state = state.clone();
    cx.spawn(async move |cx| {
        let result = task.await;
        state.update(cx, |s, cx| {
            s.busy = false;
            match result {
                Ok((users, shares)) => {
                    s.users = users;
                    s.shares = shares;
                }
                Err(e) => s.error = Some(e.to_string()),
            }
            cx.notify();
        });
    })
    .detach();
}

#[allow(clippy::too_many_arguments)]
fn set(
    state: &Entity<State>,
    card: &Card,
    key: Option<CardKey>,
    user: UserId,
    choice: Option<SharePermission>,
    on_change: std::rc::Rc<dyn Fn(&mut App)>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(actor) = AppState::actor(cx) else {
        return;
    };
    let me = actor.user;
    let db = AppState::db(cx);
    let card = card.clone();
    let encrypted = card.lock_mode == LockMode::Encrypted;
    let task = cx.background_spawn(async move {
        match choice {
            None => {
                // The removed user may have kept the card key: move to a new one.
                let rotation = if encrypted {
                    let key = key.ok_or_else(|| anyhow::anyhow!(t("share.unlock_encrypted")))?;
                    Some(rotate_key(&db, card.id, &key, user)?)
                } else {
                    None
                };
                db.unshare_card(&actor, card.id, user, rotation.as_ref())?;
            }
            Some(permission) => {
                let wrapped = if encrypted {
                    let key = key.ok_or_else(|| anyhow::anyhow!(t("share.unlock_encrypted")))?;
                    let recipient = db.user(user)?;
                    let public = recipient.public_key.ok_or_else(|| {
                        anyhow::anyhow!(t_args(
                            "share.no_keys",
                            &[("name", &recipient.display_name)]
                        ))
                    })?;
                    Some(cardfile_crypto::wrap_card_key(
                        &key,
                        &public,
                        &card.id.to_string(),
                    )?)
                } else {
                    None
                };
                db.share_card(&actor, card.id, user, permission, wrapped.as_deref())?;
            }
        }
        anyhow::Ok(())
    });
    let state = state.clone();
    let card_id = card.id;
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| match result {
                Ok(()) => {
                    state.update(cx, |s, _| s.error = None);
                    reload(&state, card_id, me, cx);
                    on_change(cx);
                }
                Err(e) => {
                    window.push_notification(Notification::error(e.to_string()), cx);
                    state.update(cx, |s, cx| {
                        s.error = Some(e.to_string());
                        cx.notify();
                    });
                }
            });
        })
        .detach();
}

/// A new key for `card` once `removed` loses access: the content and every history
/// snapshot re-encrypted from `key`, and the new key wrapped for the owner and the
/// users who keep access. Snapshots `key` cannot open (e.g. legacy ones) are left
/// out, so the database drops them with the rotation.
fn rotate_key(
    db: &Database,
    card: CardId,
    key: &CardKey,
    removed: UserId,
) -> anyhow::Result<KeyRotation> {
    let current = db.card(card)?;
    let aad = card.to_string();
    let payload = current
        .encrypted
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!(t("card.not_unlocked")))?;
    let new = CardKey::generate()?;
    let encrypted = cardfile_crypto::reencrypt_with_key(payload, key, &new, aad.as_bytes())?;
    let revisions = db
        .card_revisions(card)?
        .into_iter()
        .filter_map(|rev| {
            let sealed = rev.encrypted.as_ref()?;
            cardfile_crypto::reencrypt_with_key(sealed, key, &new, aad.as_bytes())
                .ok()
                .map(|p| (rev.id, p))
        })
        .collect();
    let mut users = vec![current.owner_id];
    users.extend(
        db.card_shares(card)?
            .into_iter()
            .map(|s| s.user_id)
            .filter(|u| *u != removed),
    );
    let card_keys = users
        .into_iter()
        .map(|id| {
            let user = db.user(id)?;
            let public = user.public_key.as_deref().ok_or_else(|| {
                anyhow::anyhow!(t_args("share.no_keys", &[("name", &user.display_name)]))
            })?;
            anyhow::Ok((id, cardfile_crypto::wrap_card_key(&new, public, &aad)?))
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(KeyRotation {
        base_rev: current.rev,
        encrypted,
        revisions,
        card_keys,
    })
}

#[allow(dead_code)]
fn _sizable(b: Button) -> Button {
    b.small()
}
