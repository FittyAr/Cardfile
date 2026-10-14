//! The card being edited: title, editor, protection, tags, history and export.

use std::time::Duration;

use cardfile_core::{Card, CardId, LockMode, Tag, TagId, mask_title};
use cardfile_db::ContentUpdate;
use cardfile_doc::Document;
use cardfile_editor::{EditorEvent, EditorToolbar, RichEditor};
use cardfile_i18n::{t, t_args};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Task, Window, div, px,
};
use zeroize::Zeroizing;

use crate::state::{AppState, run_db};
use crate::widgets::{error, format_date};

/// Delay between the last edit and the automatic save.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(700);
/// Minimum time between two history snapshots of a card.
const SNAPSHOT_EVERY_MS: i64 = 5 * 60 * 1000;

pub enum CardPaneEvent {
    /// The card's title, content or flags changed and were saved.
    Saved,
    /// The card moved to or out of the recycle bin, or was purged.
    ListChanged,
}

/// What the pane is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    /// No card selected.
    Empty,
    /// Content visible and editable.
    Open,
    /// Read-only card not yet unlocked for editing.
    ReadOnly,
    /// Private or encrypted card waiting for the password.
    Locked,
    /// In the recycle bin.
    Deleted,
}

pub struct CardPane {
    card: Option<Card>,
    access: Access,
    tags: Vec<Tag>,
    editor: Entity<RichEditor>,
    toolbar: Entity<EditorToolbar>,
    title: Entity<InputState>,
    unlock: Entity<InputState>,
    unlock_error: Option<SharedString>,
    unlocking: bool,
    /// Password of the open encrypted card, needed to re-encrypt on save.
    card_password: Option<Zeroizing<String>>,
    saved_revision: u64,
    title_dirty: bool,
    save_task: Option<Task<()>>,
    relock_task: Option<Task<()>>,
    suppress_title_event: bool,
    _subs: Vec<Subscription>,
}

impl EventEmitter<CardPaneEvent> for CardPane {}

impl CardPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut e = RichEditor::new(Document::default(), cx);
            e.set_blob_store(crate::state::blob_store(cx));
            e.set_placeholder(Some(t("editor.placeholder")));
            e
        });
        let toolbar = cx.new(|cx| EditorToolbar::new(editor.clone(), cx));
        let title = cx.new(|cx| InputState::new(window, cx).placeholder(t("card.untitled")));
        let unlock = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t("card.unlock.placeholder"))
                .masked(true)
        });
        let subs = vec![
            cx.subscribe_in(&editor, window, Self::on_editor_event),
            cx.subscribe_in(
                &title,
                window,
                |this, _, ev: &InputEvent, window, cx| match ev {
                    InputEvent::Change if !this.suppress_title_event => {
                        this.title_dirty = true;
                        this.schedule_save(cx);
                    }
                    InputEvent::PressEnter { .. } => {
                        let focus = this.editor.focus_handle(cx);
                        focus.focus(window, cx);
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(&unlock, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.try_unlock(window, cx);
                }
            }),
        ];
        let mut pane = Self {
            card: None,
            access: Access::Empty,
            tags: Vec::new(),
            editor,
            toolbar,
            title,
            unlock,
            unlock_error: None,
            unlocking: false,
            card_password: None,
            saved_revision: 0,
            title_dirty: false,
            save_task: None,
            relock_task: None,
            suppress_title_event: false,
            _subs: subs,
        };
        pane.apply_user_settings(cx);
        pane
    }

    pub fn card_id(&self) -> Option<CardId> {
        self.card.as_ref().map(|c| c.id)
    }

    pub fn card(&self) -> Option<&Card> {
        self.card.as_ref()
    }

    pub fn set_tags(&mut self, tags: Vec<Tag>, cx: &mut Context<Self>) {
        self.tags = tags;
        cx.notify();
    }

    pub fn apply_user_settings(&mut self, cx: &mut Context<Self>) {
        let s = AppState::user_settings(cx);
        let spell = crate::spell::checker(&s, cx);
        self.editor.update(cx, |e, cx| {
            e.set_font_size(px(s.editor_font_size), cx);
            e.set_input_rules(s.markdown_shortcuts);
            e.set_spell_checker(spell, cx);
        });
    }

    /// Show `card` (or nothing). Saves the previous card first.
    pub fn open(&mut self, card: Option<Card>, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_save(cx);
        self.card_password = None;
        self.relock_task = None;
        self.unlock_error = None;
        self.unlock.update(cx, |s, cx| s.set_value("", window, cx));
        self.card = card;
        let Some(card) = self.card.clone() else {
            self.access = Access::Empty;
            self.set_editor_doc(Document::default(), true, cx);
            cx.notify();
            return;
        };
        self.access = if card.is_deleted() {
            Access::Deleted
        } else {
            match card.lock_mode {
                LockMode::None => Access::Open,
                LockMode::ReadOnly => Access::ReadOnly,
                LockMode::Private | LockMode::Encrypted => Access::Locked,
            }
        };
        self.set_title(&card.title, window, cx);
        match self.access {
            Access::Locked => {
                self.set_editor_doc(Document::default(), true, cx);
                let focus = self.unlock.focus_handle(cx);
                focus.focus(window, cx);
            }
            _ => {
                let doc = Document::from_json(&card.content).unwrap_or_else(|e| {
                    tracing::error!("card {} has invalid content: {e}", card.id);
                    Document::default()
                });
                self.set_editor_doc(doc, self.access != Access::Open, cx);
            }
        }
        cx.notify();
    }

    fn set_title(&mut self, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        let shown = match (&self.card, self.access) {
            (Some(c), Access::Locked) if c.lock_mode == LockMode::Private => {
                mask_title(title, AppState::user_settings(cx).mask_visible_chars)
            }
            _ => title.to_owned(),
        };
        self.suppress_title_event = true;
        self.title.update(cx, |s, cx| {
            s.set_value(shown, window, cx);
            s.set_disabled(self.access != Access::Open, cx);
        });
        self.suppress_title_event = false;
        self.title_dirty = false;
    }

    fn set_editor_doc(&mut self, doc: Document, read_only: bool, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| {
            e.set_document(doc, cx);
            e.set_read_only(read_only, cx);
        });
        self.saved_revision = self.editor.read(cx).revision();
    }

    fn on_editor_event(
        &mut self,
        _: &Entity<RichEditor>,
        ev: &EditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ev {
            EditorEvent::Changed => {
                self.schedule_save(cx);
                self.bump_relock(cx);
            }
            EditorEvent::SelectionChanged => self.bump_relock(cx),
            EditorEvent::EditBlocked => {
                if self.access == Access::ReadOnly {
                    window.push_notification(Notification::info(t("editor.read_only")), cx);
                }
            }
            EditorEvent::OpenLink(url) => cx.open_url(url),
            EditorEvent::OpenAttachment { blob, file_name } => {
                crate::export::open_attachment(blob.clone(), file_name.clone(), window, cx)
            }
        }
    }

    // ---------------------------------------------------------------------
    // Saving
    // ---------------------------------------------------------------------

    fn is_dirty(&self, cx: &Context<Self>) -> bool {
        self.title_dirty || self.editor.read(cx).revision() != self.saved_revision
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        if self.access != Access::Open {
            return;
        }
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| this.save(cx));
        }));
    }

    /// Save immediately if there are pending changes (e.g. before switching cards).
    pub fn flush_save(&mut self, cx: &mut Context<Self>) {
        if self.save_task.take().is_some() || self.is_dirty(cx) {
            self.save(cx);
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        let Some(card) = self.card.as_ref() else {
            return;
        };
        if self.access != Access::Open || !self.is_dirty(cx) {
            return;
        }
        let id = card.id;
        let encrypted = card.lock_mode == LockMode::Encrypted;
        let doc = self.editor.read(cx).document().clone();
        let title = self.title.read(cx).value().trim().to_string();
        let title = if title.is_empty() {
            t("card.untitled")
        } else {
            title
        };
        let revision = self.editor.read(cx).revision();
        let password = self.card_password.clone();
        let keep = AppState::user_settings(cx).max_revisions;
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            let json = doc.to_json();
            let update = if encrypted {
                let password =
                    password.ok_or_else(|| anyhow::anyhow!("encrypted card is not unlocked"))?;
                let payload =
                    cardfile_crypto::encrypt(&json, &password, id.to_string().as_bytes())?;
                ContentUpdate {
                    title,
                    encrypted: Some(payload),
                    blob_ids: doc.blob_ids(),
                    ..Default::default()
                }
            } else {
                ContentUpdate {
                    title,
                    plain_text: cardfile_doc::to_plain_text(&doc),
                    content: json,
                    encrypted: None,
                    blob_ids: doc.blob_ids(),
                }
            };
            anyhow::Ok(db.update_card_content(id, update, keep, SNAPSHOT_EVERY_MS)?)
        });
        self.saved_revision = revision;
        self.title_dirty = false;
        cx.spawn(async move |this, cx| {
            match task.await {
                Ok(saved) => {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(card) = this.card.as_mut().filter(|c| c.id == saved.id) {
                            // Keep the decrypted content out of the cached card.
                            card.title = saved.title;
                            card.updated_at = saved.updated_at;
                            card.rev = saved.rev;
                            if saved.lock_mode != LockMode::Encrypted {
                                card.content = saved.content;
                                card.plain_text = saved.plain_text;
                            }
                        }
                        cx.emit(CardPaneEvent::Saved);
                        cx.notify();
                    });
                }
                Err(e) => tracing::error!("save card: {e:#}"),
            }
        })
        .detach();
    }

    // ---------------------------------------------------------------------
    // Protection
    // ---------------------------------------------------------------------

    fn try_unlock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if self.access != Access::Locked || self.unlocking {
            return;
        }
        let password = Zeroizing::new(self.unlock.read(cx).value().to_string());
        let Some(hash) = AppState::lock_password_hash(cx) else {
            self.unlock_error = Some(t("card.unlock.no_password").into());
            cx.notify();
            return;
        };
        self.unlocking = true;
        self.unlock_error = None;
        cx.notify();
        let task = cx.background_spawn(async move {
            if !cardfile_crypto::verify_password(&hash, &password) {
                return Err(anyhow::anyhow!(t("card.unlock.wrong")));
            }
            let doc = match &card.encrypted {
                Some(payload) if card.lock_mode == LockMode::Encrypted => {
                    let json = cardfile_crypto::decrypt(
                        payload,
                        &password,
                        card.id.to_string().as_bytes(),
                    )?;
                    Document::from_json(&json)?
                }
                _ => Document::from_json(&card.content)?,
            };
            anyhow::Ok((doc, password))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.unlocking = false;
                match result {
                    Ok((doc, password)) => {
                        this.card_password = Some(password);
                        this.access = Access::Open;
                        let title = this
                            .card
                            .as_ref()
                            .map(|c| c.title.clone())
                            .unwrap_or_default();
                        this.set_title(&title, window, cx);
                        this.set_editor_doc(doc, false, cx);
                        this.unlock.update(cx, |s, cx| s.set_value("", window, cx));
                        let focus = this.editor.focus_handle(cx);
                        focus.focus(window, cx);
                        this.bump_relock(cx);
                    }
                    Err(e) => this.unlock_error = Some(e.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Restart the inactivity timer that hides an unlocked private/encrypted card.
    fn bump_relock(&mut self, cx: &mut Context<Self>) {
        let hides = self
            .card
            .as_ref()
            .is_some_and(|c| c.lock_mode.hides_content());
        let secs = AppState::user_settings(cx).relock_card_secs;
        if !hides || secs == 0 || self.access != Access::Open {
            return;
        }
        self.relock_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_secs(u64::from(secs)))
                .await;
            let _ = this.update(cx, |this, cx| this.relock(cx));
        }));
    }

    /// Hide the content of a private or encrypted card again.
    pub fn relock(&mut self, cx: &mut Context<Self>) {
        let hides = self
            .card
            .as_ref()
            .is_some_and(|c| c.lock_mode.hides_content());
        if !hides || self.access != Access::Open {
            return;
        }
        self.flush_save(cx);
        self.card_password = None;
        self.access = Access::Locked;
        self.set_editor_doc(Document::default(), true, cx);
        cx.notify();
    }

    /// Allow editing a read-only card until another card is opened.
    fn unlock_read_only(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.access != Access::ReadOnly {
            return;
        }
        self.access = Access::Open;
        let title = self
            .card
            .as_ref()
            .map(|c| c.title.clone())
            .unwrap_or_default();
        self.set_title(&title, window, cx);
        self.editor.update(cx, |e, cx| e.set_read_only(false, cx));
        let focus = self.editor.focus_handle(cx);
        focus.focus(window, cx);
        cx.notify();
    }

    fn set_lock_mode(&mut self, mode: LockMode, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if card.lock_mode == mode {
            return;
        }
        // A locked Private or Encrypted card must be unlocked first, or changing the
        // mode would reveal it without the password.
        if card.lock_mode.hides_content() && self.access != Access::Open {
            window.push_notification(Notification::warning(t("card.protect.unlock_first")), cx);
            return;
        }
        if mode.hides_content() && AppState::lock_password_hash(cx).is_none() {
            window.push_notification(Notification::warning(t("card.protect.needs_password")), cx);
            return;
        }
        if mode == LockMode::Encrypted && self.card_password.is_none() {
            crate::dialogs::ask_password(
                t("card.protect.encrypt_title"),
                t("card.protect.encrypt_body"),
                cx.entity(),
                |this, password, window, cx| {
                    this.card_password = Some(password);
                    this.set_lock_mode(LockMode::Encrypted, window, cx);
                },
                window,
                cx,
            );
            return;
        }
        self.flush_save(cx);
        let doc = self.editor.read(cx).document().clone();
        let password = self.card_password.clone();
        let db = AppState::db(cx);
        let content_doc = doc;
        let task = cx.background_spawn(async move {
            let json = content_doc.to_json();
            let plain = cardfile_doc::to_plain_text(&content_doc);
            let card = if mode == LockMode::Encrypted {
                let pw = password.ok_or_else(|| anyhow::anyhow!("missing password"))?;
                let payload = cardfile_crypto::encrypt(&json, &pw, card.id.to_string().as_bytes())?;
                db.set_card_lock(card.id, mode, "", "", Some(&payload))?
            } else {
                db.set_card_lock(card.id, mode, &json, &plain, None)?
            };
            anyhow::Ok(card)
        });
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok(saved) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    let keep_open = this.access == Access::Open;
                    let password = this.card_password.clone();
                    let doc = this.editor.read(cx).document().clone();
                    this.card = Some(saved);
                    if keep_open {
                        // Stay unlocked after changing protection.
                        this.access = Access::Open;
                        this.card_password = password;
                        this.editor.update(cx, |e, cx| e.set_read_only(false, cx));
                        let _ = doc;
                    } else {
                        let card = this.card.clone();
                        this.open(card, window, cx);
                    }
                    cx.emit(CardPaneEvent::Saved);
                    cx.notify();
                });
            }
            Err(e) => tracing::error!("change protection: {e:#}"),
        })
        .detach();
    }

    // ---------------------------------------------------------------------
    // Flags, tags, recycle bin
    // ---------------------------------------------------------------------

    pub fn toggle_pin(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self.card.as_mut() else {
            return;
        };
        card.is_pinned = !card.is_pinned;
        let (id, v) = (card.id, card.is_pinned);
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.set_card_pinned(id, v));
        self.after_flag(task, cx);
    }

    pub fn toggle_favorite(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self.card.as_mut() else {
            return;
        };
        card.is_favorite = !card.is_favorite;
        let (id, v) = (card.id, card.is_favorite);
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.set_card_favorite(id, v));
        self.after_flag(task, cx);
    }

    fn after_flag(&mut self, task: Task<anyhow::Result<()>>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            if task.await.is_ok() {
                let _ = this.update(cx, |_, cx| cx.emit(CardPaneEvent::Saved));
            }
        })
        .detach();
        cx.notify();
    }

    fn toggle_tag(&mut self, tag: TagId, cx: &mut Context<Self>) {
        let Some(card) = self.card.as_mut() else {
            return;
        };
        if let Some(pos) = card.tags.iter().position(|t| *t == tag) {
            card.tags.remove(pos);
        } else {
            card.tags.push(tag);
        }
        let (id, tags) = (card.id, card.tags.clone());
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.set_card_tags(id, &tags));
        self.after_flag(task, cx);
    }

    /// Move the card to the recycle bin.
    pub fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if card.is_deleted() {
            self.purge(window, cx);
            return;
        }
        if AppState::user_settings(cx).confirm_delete {
            let pane = cx.entity();
            crate::dialogs::confirm(
                t("card.delete"),
                t_args("card.delete_body", &[("title", &card.title)]),
                t("card.delete"),
                window,
                cx,
                move |window, cx| pane.update(cx, |p, cx| p.delete_now(window, cx)),
            );
            return;
        }
        self.delete_now(window, cx);
    }

    fn delete_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        self.flush_save(cx);
        let db = AppState::db(cx);
        let id = card.id;
        let task = run_db(cx, move |_| db.delete_card(id));
        cx.spawn_in(window, async move |this, cx| {
            if task.await.is_ok() {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.open(None, window, cx);
                    cx.emit(CardPaneEvent::ListChanged);
                });
            }
        })
        .detach();
    }

    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.card_id() else { return };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| {
            db.restore_card(id)?;
            db.card(id)
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(card) = task.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.open(Some(card), window, cx);
                    cx.emit(CardPaneEvent::ListChanged);
                });
            }
        })
        .detach();
    }

    fn purge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.card_id() else { return };
        let pane = cx.entity();
        crate::dialogs::confirm(
            t("card.purge.title"),
            t("card.purge.body"),
            t("card.purge.confirm"),
            window,
            cx,
            move |window, cx| {
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| db.purge_card(id));
                let pane = pane.clone();
                window
                    .spawn(cx, async move |cx| {
                        if task.await.is_ok() {
                            let _ = pane.update_in(cx, |this, window, cx| {
                                this.open(None, window, cx);
                                cx.emit(CardPaneEvent::ListChanged);
                            });
                        }
                    })
                    .detach();
            },
        );
    }

    pub fn show_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if self.access != Access::Open && self.access != Access::ReadOnly {
            return;
        }
        crate::history::open(card, self.card_password.clone(), cx.entity(), window, cx);
    }

    /// Replace the content with a revision (called by the history dialog).
    pub fn restore_revision(
        &mut self,
        title: String,
        doc: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.access == Access::ReadOnly {
            self.unlock_read_only(window, cx);
        }
        self.editor.update(cx, |e, cx| {
            e.set_document(doc, cx);
        });
        // A fresh document resets the editor revision; force a save.
        self.saved_revision = u64::MAX;
        self.set_title_value(&title, window, cx);
        self.save(cx);
    }

    fn set_title_value(&mut self, title: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.suppress_title_event = true;
        self.title
            .update(cx, |s, cx| s.set_value(title.to_owned(), window, cx));
        self.suppress_title_event = false;
        self.title_dirty = true;
    }

    pub fn export(
        &mut self,
        format: crate::export::Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.card_id() else { return };
        if self.access == Access::Locked {
            window.push_notification(Notification::warning(t("card.protect.unlock_first")), cx);
            return;
        }
        self.flush_save(cx);
        let title = self.title.read(cx).value().to_string();
        let doc = self.editor.read(cx).document().clone();
        crate::export::export_card(id, title, doc, format, window, cx);
    }

    // ---------------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------------

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let card = self.card.as_ref();
        let pinned = card.is_some_and(|c| c.is_pinned);
        let favorite = card.is_some_and(|c| c.is_favorite);
        let mode = card.map(|c| c.lock_mode).unwrap_or_default();
        let enabled = AppState::get(cx).app.enabled_lock_modes.clone();
        let deleted = self.access == Access::Deleted;
        let pane = cx.entity();
        let lock_icon = match mode {
            LockMode::None => IconName::LockOpen,
            _ => IconName::Lock,
        };
        let protect = {
            let pane = pane.clone();
            Button::new("protection")
                .icon(lock_icon)
                .label(t(&format!("lock.{}", mode.as_str())))
                .ghost()
                .small()
                .dropdown_caret(true)
                .tooltip(t("card.protect.tooltip"))
                .dropdown_menu(move |mut menu, _, _| {
                    for m in LockMode::ALL {
                        if m != LockMode::None && !enabled.contains(&m) {
                            continue;
                        }
                        let pane = pane.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(&format!("lock.{}", m.as_str())))
                                .checked(m == mode)
                                .on_click(move |_, window, cx| {
                                    pane.update(cx, |p, cx| p.set_lock_mode(m, window, cx))
                                }),
                        );
                    }
                    menu
                })
        };
        let tags_menu = {
            let pane = pane.clone();
            let all = self.tags.clone();
            let selected = card.map(|c| c.tags.clone()).unwrap_or_default();
            Button::new("tags")
                .icon(IconName::Tag)
                .ghost()
                .small()
                .tooltip(t("card.tags"))
                .dropdown_menu(move |mut menu, _, _| {
                    for tag in &all {
                        let pane = pane.clone();
                        let id = tag.id;
                        menu = menu.item(
                            PopupMenuItem::new(tag.name.clone())
                                .checked(selected.contains(&id))
                                .on_click(move |_, _, cx| {
                                    pane.update(cx, |p, cx| p.toggle_tag(id, cx))
                                }),
                        );
                    }
                    if !all.is_empty() {
                        menu = menu.separator();
                    }
                    menu.item(
                        PopupMenuItem::new(t("tags.new"))
                            .on_click(|_, window, cx| crate::tags::new_tag_dialog(window, cx)),
                    )
                })
        };
        let more = {
            let pane = pane.clone();
            Button::new("card-more")
                .icon(IconName::Ellipsis)
                .ghost()
                .small()
                .dropdown_menu(move |menu, _, _| {
                    let p1 = pane.clone();
                    let p2 = pane.clone();
                    let p3 = pane.clone();
                    let p4 = pane.clone();
                    let p5 = pane.clone();
                    let p6 = pane.clone();
                    menu.item(
                        PopupMenuItem::new(t("card.history"))
                            .on_click(move |_, w, cx| p1.update(cx, |p, cx| p.show_history(w, cx))),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new(t("export.markdown")).on_click(move |_, w, cx| {
                            p2.update(cx, |p, cx| p.export(crate::export::Format::Markdown, w, cx))
                        }),
                    )
                    .item(
                        PopupMenuItem::new(t("export.html")).on_click(move |_, w, cx| {
                            p3.update(cx, |p, cx| p.export(crate::export::Format::Html, w, cx))
                        }),
                    )
                    .item(
                        PopupMenuItem::new(t("export.pdf")).on_click(move |_, w, cx| {
                            p4.update(cx, |p, cx| p.export(crate::export::Format::Pdf, w, cx))
                        }),
                    )
                    .item(
                        PopupMenuItem::new(t("export.json")).on_click(move |_, w, cx| {
                            p5.update(cx, |p, cx| p.export(crate::export::Format::Json, w, cx))
                        }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new(t("card.delete"))
                            .on_click(move |_, w, cx| p6.update(cx, |p, cx| p.delete(w, cx))),
                    )
                })
        };
        let th = cx.theme();
        h_flex()
            .w_full()
            .gap(px(4.))
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(th.border)
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.title)
                        .appearance(false)
                        .text_size(px(20.))
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD),
                ),
            )
            .when(!deleted, |d| {
                d.child(
                    Button::new("pin")
                        .icon(if pinned {
                            IconName::PinOff
                        } else {
                            IconName::Pin
                        })
                        .ghost()
                        .small()
                        .tooltip(if pinned {
                            t("card.unpin")
                        } else {
                            t("card.pin")
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_pin(cx))),
                )
                .child(
                    Button::new("favorite")
                        .icon(IconName::Star)
                        .ghost()
                        .small()
                        .when(favorite, |b| b.text_color(th.warning))
                        .tooltip(if favorite {
                            t("card.unfavorite")
                        } else {
                            t("card.favorite")
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_favorite(cx))),
                )
                .child(tags_menu)
                .child(protect)
                .child(more)
            })
            .into_any_element()
    }

    fn render_tags(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let card = self.card.as_ref()?;
        if card.tags.is_empty() {
            return None;
        }
        let mut row = h_flex().gap(px(6.)).px(px(16.)).pt(px(6.)).flex_wrap();
        for tag in self.tags.iter().filter(|t| card.tags.contains(&t.id)) {
            let color = cardfile_editor::parse_hex(&tag.color).unwrap_or(cx.theme().primary);
            row = row.child(
                h_flex()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(1.))
                    .rounded_full()
                    .bg(color.opacity(0.15))
                    .text_size(px(12.))
                    .child(div().size(px(7.)).rounded_full().bg(color))
                    .child(tag.name.clone()),
            );
        }
        Some(row.into_any_element())
    }

    fn render_locked(&self, cx: &mut Context<Self>) -> AnyElement {
        let card = self.card.as_ref();
        let encrypted = card.is_some_and(|c| c.lock_mode == LockMode::Encrypted);
        let th = cx.theme();
        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .w(px(320.))
                    .gap(px(12.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(40.))
                            .child(if encrypted { "🔐" } else { "🔒" }),
                    )
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(if encrypted {
                                t("card.unlock.encrypted")
                            } else {
                                t("card.unlock.private")
                            }),
                    )
                    .child(div().w_full().child(Input::new(&self.unlock).mask_toggle()))
                    .child(error(self.unlock_error.as_ref(), cx))
                    .child(
                        Button::new("unlock-card")
                            .w_full()
                            .primary()
                            .loading(self.unlocking)
                            .label(t("card.unlock.button"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.try_unlock(window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(th.muted_foreground)
                            .child(t("card.unlock.hint")),
                    ),
            )
            .into_any_element()
    }

    fn render_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let th = cx.theme();
        match self.access {
            Access::ReadOnly => {
                Some(
                    h_flex()
                        .justify_between()
                        .px(px(16.))
                        .py(px(6.))
                        .bg(th.warning.opacity(0.12))
                        .text_size(px(13.))
                        .child(t("card.read_only_banner"))
                        .child(
                            Button::new("unlock-ro")
                                .small()
                                .icon(IconName::LockOpen)
                                .label(t("card.unlock_edit"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.unlock_read_only(window, cx)
                                })),
                        )
                        .into_any_element(),
                )
            }
            Access::Deleted => Some(
                h_flex()
                    .justify_between()
                    .px(px(16.))
                    .py(px(6.))
                    .bg(th.danger.opacity(0.12))
                    .text_size(px(13.))
                    .child(t("card.deleted_banner"))
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(
                                Button::new("restore")
                                    .small()
                                    .label(t("card.restore"))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.restore(window, cx)),
                                    ),
                            )
                            .child(
                                Button::new("purge")
                                    .small()
                                    .danger()
                                    .label(t("card.purge.confirm"))
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.purge(window, cx)),
                                    ),
                            ),
                    )
                    .into_any_element(),
            ),
            _ => None,
        }
    }

    fn render_status(&self, cx: &Context<Self>) -> AnyElement {
        let th = cx.theme();
        let card = self.card.as_ref();
        let words = if self.access == Access::Open || self.access == Access::ReadOnly {
            let text = cardfile_doc::to_plain_text(self.editor.read(cx).document());
            text.split_whitespace().count()
        } else {
            0
        };
        let saving = self.save_task.is_some();
        h_flex()
            .justify_between()
            .px(px(16.))
            .py(px(4.))
            .border_t_1()
            .border_color(th.border)
            .text_size(px(12.))
            .text_color(th.muted_foreground)
            .child(t_args("card.words", &[("n", &words.to_string())]))
            .child(match card {
                Some(_) if saving => t("card.saving"),
                Some(c) => t_args("card.updated", &[("date", &format_date(c.updated_at))]),
                None => String::new(),
            })
            .into_any_element()
    }
}

impl Render for CardPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.access == Access::Empty {
            let th = cx.theme();
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(th.background)
                .text_color(th.muted_foreground)
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_size(px(40.)).child("🗂"))
                        .child(t("card.none_selected"))
                        .child(div().text_size(px(12.)).child(t("card.none_hint"))),
                )
                .into_any_element();
        }
        let header = self.render_header(cx);
        let tags = self.render_tags(cx);
        let banner = self.render_banner(cx);
        let body = if self.access == Access::Locked {
            self.render_locked(cx).into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .when(self.access == Access::Open, |d| {
                    d.child(self.toolbar.clone())
                })
                .child(div().flex_1().min_h_0().child(self.editor.clone()))
                .into_any_element()
        };
        let status = self.render_status(cx);
        v_flex()
            .id("card-pane")
            .size_full()
            .bg(cx.theme().background)
            .child(header)
            .children(tags)
            .children(banner)
            .child(body)
            .child(status)
            .into_any_element()
    }
}
