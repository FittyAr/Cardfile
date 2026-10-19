//! The card being edited: title, editor, protection, tags, history and export.

use std::sync::Arc;
use std::time::Duration;

use cardfile_core::{BlobRef, Card, CardAccess, CardId, LockMode, Tag, TagId, mask_title};
use cardfile_crypto::CardKey;
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

use crate::state::{AppState, DbBlobs, run_db};
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
    /// The editor's blob store; seals new images while an encrypted card is open.
    blobs: Arc<DbBlobs>,
    toolbar: Entity<EditorToolbar>,
    /// One-line editor (it lays out right-to-left titles correctly).
    title: Entity<RichEditor>,
    unlock: Entity<InputState>,
    unlock_error: Option<SharedString>,
    unlocking: bool,
    /// Key of the open encrypted card, needed to re-encrypt on save.
    card_key: Option<CardKey>,
    /// Lock password just entered to encrypt the card (used once, then dropped).
    pending_password: Option<Zeroizing<String>>,
    /// What the signed-in user may do with the open card.
    card_access: CardAccess,
    /// Display name of whoever changed the card last, and of its owner when shared.
    updated_by: Option<SharedString>,
    owner_name: Option<SharedString>,
    names_task: Option<Task<()>>,
    saved_revision: u64,
    title_dirty: bool,
    save_task: Option<Task<()>>,
    relock_task: Option<Task<()>>,
    /// Loads the spelling dictionary; replaced when the settings change again.
    spell_task: Option<Task<()>>,
    /// Card whose protection is being changed. Saves wait for it: they would write
    /// with the old lock mode (e.g. plaintext over a card just encrypted).
    lock_change: Option<CardId>,
    suppress_title_event: bool,
    _subs: Vec<Subscription>,
}

impl EventEmitter<CardPaneEvent> for CardPane {}

impl CardPane {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let blobs = crate::state::blob_store(cx);
        let editor = cx.new(|cx| {
            let mut e = RichEditor::new(Document::default(), cx);
            e.set_blob_store(blobs.clone());
            // The editor handles right-to-left text itself: give it logical order.
            e.set_placeholder(Some(cardfile_i18n::t_raw("editor.placeholder")));
            e
        });
        let toolbar = cx.new(|cx| EditorToolbar::new(editor.clone(), cx));
        let title = cx.new(|cx| {
            let mut e = RichEditor::new(Document::default(), cx);
            e.set_single_line(true);
            e.set_strong(true, cx);
            e.set_font_size(px(20.), cx);
            e.set_placeholder(Some(cardfile_i18n::t_raw("card.untitled")));
            e
        });
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
                |this, _, ev: &EditorEvent, window, cx| match ev {
                    EditorEvent::Changed if !this.suppress_title_event => {
                        this.title_dirty = true;
                        this.schedule_save(cx);
                    }
                    EditorEvent::Submit => {
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
            blobs,
            spell_task: None,
            lock_change: None,
            card: None,
            access: Access::Empty,
            tags: Vec::new(),
            editor,
            toolbar,
            title,
            unlock,
            unlock_error: None,
            unlocking: false,
            card_key: None,
            pending_password: None,
            card_access: CardAccess::Owner,
            updated_by: None,
            owner_name: None,
            names_task: None,
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

    pub fn editor(&self) -> &Entity<RichEditor> {
        &self.editor
    }

    /// Whether the content is visible and the user may change it.
    fn editable(&self) -> bool {
        self.access == Access::Open && self.card_access.can_edit()
    }

    /// Whether the open card has unsaved changes.
    pub fn has_unsaved_changes(&self, cx: &gpui_kit::App) -> bool {
        self.save_task.is_some()
            || self.title_dirty
            || self.editor.read(cx).revision() != self.saved_revision
    }

    /// Show a newer version of the open card saved elsewhere (another user, the AI),
    /// unless local edits are pending or the content is hidden.
    pub fn refresh_from(
        &mut self,
        card: Card,
        access: CardAccess,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.card.as_ref().filter(|c| c.id == card.id) else {
            return;
        };
        if card.rev <= current.rev || self.has_unsaved_changes(cx) || self.lock_change.is_some() {
            return;
        }
        if card.lock_mode == current.lock_mode && card.lock_mode.hides_content() {
            // Keep the unlocked state; only the encrypted payload changed.
            if self.access == Access::Open
                && let Some(key) = self.card_key.clone()
                && let Some(payload) = card.encrypted.clone()
            {
                let id = card.id;
                let task = cx.background_spawn(async move {
                    let json = cardfile_crypto::decrypt_with_key(
                        &payload,
                        &key,
                        id.to_string().as_bytes(),
                    )?;
                    anyhow::Ok(Document::from_json(&json)?)
                });
                cx.spawn_in(window, async move |this, cx| {
                    if let Ok(doc) = task.await {
                        let _ = this.update_in(cx, |this, window, cx| {
                            let title = card.title.clone();
                            this.card = Some(card);
                            this.set_title(&title, window, cx);
                            this.set_editor_doc(doc, !this.editable(), cx);
                            cx.notify();
                        });
                    }
                })
                .detach();
            } else if self.access == Access::Open && card.lock_mode == LockMode::Private {
                if let Ok(doc) = Document::from_json(&card.content) {
                    let title = card.title.clone();
                    self.card = Some(card);
                    self.set_title(&title, window, cx);
                    self.set_editor_doc(doc, !self.editable(), cx);
                    cx.notify();
                }
            } else {
                self.card = Some(card);
                cx.notify();
            }
            return;
        }
        self.open(Some(card), access, window, cx);
    }

    /// Look up the names shown in the status bar and header.
    fn load_names(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self.card.as_ref() else {
            self.updated_by = None;
            self.owner_name = None;
            return;
        };
        let (by, owner, shared) = (card.updated_by, card.owner_id, !self.card_access.is_owner());
        let me = AppState::session(cx).map(|s| s.user.id);
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            let name = |id| db.user(id).ok().map(|u| SharedString::from(u.display_name));
            let by = by.filter(|b| Some(*b) != me).and_then(name);
            let owner = if shared { name(owner) } else { None };
            (by, owner)
        });
        self.names_task = Some(cx.spawn(async move |this, cx| {
            let (by, owner) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.updated_by = by;
                this.owner_name = owner;
                cx.notify();
            });
        }));
    }

    pub fn set_tags(&mut self, tags: Vec<Tag>, cx: &mut Context<Self>) {
        self.tags = tags;
        cx.notify();
    }

    pub fn apply_user_settings(&mut self, cx: &mut Context<Self>) {
        let s = AppState::user_settings(cx);
        self.editor.update(cx, |e, cx| {
            e.set_font_size(px(s.editor_font_size), cx);
            e.set_input_rules(s.markdown_shortcuts);
        });
        let spell = crate::spell::checker(&s, cx);
        self.spell_task = Some(cx.spawn(async move |this, cx| {
            let spell = spell.await;
            let _ = this.update(cx, |this, cx| {
                this.editor
                    .update(cx, |e, cx| e.set_spell_checker(spell, cx));
            });
        }));
    }

    /// Show `card` (or nothing) with the user's `access` to it. Saves the previous
    /// card first.
    pub fn open(
        &mut self,
        card: Option<Card>,
        access: CardAccess,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.flush_save(cx);
        self.card_key = None;
        self.pending_password = None;
        self.card_access = access;
        self.relock_task = None;
        self.unlock_error = None;
        self.unlock.update(cx, |s, cx| s.set_value("", window, cx));
        self.card = card;
        self.load_names(cx);
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
        self.blobs
            .set_sealing(card.lock_mode == LockMode::Encrypted);
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
                self.set_editor_doc(doc, !self.editable(), cx);
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
        let disabled = !self.editable();
        let _ = window;
        self.title.update(cx, |e, cx| {
            e.set_text(&shown, cx);
            e.set_read_only(disabled, cx);
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
                } else if !self.card_access.can_edit() {
                    window.push_notification(Notification::info(t("share.view_only")), cx);
                }
            }
            EditorEvent::OpenLink(url) => cx.open_url(url),
            EditorEvent::OpenAttachment { blob, file_name } => {
                crate::export::open_attachment(blob.clone(), file_name.clone(), window, cx)
            }
            EditorEvent::Submit => {}
        }
    }

    // ---------------------------------------------------------------------
    // Saving
    // ---------------------------------------------------------------------

    fn is_dirty(&self, cx: &Context<Self>) -> bool {
        self.title_dirty || self.editor.read(cx).revision() != self.saved_revision
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        if !self.editable() {
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
        if self.lock_change == Some(card.id) {
            // Retried when the protection change finishes.
            return;
        }
        if !self.editable() || !self.is_dirty(cx) {
            return;
        }
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        let id = card.id;
        let encrypted = card.lock_mode == LockMode::Encrypted;
        let doc = self.editor.read(cx).document().clone();
        let title = self.title.read(cx).text().trim().to_string();
        let title = if title.is_empty() {
            cardfile_i18n::t_raw("card.untitled")
        } else {
            title
        };
        let revision = self.editor.read(cx).revision();
        let key = self.card_key.clone();
        let keep = AppState::user_settings(cx).max_revisions;
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            let json = doc.to_json();
            let update = if encrypted {
                let key = key.ok_or_else(|| anyhow::anyhow!("encrypted card is not unlocked"))?;
                let payload =
                    cardfile_crypto::encrypt_with_key(&json, &key, id.to_string().as_bytes())?;
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
            anyhow::Ok(db.update_card_content(&actor, id, update, keep, SNAPSHOT_EVERY_MS)?)
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
                            card.updated_by = saved.updated_by;
                            card.rev = saved.rev;
                            this.updated_by = None;
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
        let Some(user) = AppState::session(cx).map(|s| s.user.clone()) else {
            return;
        };
        if user.effective_lock_hash().is_none() {
            self.unlock_error = Some(t("card.unlock.no_password").into());
            cx.notify();
            return;
        }
        self.unlocking = true;
        self.unlock_error = None;
        cx.notify();
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            let opened = crate::locks::open_card(&db, &user, &card, &password)?;
            // A legacy card was upgraded: reload it so later saves see the new format.
            let card = db.card(card.id)?;
            anyhow::Ok((opened, card))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.unlocking = false;
                match result {
                    Ok((opened, fresh)) => {
                        AppState::refresh_user(cx, opened.user);
                        let doc = opened.doc;
                        this.card_key = opened.key;
                        if let Some(card) = this.card.as_mut().filter(|c| c.id == fresh.id) {
                            card.encrypted = fresh.encrypted;
                            card.rev = fresh.rev;
                        }
                        this.access = Access::Open;
                        let title = this
                            .card
                            .as_ref()
                            .map(|c| c.title.clone())
                            .unwrap_or_default();
                        this.set_title(&title, window, cx);
                        let read_only = !this.editable();
                        this.set_editor_doc(doc, read_only, cx);
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
        self.card_key = None;
        self.access = Access::Locked;
        self.set_editor_doc(Document::default(), true, cx);
        cx.notify();
    }

    /// Allow editing a read-only card until another card is opened.
    fn unlock_read_only(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.access != Access::ReadOnly || !self.card_access.can_edit() {
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
        if card.lock_mode == mode || !self.card_access.is_owner() {
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
        if mode == LockMode::Encrypted && self.card_key.is_none() && self.pending_password.is_none()
        {
            crate::dialogs::ask_password(
                t("card.protect.encrypt_title"),
                t("card.protect.encrypt_body"),
                cx.entity(),
                |this, password, window, cx| {
                    this.pending_password = Some(password);
                    this.set_lock_mode(LockMode::Encrypted, window, cx);
                },
                window,
                cx,
            );
            return;
        }
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        let Some(user) = AppState::session(cx).map(|s| s.user.clone()) else {
            return;
        };
        self.flush_save(cx);
        let doc = self.editor.read(cx).document().clone();
        let existing_key = self.card_key.clone();
        let password = self.pending_password.take();
        let db = AppState::db(cx);
        let mut content_doc = doc;
        let blobs = self.blobs.clone();
        // The document is rewritten in the background; edits made meanwhile would be lost.
        let was_open = self.access == Access::Open;
        if was_open {
            self.editor.update(cx, |e, cx| e.set_read_only(true, cx));
        }
        let card_id = card.id;
        self.lock_change = Some(card_id);
        let task = cx.background_spawn(async move {
            if mode == LockMode::Encrypted {
                // Re-store plaintext images sealed; the old copies are dropped below.
                content_doc.map_blob_refs(|r| {
                    if BlobRef::parse(r).key.is_some() {
                        return anyhow::Ok(None);
                    }
                    let Some((_, data)) = blobs.read(r)? else {
                        // Already broken; keep the reference rather than fail the change.
                        tracing::warn!("missing blob {r} while encrypting a card");
                        return Ok(None);
                    };
                    Ok(Some(blobs.put_sealed(&data)?))
                })?;
            }
            let json = content_doc.to_json();
            let plain = cardfile_doc::to_plain_text(&content_doc);
            let blob_ids = content_doc.blob_ids();
            let (card, key, user) = if mode == LockMode::Encrypted {
                let (key, user) = match (existing_key, password) {
                    (Some(key), _) => (key, None),
                    (None, Some(pw)) => {
                        // Creates the user's key pair if this is their first encryption.
                        let (user, _) = crate::locks::unlock_user(&db, &user, &pw)?;
                        (CardKey::generate()?, Some(user))
                    }
                    (None, None) => return Err(anyhow::anyhow!("missing password")),
                };
                let keys = crate::locks::wrap_for_all(&db, card.id, card.owner_id, &key)?;
                let payload =
                    cardfile_crypto::encrypt_with_key(&json, &key, card.id.to_string().as_bytes())?;
                let saved = db.set_card_lock(
                    &actor,
                    card.id,
                    mode,
                    "",
                    "",
                    Some(&payload),
                    &keys,
                    &blob_ids,
                )?;
                (saved, Some(key), user)
            } else {
                let saved =
                    db.set_card_lock(&actor, card.id, mode, &json, &plain, None, &[], &blob_ids)?;
                (saved, None, None)
            };
            anyhow::Ok((card, content_doc, key, user))
        });
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok((saved, doc, key, user)) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.lock_change = None;
                    if let Some(user) = user {
                        AppState::refresh_user(cx, user);
                    }
                    // Another card may have been opened meanwhile.
                    if this.card.as_ref().map(|c| c.id) != Some(saved.id) {
                        return;
                    }
                    let keep_open = this.access == Access::Open;
                    this.blobs
                        .set_sealing(saved.lock_mode == LockMode::Encrypted);
                    this.card = Some(saved);
                    if keep_open {
                        // Stay unlocked after changing protection. Image references
                        // may have been rewritten to sealed ones.
                        this.access = Access::Open;
                        this.card_key = key;
                        if this.editor.read(cx).document() != &doc {
                            this.set_editor_doc(doc, false, cx);
                        }
                        this.editor.update(cx, |e, cx| e.set_read_only(false, cx));
                    } else {
                        let card = this.card.clone();
                        this.open(card, this.card_access, window, cx);
                    }
                    // A title typed meanwhile is saved now, with the new lock mode.
                    if this.is_dirty(cx) {
                        this.save(cx);
                    }
                    cx.emit(CardPaneEvent::Saved);
                    cx.notify();
                });
            }
            Err(e) => {
                tracing::error!("change protection: {e:#}");
                let _ = this.update_in(cx, |this, window, cx| {
                    this.lock_change = None;
                    let same_card = this.card.as_ref().map(|c| c.id) == Some(card_id);
                    if was_open && same_card {
                        this.editor.update(cx, |e, cx| e.set_read_only(false, cx));
                        if this.is_dirty(cx) {
                            this.save(cx);
                        }
                    }
                    let error = format!("{e:#}");
                    window.push_notification(
                        Notification::error(t_args("card.protect.failed", &[("error", &error)])),
                        cx,
                    );
                });
            }
        })
        .detach();
    }

    // ---------------------------------------------------------------------
    // Flags, tags, recycle bin
    // ---------------------------------------------------------------------

    pub fn toggle_pin(&mut self, cx: &mut Context<Self>) {
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        if !self.card_access.is_owner() {
            return;
        }
        let Some(card) = self.card.as_mut() else {
            return;
        };
        card.is_pinned = !card.is_pinned;
        let (id, v) = (card.id, card.is_pinned);
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.set_card_pinned(&actor, id, v));
        self.after_flag(task, cx);
    }

    pub fn toggle_favorite(&mut self, cx: &mut Context<Self>) {
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        if !self.card_access.is_owner() {
            return;
        }
        let Some(card) = self.card.as_mut() else {
            return;
        };
        card.is_favorite = !card.is_favorite;
        let (id, v) = (card.id, card.is_favorite);
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.set_card_favorite(&actor, id, v));
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
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
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
        let task = run_db(cx, move |_| db.set_card_tags(&actor, id, &tags));
        self.after_flag(task, cx);
    }

    /// Move the card to the recycle bin.
    pub fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if !self.card_access.is_owner() {
            window.push_notification(Notification::info(t("share.owner_only")), cx);
            return;
        }
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
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        let db = AppState::db(cx);
        let id = card.id;
        let task = run_db(cx, move |_| db.delete_card(&actor, id));
        cx.spawn_in(window, async move |this, cx| {
            if task.await.is_ok() {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.open(None, CardAccess::Owner, window, cx);
                    cx.emit(CardPaneEvent::ListChanged);
                });
            }
        })
        .detach();
    }

    fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.card_id() else { return };
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| {
            db.restore_card(&actor, id)?;
            db.card(id)
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(card) = task.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.open(Some(card), CardAccess::Owner, window, cx);
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
                let Some(actor) = AppState::actor(cx) else {
                    return;
                };
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| db.purge_card(&actor, id));
                let pane = pane.clone();
                window
                    .spawn(cx, async move |cx| {
                        if task.await.is_ok() {
                            let _ = pane.update_in(cx, |this, window, cx| {
                                this.open(None, CardAccess::Owner, window, cx);
                                cx.emit(CardPaneEvent::ListChanged);
                            });
                        }
                    })
                    .detach();
            },
        );
    }

    /// Open the share dialog (owner only, multi-user installs).
    pub fn share(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if !self.card_access.is_owner() || card.is_deleted() {
            return;
        }
        self.flush_save(cx);
        let pane = cx.entity().downgrade();
        crate::share::open(
            card,
            self.card_key.clone(),
            move |cx| {
                let _ = pane.update(cx, |_, cx| cx.emit(CardPaneEvent::ListChanged));
            },
            window,
            cx,
        );
    }

    pub fn show_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(card) = self.card.clone() else {
            return;
        };
        if self.access != Access::Open && self.access != Access::ReadOnly {
            return;
        }
        crate::history::open(card, self.card_key.clone(), cx.entity(), window, cx);
    }

    /// Replace the content with a revision (called by the history dialog).
    pub fn restore_revision(
        &mut self,
        title: String,
        doc: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.card_access.can_edit() {
            return;
        }
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
        let _ = window;
        self.title.update(cx, |e, cx| e.set_text(title, cx));
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
        let title = self.title.read(cx).text();
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
        let owner = self.card_access.is_owner();
        let can_share =
            owner && AppState::get(cx).app.auth_mode == cardfile_core::AuthMode::MultiUser;
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
                    let p7 = pane.clone();
                    let menu = if can_share {
                        menu.item(
                            PopupMenuItem::new(t("share.button"))
                                .on_click(move |_, w, cx| p7.update(cx, |p, cx| p.share(w, cx))),
                        )
                        .separator()
                    } else {
                        menu
                    };
                    let menu = menu.item(
                        PopupMenuItem::new(t("card.history"))
                            .on_click(move |_, w, cx| p1.update(cx, |p, cx| p.show_history(w, cx))),
                    );
                    menu.separator()
                        .item(
                            PopupMenuItem::new(t("export.markdown")).on_click(move |_, w, cx| {
                                p2.update(cx, |p, cx| {
                                    p.export(crate::export::Format::Markdown, w, cx)
                                })
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
                        .when(owner, |m| {
                            m.separator().item(
                                PopupMenuItem::new(t("card.delete")).on_click(move |_, w, cx| {
                                    p6.update(cx, |p, cx| p.delete(w, cx))
                                }),
                            )
                        })
                })
        };
        let shared_badge = self.owner_name.clone().map(|name| {
            let perm = match self.card_access {
                CardAccess::Shared(cardfile_core::SharePermission::Edit) => t("share.edit"),
                _ => t("share.view"),
            };
            h_flex()
                .gap(px(4.))
                .px(px(8.))
                .py(px(2.))
                .rounded_full()
                .bg(cx.theme().accent)
                .text_size(px(12.))
                .child(gpui_kit::component::Icon::new(IconName::Users).size_3())
                .child(t_args(
                    "share.shared_by",
                    &[("name", &name), ("permission", &perm)],
                ))
        });
        let th = cx.theme();
        h_flex()
            .w_full()
            .gap(px(4.))
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(th.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(38.))
                    .child(self.title.clone()),
            )
            .children(shared_badge)
            .when(!deleted && can_share, |d| {
                d.child(
                    Button::new("share")
                        .icon(IconName::Users)
                        .ghost()
                        .small()
                        .tooltip(t("share.button"))
                        .on_click(cx.listener(|this, _, window, cx| this.share(window, cx))),
                )
            })
            .when(!deleted && owner, |d| {
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
            })
            .when(!deleted, |d| d.child(more))
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
                Some(c) => match &self.updated_by {
                    Some(by) => t_args(
                        "card.updated_by",
                        &[("date", &format_date(c.updated_at)), ("name", by)],
                    ),
                    None => t_args("card.updated", &[("date", &format_date(c.updated_at))]),
                },
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
                        .child(
                            gpui_kit::img("cardfile/icon.png")
                                .size(px(56.))
                                .opacity(0.85),
                        )
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
