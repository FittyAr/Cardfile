//! The main screen: sidebar, card list and card pane.

use std::time::Duration;

use cardfile_core::{
    AuthMode, CardAccess, CardFilter, CardId, CardSummary, LockMode, SortOrder, Tag, TagId,
    mask_title,
};
use cardfile_i18n::{t, t_args};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    FontWeight, Global, HighlightStyle, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, ScrollStrategy, SharedString, StatefulInteractiveElement as _, Styled as _, StyledText,
    Subscription, Task, UniformListScrollHandle, WeakEntity, Window, div, img, px, uniform_list,
};

use crate::card_pane::{CardPane, CardPaneEvent};
use crate::chat::{ChatEvent, ChatPanel};
use crate::keymap::*;
use crate::state::{AppState, TaskExt as _, run_db};
use crate::widgets::relative_time;

pub enum WorkspaceEvent {
    Lock,
    SignOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    All,
    Favorites,
    Tag(TagId),
    /// Cards other users share with me.
    Shared,
    RecycleBin,
}

/// A card being dragged in the list.
#[derive(Clone)]
struct DraggedCard {
    id: CardId,
    title: SharedString,
}

struct DragPreview(SharedString);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().primary)
            .shadow_md()
            .child(self.0.clone())
    }
}

pub struct Workspace {
    focus: FocusHandle,
    view: View,
    search: Entity<InputState>,
    cards: Vec<CardSummary>,
    tags: Vec<Tag>,
    selected: Option<CardId>,
    pane: Entity<CardPane>,
    list_scroll: UniformListScrollHandle,
    load_task: Option<Task<()>>,
    search_task: Option<Task<()>>,
    /// All, favorites, recycle bin, shared with me.
    counts: (usize, usize, usize, usize),
    /// The assistant panel, created the first time it is shown.
    chat: Option<Entity<ChatPanel>>,
    /// Last [`crate::mcp_host::DATA_VERSION`] seen, to notice changes made by AI clients.
    data_version: u64,
    _watch: Option<Task<()>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

/// The signed-in workspace, so views that may outlive it (dialogs) look it up live
/// instead of holding on to it.
#[derive(Default)]
struct CurrentWorkspace(Option<WeakEntity<Workspace>>);

impl Global for CurrentWorkspace {}

impl Workspace {
    /// The workspace on screen, if someone is signed in.
    pub(crate) fn current(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<CurrentWorkspace>()?.0.as_ref()?.upgrade()
    }

    /// Record the workspace on screen (`None` when leaving it).
    pub(crate) fn set_current(ws: Option<&Entity<Self>>, cx: &mut App) {
        cx.set_global(CurrentWorkspace(ws.map(Entity::downgrade)));
    }
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

const ITEM_HEIGHT: f32 = 74.;

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(t("search.placeholder")));
        let pane = cx.new(|cx| CardPane::new(window, cx));
        let subs = vec![
            cx.subscribe_in(
                &search,
                window,
                |this, _, ev: &InputEvent, window, cx| match ev {
                    InputEvent::Change => {
                        // Debounce typing in the search box.
                        this.search_task = Some(cx.spawn_in(window, async move |this, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(150))
                                .await;
                            let _ = this.update_in(cx, |this, window, cx| this.reload(window, cx));
                        }));
                    }
                    InputEvent::PressEnter { .. } => {
                        if let Some(first) = this.cards.first().map(|c| c.id) {
                            this.select(first, window, cx);
                        }
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(
                &pane,
                window,
                |this, pane, ev: &CardPaneEvent, window, cx| match ev {
                    CardPaneEvent::Saved => this.reload(window, cx),
                    CardPaneEvent::ListChanged => {
                        // The card may have been deleted, purged or closed.
                        this.selected = pane.read(cx).card_id();
                        this.reload(window, cx);
                    }
                    CardPaneEvent::SaveFailed(error) => window.push_notification(
                        Notification::error(t_args("card.save_failed", &[("error", error)])),
                        cx,
                    ),
                },
            ),
            cx.observe_global_in::<crate::tags::TagsChanged>(window, |this, window, cx| {
                this.reload(window, cx)
            }),
        ];
        let mut ws = Self {
            focus: cx.focus_handle(),
            view: View::All,
            search,
            cards: Vec::new(),
            tags: Vec::new(),
            selected: None,
            pane,
            list_scroll: UniformListScrollHandle::new(),
            load_task: None,
            search_task: None,
            counts: (0, 0, 0, 0),
            chat: None,
            data_version: crate::mcp_host::DATA_VERSION.load(std::sync::atomic::Ordering::Relaxed),
            _watch: None,
            _subs: subs,
        };
        ws._watch = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                let now = crate::mcp_host::DATA_VERSION.load(std::sync::atomic::Ordering::Relaxed);
                let alive = this.update_in(cx, |this, window, cx| {
                    if this.data_version != now {
                        this.data_version = now;
                        this.data_changed(window, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
        ws.reload(window, cx);
        ws
    }

    fn ensure_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<ChatPanel> {
        if let Some(chat) = &self.chat {
            return chat.clone();
        }
        let chat = cx.new(|cx| ChatPanel::new(window, cx));
        self._subs.push(
            cx.subscribe_in(
                &chat,
                window,
                |this, _, ev: &ChatEvent, window, cx| match ev {
                    ChatEvent::BeforeRun => this.pane.update(cx, |p, cx| p.flush_save(cx)),
                    ChatEvent::DataChanged(card) => {
                        this.data_changed(window, cx);
                        let open = this.pane.read(cx).card_id();
                        if let Some(id) = *card
                            && open != Some(id)
                            && !this.pane.read(cx).has_unsaved_changes(cx)
                        {
                            this.reveal(id, window, cx);
                        }
                    }
                    ChatEvent::Close => {
                        AppState::update_user_settings(cx, |s| s.ai_panel_open = false);
                        cx.notify();
                    }
                },
            ),
        );
        self.chat = Some(chat.clone());
        chat
    }

    fn toggle_assistant(
        &mut self,
        _: &ToggleAssistant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::ai_settings::active_provider(cx).is_none() {
            return;
        }
        let open = !AppState::user_settings(cx).ai_panel_open || self.chat.is_none();
        AppState::update_user_settings(cx, |s| s.ai_panel_open = open);
        if open {
            let chat = self.ensure_chat(window, cx);
            chat.update(cx, |c, cx| c.focus(window, cx));
        }
        cx.notify();
    }

    fn share_card(&mut self, _: &ShareCard, window: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.share(window, cx));
    }

    fn show_shared(&mut self, _: &ShowShared, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view(View::Shared, window, cx);
    }

    fn owner(cx: &App) -> Option<cardfile_core::UserId> {
        AppState::session(cx).map(|s| s.user.id)
    }

    fn filter(&self, cx: &App) -> CardFilter {
        let s = AppState::user_settings(cx);
        CardFilter {
            query: self.search.read(cx).value().to_string(),
            scope: s.search_scope,
            tag: match self.view {
                View::Tag(t) => Some(t),
                _ => None,
            },
            favorites_only: self.view == View::Favorites,
            deleted: self.view == View::RecycleBin,
            shared_with_me: self.view == View::Shared,
            sort: s.sort,
        }
    }

    /// Reload the card list, tags and counters.
    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(owner) = Self::owner(cx) else { return };
        let filter = self.filter(cx);
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| {
            let cards = db.list_cards(owner, &filter)?;
            let tags = db.list_tags(owner)?;
            let all = db.list_cards(owner, &CardFilter::default())?;
            let favorites = all.iter().filter(|c| c.is_favorite).count();
            let deleted = db
                .list_cards(
                    owner,
                    &CardFilter {
                        deleted: true,
                        ..Default::default()
                    },
                )?
                .len();
            let shared = db
                .list_cards(
                    owner,
                    &CardFilter {
                        shared_with_me: true,
                        ..Default::default()
                    },
                )?
                .len();
            Ok((cards, tags, (all.len(), favorites, deleted, shared)))
        });
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let Ok((cards, tags, counts)) = task.await else {
                return;
            };
            let _ = this.update_in(cx, |this, _window, cx| {
                this.cards = cards;
                this.counts = counts;
                this.pane.update(cx, |p, cx| p.set_tags(tags.clone(), cx));
                this.tags = tags;
                cx.notify();
            });
        }));
    }

    pub fn select(&mut self, id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        self.select_then(id, |_, _, _| {}, window, cx);
    }

    /// Select card `id` and run `then` on the pane once that card is open in it,
    /// so per-card commands (context menu) act on the card that was clicked.
    fn select_then(
        &mut self,
        id: CardId,
        then: impl FnOnce(&mut CardPane, &mut Window, &mut Context<CardPane>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected == Some(id) && self.pane.read(cx).card_id() == Some(id) {
            self.pane.update(cx, |p, cx| then(p, window, cx));
            return;
        }
        self.selected = Some(id);
        if let Some(ix) = self.cards.iter().position(|c| c.id == id) {
            self.list_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        let Some(me) = Self::owner(cx) else { return };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| crate::state::load_card(&db, id, me));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok((card, access)) = task.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.selected == Some(card.id) {
                        this.pane.update(cx, |p, cx| {
                            p.open(Some(card), access, window, cx);
                            then(p, window, cx);
                        });
                    }
                });
            }
        })
        .detach();
        cx.notify();
    }

    /// Something changed the data outside this view (the AI assistant, the MCP
    /// server): reload the list and refresh the open card if nobody is editing it.
    pub fn data_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reload(window, cx);
        let Some(me) = Self::owner(cx) else { return };
        let Some(id) = self.pane.read(cx).card_id() else {
            return;
        };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| crate::state::load_card(&db, id, me));
        cx.spawn_in(window, async move |this, cx| match task.await {
            Ok((card, access)) => {
                let _ = this.update_in(cx, |this, window, cx| {
                    // The user may have opened another card meanwhile.
                    if this.pane.read(cx).card_id() != Some(id) {
                        return;
                    }
                    let deleted_elsewhere = card.is_deleted()
                        && !this.pane.read(cx).card().is_some_and(|c| c.is_deleted());
                    this.pane.update(cx, |p, cx| {
                        if deleted_elsewhere {
                            p.open(None, CardAccess::Owner, window, cx);
                        } else {
                            p.refresh_from(card, access, window, cx);
                        }
                    });
                    if deleted_elsewhere {
                        this.selected = None;
                    }
                });
            }
            Err(_) => {
                // Purged or unshared meanwhile.
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.pane.read(cx).card_id() != Some(id) {
                        return;
                    }
                    this.pane
                        .update(cx, |p, cx| p.open(None, CardAccess::Owner, window, cx));
                    this.selected = None;
                });
            }
        })
        .detach();
    }

    /// Open a card by id (used by the assistant to show what it changed).
    pub fn reveal(&mut self, id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.select(id, window, cx);
    }

    fn set_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        self.view = view;
        self.reload(window, cx);
    }

    fn new_card(&mut self, _: &NewCard, window: &mut Window, cx: &mut Context<Self>) {
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        if matches!(self.view, View::RecycleBin | View::Shared) {
            self.view = View::All;
        }
        let settings = AppState::user_settings(cx);
        // Content-hiding modes need a password; fall back to no protection otherwise.
        let mut lock = settings.default_lock_mode;
        if lock.hides_content() && AppState::lock_password_hash(cx).is_none()
            || lock == LockMode::Encrypted
        {
            lock = LockMode::None;
        }
        let tag = match self.view {
            View::Tag(t) => Some(t),
            _ => None,
        };
        let favorite = self.view == View::Favorites;
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| {
            let card = db.create_card(
                &actor,
                cardfile_core::NewCard {
                    title: String::new(),
                    lock_mode: lock,
                    ..Default::default()
                },
            )?;
            if let Some(tag) = tag {
                db.set_card_tags(&actor, card.id, &[tag])?;
            }
            if favorite {
                db.set_card_favorite(&actor, card.id, true)?;
            }
            db.card(card.id)
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(card) = task.await {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.selected = Some(card.id);
                    this.pane.update(cx, |p, cx| {
                        p.open(Some(card), CardAccess::Owner, window, cx);
                    });
                    // A brand-new card is open for editing even if read-only by default.
                    this.reload(window, cx);
                    window.focus_next(cx);
                });
            }
        })
        .detach();
    }

    /// An image or attachment dragged from the open card was dropped on another card:
    /// add it there, and remove it from the open card unless Ctrl/Alt was held.
    fn drop_block_on_card(
        &mut self,
        dragged: &cardfile_editor::DraggedBlock,
        target: CardId,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pane = self.pane.read(cx);
        if pane.card_id() == Some(target) || pane.editor().entity_id() != dragged.source {
            return;
        }
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        self.pane.update(cx, |p, cx| p.flush_save(cx));
        let db = AppState::db(cx);
        let blobs = crate::state::blob_store(cx);
        let keep = AppState::user_settings(cx).max_revisions;
        let block = dragged.block.clone();
        let task = cx.background_spawn(async move {
            let (card, access) = crate::state::load_card(&db, target, actor.user)?;
            if !access.can_edit() || card.is_deleted() {
                return Err(anyhow::anyhow!(t("drag.cannot_edit")));
            }
            if card.lock_mode.hides_content() {
                return Err(anyhow::anyhow!(t("drag.protected")));
            }
            // Sealed blobs belong to an encrypted card; store a plain copy for this one.
            let mut block = block;
            let mut doc = cardfile_doc::Document {
                version: cardfile_doc::DOC_VERSION,
                blocks: vec![block.clone()],
            };
            doc.map_blob_refs(|r| {
                if cardfile_core::BlobRef::parse(r).key.is_none() {
                    return anyhow::Ok(None);
                }
                let Some((mime, data)) = blobs.read(r)? else {
                    return Ok(None);
                };
                let name = cardfile_doc::blob_file_name(r).to_owned();
                Ok(Some(db.put_blob(&data, &mime, Some(&name))?.id.0))
            })?;
            if let Some(b) = doc.blocks.pop() {
                block = b;
            }
            let mut target_doc = cardfile_doc::Document::from_json(&card.content)?;
            if target_doc.blocks.len() == 1
                && target_doc.blocks[0].kind.is_textual()
                && target_doc.blocks[0].text.is_empty()
            {
                target_doc.blocks.clear();
            }
            target_doc.blocks.push(block);
            target_doc.normalize();
            let update = cardfile_db::ContentUpdate {
                title: card.title.clone(),
                content: target_doc.to_json(),
                plain_text: cardfile_doc::to_plain_text(&target_doc),
                encrypted: None,
                blob_ids: target_doc.blob_ids(),
            };
            db.update_card_content(&actor, target, update, keep, 0)?;
            anyhow::Ok(card.title)
        });
        let (index, original) = (dragged.index, dragged.block.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(title) => {
                    if !copy {
                        let editor = this.pane.read(cx).editor().clone();
                        editor.update(cx, |e, cx| e.take_block(index, &original, cx));
                    }
                    let key = if copy { "drag.copied" } else { "drag.moved" };
                    window.push_notification(
                        gpui_kit::component::notification::Notification::success(t_args(
                            key,
                            &[("title", &title)],
                        )),
                        cx,
                    );
                    this.reload(window, cx);
                }
                Err(e) => window.push_notification(
                    gpui_kit::component::notification::Notification::error(e.to_string()),
                    cx,
                ),
            });
        })
        .detach();
    }

    fn move_card(
        &mut self,
        dragged: CardId,
        target_ix: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(from) = self.cards.iter().position(|c| c.id == dragged) else {
            return;
        };
        if from == target_ix {
            return;
        }
        // Neighbours at the drop position (after removing the dragged card).
        let mut order: Vec<CardId> = self.cards.iter().map(|c| c.id).collect();
        let id = order.remove(from);
        let at = target_ix.min(order.len());
        order.insert(at, id);
        let before = at.checked_sub(1).map(|i| order[i]);
        let after = order.get(at + 1).copied();
        let card = self.cards.remove(from);
        self.cards.insert(at, card);
        cx.notify();
        let Some(actor) = AppState::actor(cx) else {
            return;
        };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.move_card(&actor, dragged, before, after));
        cx.spawn_in(window, async move |this, cx| {
            let _ = task.await;
            let _ = this.update_in(cx, |this, window, cx| this.reload(window, cx));
        })
        .detach();
    }

    // ---------------------------------------------------------------------
    // Actions
    // ---------------------------------------------------------------------

    fn delete_card(&mut self, _: &DeleteCard, window: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.delete(window, cx));
    }
    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.search.focus_handle(cx);
        focus.focus(window, cx);
    }
    fn toggle_pin(&mut self, _: &TogglePin, _: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.toggle_pin(cx));
    }
    fn toggle_favorite(&mut self, _: &ToggleFavorite, _: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.toggle_favorite(cx));
    }
    fn export_card(&mut self, _: &ExportCard, window: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| {
            p.export(crate::export::Format::Markdown, window, cx)
        });
    }
    fn export_all(&mut self, _: &ExportAll, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = Self::owner(cx) {
            crate::export::export_all(owner, crate::export::Format::Markdown, window, cx);
        }
    }
    fn show_history(&mut self, _: &ShowHistory, window: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.show_history(window, cx));
    }
    fn show_all(&mut self, _: &ShowAllCards, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view(View::All, window, cx);
    }
    fn show_favorites(&mut self, _: &ShowFavorites, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view(View::Favorites, window, cx);
    }
    fn show_recycle(&mut self, _: &ShowRecycleBin, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view(View::RecycleBin, window, cx);
    }
    fn lock_app(&mut self, _: &LockApp, _: &mut Window, cx: &mut Context<Self>) {
        if AppState::get(cx).app.auth_mode == AuthMode::NoPassword {
            // Nothing to unlock with; just hide protected content.
            self.pane.update(cx, |p, cx| p.relock(cx));
            return;
        }
        self.pane.update(cx, |p, cx| p.flush_save(cx));
        cx.emit(WorkspaceEvent::Lock);
    }
    fn sign_out(&mut self, _: &SignOut, _: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.flush_save(cx));
        cx.emit(WorkspaceEvent::SignOut);
    }
    fn empty_recycle_bin(
        &mut self,
        _: &EmptyRecycleBin,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(owner) = Self::owner(cx) else { return };
        let ws = cx.entity();
        crate::dialogs::confirm(
            t("recycle.empty_title"),
            t("recycle.empty_body"),
            t("recycle.empty"),
            window,
            cx,
            move |window, cx| {
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| db.empty_recycle_bin(owner));
                let ws = ws.clone();
                window
                    .spawn(cx, async move |cx| {
                        let result = task.await;
                        let _ = ws.update_in(cx, |ws, window, cx| {
                            if let Err(e) = result {
                                window.push_notification(
                                    Notification::error(t_args(
                                        "recycle.empty_failed",
                                        &[("error", &format!("{e:#}"))],
                                    )),
                                    cx,
                                );
                            }
                            // Close the open card if it was in the bin (now purged).
                            let closed = ws.pane.update(cx, |p, cx| {
                                let deleted = p.card().is_some_and(|c| c.is_deleted());
                                if deleted {
                                    p.open(None, CardAccess::Owner, window, cx);
                                }
                                deleted
                            });
                            if closed {
                                ws.selected = None;
                            }
                            ws.reload(window, cx);
                        });
                    })
                    .detach();
            },
        );
    }

    /// Save the open card now; the task finishes once the write landed.
    pub fn flush(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.pane.update(cx, |p, cx| p.save_now(cx))
    }

    pub fn apply_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pane.update(cx, |p, cx| p.apply_user_settings(cx));
        self.reload(window, cx);
    }

    // ---------------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------------

    fn nav_item(
        &self,
        id: &'static str,
        icon: IconName,
        label: String,
        count: Option<usize>,
        view: View,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.view == view;
        let th = cx.theme();
        h_flex()
            .id(id)
            .gap(px(8.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .text_size(px(14.))
            .when(active, |d| {
                d.bg(th.sidebar_accent).font_weight(FontWeight::MEDIUM)
            })
            .hover(|d| d.bg(th.sidebar_accent.opacity(0.6)))
            .on_click(cx.listener(move |this, _, window, cx| this.set_view(view, window, cx)))
            .child(
                gpui_kit::component::Icon::new(icon)
                    .size_4()
                    .text_color(th.muted_foreground),
            )
            .child(div().flex_1().child(label))
            .children(count.map(|n| {
                div()
                    .text_size(px(12.))
                    .text_color(th.muted_foreground)
                    .child(n.to_string())
            }))
            .into_any_element()
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let session = AppState::session(cx).cloned();
        let multi = AppState::get(cx).app.auth_mode != AuthMode::NoPassword;
        let multi_user = AppState::get(cx).app.auth_mode == AuthMode::MultiUser;
        let (all, favs, deleted, shared) = self.counts;
        let mut tags = v_flex().gap(px(1.));
        for tag in &self.tags {
            let id = tag.id;
            let active = self.view == View::Tag(id);
            let color = cardfile_editor::parse_hex(&tag.color).unwrap_or(cx.theme().primary);
            let th = cx.theme();
            let tag_clone = tag.clone();
            tags = tags.child(
                h_flex()
                    .id(SharedString::from(format!("tag-{id}")))
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(14.))
                    .when(active, |d| d.bg(th.sidebar_accent))
                    .hover(|d| d.bg(th.sidebar_accent.opacity(0.6)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_view(View::Tag(id), window, cx)
                    }))
                    .child(div().size(px(9.)).rounded_full().bg(color))
                    .child(cardfile_editor::bidi::display(&tag.name))
                    .context_menu(move |menu, _, _| {
                        let edit = tag_clone.clone();
                        let del = tag_clone.clone();
                        menu.item(PopupMenuItem::new(t("tags.edit")).on_click(
                            move |_, window, cx| {
                                crate::tags::edit_tag_dialog(Some(edit.clone()), window, cx);
                            },
                        ))
                        .item(
                            PopupMenuItem::new(t("tags.delete")).on_click(move |_, window, cx| {
                                let id = del.id;
                                crate::dialogs::confirm(
                                    t("tags.delete"),
                                    t_args("tags.delete_body", &[("name", &del.name)]),
                                    t("tags.delete"),
                                    window,
                                    cx,
                                    move |_, cx| {
                                        let Some(owner) = Self::owner(cx) else {
                                            return;
                                        };
                                        let db = AppState::db(cx);
                                        run_db(cx, move |_| db.delete_tag(owner, id))
                                            .detach_and_log_err(cx);
                                        cx.set_global(crate::tags::TagsChanged);
                                    },
                                );
                            }),
                        )
                    }),
            );
        }
        let th = cx.theme();
        let user_menu = Button::new("user-menu")
            .ghost()
            .small()
            .icon(IconName::Settings)
            .tooltip(t("menu.settings"))
            .dropdown_menu(move |menu, _, _| {
                let mut menu = menu
                    .menu(t("menu.settings"), Box::new(OpenSettings))
                    .menu(t("menu.command_palette"), Box::new(CommandPalette))
                    .separator()
                    .menu(t("menu.export_all"), Box::new(ExportAll))
                    .menu(t("menu.backup"), Box::new(BackupNow))
                    .menu(t("menu.open_data_folder"), Box::new(OpenDataFolder))
                    .separator()
                    .menu(t("menu.check_updates"), Box::new(CheckForUpdates))
                    .menu(t("menu.about"), Box::new(About));
                if multi {
                    menu = menu
                        .separator()
                        .menu(t("menu.lock"), Box::new(LockApp))
                        .menu(t("menu.sign_out"), Box::new(SignOut));
                }
                menu.separator().menu(t("menu.quit"), Box::new(Quit))
            });
        v_flex()
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            .bg(th.sidebar)
            .border_r_1()
            .border_color(th.sidebar_border)
            .text_color(th.sidebar_foreground)
            .child(
                h_flex()
                    .gap(px(8.))
                    .px(px(14.))
                    .py(px(12.))
                    .child(img("cardfile/icon.png").size(px(24.)))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(16.))
                            .child(t("app.name")),
                    ),
            )
            .child(
                div().px(px(10.)).pb(px(8.)).child(
                    Input::new(&self.search)
                        .small()
                        .prefix(gpui_kit::component::Icon::new(IconName::Search).size_4())
                        .cleanable(true),
                ),
            )
            .child(
                v_flex()
                    .px(px(6.))
                    .gap(px(1.))
                    .child(self.nav_item(
                        "nav-all",
                        IconName::NotebookPen,
                        t("nav.all"),
                        Some(all),
                        View::All,
                        cx,
                    ))
                    .child(self.nav_item(
                        "nav-fav",
                        IconName::Star,
                        t("nav.favorites"),
                        Some(favs),
                        View::Favorites,
                        cx,
                    ))
                    .when(multi_user || shared > 0, |d| {
                        d.child(self.nav_item(
                            "nav-shared",
                            IconName::Users,
                            t("nav.shared"),
                            Some(shared),
                            View::Shared,
                            cx,
                        ))
                    })
                    .child(self.nav_item(
                        "nav-bin",
                        IconName::Delete,
                        t("nav.recycle_bin"),
                        Some(deleted),
                        View::RecycleBin,
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .justify_between()
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(4.))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(t("nav.tags"))
                    .child(
                        Button::new("add-tag")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip(t("tags.new"))
                            .on_click(|_, window, cx| crate::tags::new_tag_dialog(window, cx)),
                    ),
            )
            .child(
                div()
                    .id("tag-list")
                    .flex_1()
                    .px(px(6.))
                    .child(tags)
                    .overflow_y_scrollbar(),
            )
            .child(
                h_flex()
                    .justify_between()
                    .px(px(12.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(cx.theme().sidebar_border)
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .min_w_0()
                            .child(gpui_kit::component::Icon::new(IconName::User).size_4())
                            .child(
                                div().text_size(px(13.)).truncate().child(
                                    session.map(|s| s.user.display_name).unwrap_or_default(),
                                ),
                            ),
                    )
                    .child(user_menu),
            )
            .into_any_element()
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = AppState::user_settings(cx);
        let manual = settings.sort == SortOrder::Manual
            && self.search.read(cx).value().is_empty()
            && !matches!(self.view, View::RecycleBin | View::Shared);
        let count = self.cards.len();
        let title = match self.view {
            View::All => t("nav.all"),
            View::Favorites => t("nav.favorites"),
            View::Shared => t("nav.shared"),
            View::RecycleBin => t("nav.recycle_bin"),
            View::Tag(id) => self
                .tags
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.name.clone())
                .unwrap_or_default(),
        };
        let sort = settings.sort;
        let sort_menu = Button::new("sort")
            .ghost()
            .xsmall()
            .icon(IconName::ArrowDownUp)
            .tooltip(t("sort.title"))
            .dropdown_menu(move |mut menu, _, _| {
                for (s, key) in [
                    (SortOrder::Manual, "sort.manual"),
                    (SortOrder::UpdatedDesc, "sort.updated"),
                    (SortOrder::CreatedDesc, "sort.created"),
                    (SortOrder::TitleAsc, "sort.title_asc"),
                ] {
                    menu = menu.item(PopupMenuItem::new(t(key)).checked(sort == s).on_click(
                        move |_, window, cx| {
                            AppState::update_user_settings(cx, |u| u.sort = s);
                            window.dispatch_action(Box::new(crate::keymap::ShowAllCards), cx);
                            window.refresh();
                        },
                    ));
                }
                menu
            });
        let header = h_flex()
            .justify_between()
            .px(px(14.))
            .py(px(10.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(t_args("list.count", &[("n", &count.to_string())])),
                    ),
            )
            .child(
                h_flex()
                    .gap(px(2.))
                    .when(crate::ai_settings::active_provider(cx).is_some(), |d| {
                        d.child(
                            Button::new("assistant")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Bot)
                                .tooltip(t("chat.toggle"))
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(ToggleAssistant), cx)
                                }),
                        )
                    })
                    .child(sort_menu)
                    .when(self.view == View::RecycleBin && count > 0, |d| {
                        d.child(
                            Button::new("empty-bin")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Delete)
                                .tooltip(t("recycle.empty"))
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(EmptyRecycleBin), cx)
                                }),
                        )
                    })
                    .when(!matches!(self.view, View::RecycleBin | View::Shared), |d| {
                        d.child(
                            Button::new("new-card")
                                .primary()
                                .xsmall()
                                .icon(IconName::Plus)
                                .tooltip(t("card.new"))
                                .on_click(|_, window, cx| {
                                    window.dispatch_action(Box::new(NewCard), cx)
                                }),
                        )
                    }),
            );
        let mask_chars = settings.mask_visible_chars;
        let list = if count == 0 {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .p(px(20.))
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(if self.search.read(cx).value().is_empty() {
                    t("list.empty")
                } else {
                    t("list.no_results")
                })
                .into_any_element()
        } else {
            let list = uniform_list(
                "cards",
                count,
                cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                    range
                        .map(|ix| this.render_item(ix, manual, mask_chars, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .size_full()
            .track_scroll(&self.list_scroll);
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(list)
                .vertical_scrollbar(&self.list_scroll)
                .into_any_element()
        };
        v_flex()
            .w(px(settings.list_width.clamp(220., 520.)))
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(header)
            .child(list)
            .into_any_element()
    }

    fn render_item(
        &self,
        ix: usize,
        manual: bool,
        mask_chars: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(card) = self.cards.get(ix) else {
            return div().into_any_element();
        };
        let th = cx.theme();
        let selected = self.selected == Some(card.id);
        // List labels are laid out left to right only; show right-to-left text in
        // visual order (`t` already does that for UI strings).
        let title = if card.title.trim().is_empty() {
            t("card.untitled")
        } else if card.lock_mode == LockMode::Private {
            cardfile_editor::bidi::display(&mask_title(&card.title, mask_chars))
        } else {
            cardfile_editor::bidi::display(&card.title)
        };
        let secondary: AnyElement = match &card.snippet {
            Some(snippet) => {
                // Matches are wrapped in \u{1}…\u{2}.
                let mut text = String::new();
                let mut highlights = Vec::new();
                let mut start = None;
                for ch in snippet.chars() {
                    match ch {
                        '\u{1}' => start = Some(text.len()),
                        '\u{2}' => {
                            if let Some(s) = start.take() {
                                highlights.push((
                                    s..text.len(),
                                    HighlightStyle {
                                        font_weight: Some(FontWeight::BOLD),
                                        background_color: Some(th.warning.opacity(0.3)),
                                        ..Default::default()
                                    },
                                ));
                            }
                        }
                        '\n' => text.push(' '),
                        c => text.push(c),
                    }
                }
                StyledText::new(text)
                    .with_highlights(highlights)
                    .into_any_element()
            }
            None if card.lock_mode.hides_content() => {
                t(&format!("lock.{}", card.lock_mode.as_str())).into_any_element()
            }
            None => cardfile_editor::bidi::display(&card.preview).into_any_element(),
        };
        let id = card.id;
        let dragged = DraggedCard {
            id,
            title: title.clone().into(),
        };
        let mut flags = h_flex().gap(px(4.)).flex_shrink_0();
        if card.is_pinned {
            flags = flags.child(
                gpui_kit::component::Icon::new(IconName::Pin)
                    .size_3()
                    .text_color(th.muted_foreground),
            );
        }
        if card.is_favorite {
            flags = flags.child(
                gpui_kit::component::Icon::new(IconName::Star)
                    .size_3()
                    .text_color(th.warning),
            );
        }
        if card.lock_mode != LockMode::None {
            flags = flags.child(
                gpui_kit::component::Icon::new(IconName::Lock)
                    .size_3()
                    .text_color(th.muted_foreground),
            );
        }
        if card.is_shared || !card.access.is_owner() {
            flags = flags.child(
                gpui_kit::component::Icon::new(IconName::Users)
                    .size_3()
                    .text_color(th.muted_foreground),
            );
        }
        let mut tag_dots = h_flex().gap(px(3.));
        for tag in self
            .tags
            .iter()
            .filter(|t| card.tags.contains(&t.id))
            .take(5)
        {
            tag_dots = tag_dots.child(
                div()
                    .size(px(6.))
                    .rounded_full()
                    .bg(cardfile_editor::parse_hex(&tag.color).unwrap_or(th.primary)),
            );
        }
        let mut item = v_flex()
            .id(("card", ix))
            .w_full()
            .overflow_hidden()
            .h(px(ITEM_HEIGHT))
            .px(px(14.))
            .py(px(8.))
            .gap(px(2.))
            .border_b_1()
            .border_color(th.border.opacity(0.5))
            .cursor_pointer()
            .when(selected, |d| {
                d.bg(th.list_active)
                    .border_l_2()
                    .border_color(th.list_active_border)
            })
            .when(!selected, |d| d.hover(|d| d.bg(th.list_hover)))
            .on_click(cx.listener(move |this, _, window, cx| this.select(id, window, cx)))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_size(px(14.))
                            .child(title),
                    )
                    .child(flags),
            )
            .child(
                h_flex().child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(th.muted_foreground)
                        .child(secondary),
                ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .text_size(px(11.))
                    .text_color(th.muted_foreground.opacity(0.8))
                    .child(relative_time(card.deleted_at.unwrap_or(card.updated_at)))
                    .child(tag_dots),
            );
        if !matches!(self.view, View::RecycleBin) {
            let accent = th.primary;
            item = item
                .drag_over::<cardfile_editor::DraggedBlock>(move |s, _, _, _| {
                    s.bg(accent.opacity(0.15))
                })
                .on_drop(cx.listener(
                    move |this, d: &cardfile_editor::DraggedBlock, window, cx| {
                        let copy = window.modifiers().control || window.modifiers().alt;
                        this.drop_block_on_card(d, id, copy, window, cx);
                    },
                ));
        }
        if manual {
            let primary = th.primary;
            item = item
                .on_drag(dragged, |d, _, _, cx| {
                    cx.new(|_| DragPreview(d.title.clone()))
                })
                .drag_over::<DraggedCard>(move |s, _, _, _| s.border_t_2().border_color(primary))
                .on_drop(cx.listener(move |this, d: &DraggedCard, window, cx| {
                    this.move_card(d.id, ix, window, cx)
                }));
        }
        let menu = CardMenu {
            ws: cx.entity(),
            id,
            deleted: matches!(self.view, View::RecycleBin),
            owner: card.access.is_owner(),
            pinned: card.is_pinned,
            favorite: card.is_favorite,
            can_share: card.access.is_owner()
                && AppState::get(cx).app.auth_mode == AuthMode::MultiUser,
        };
        item.context_menu(move |m, _, _| menu.build(m))
            .into_any_element()
    }
}

/// What the right-click menu of a card in the list needs to know.
#[derive(Clone)]
struct CardMenu {
    ws: Entity<Workspace>,
    id: CardId,
    deleted: bool,
    owner: bool,
    pinned: bool,
    favorite: bool,
    can_share: bool,
}

impl CardMenu {
    /// A menu item that selects the card and then runs `f` on the card pane.
    fn item(
        &self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut CardPane, &mut Window, &mut Context<CardPane>) + Clone + 'static,
    ) -> PopupMenuItem {
        let (ws, id) = (self.ws.clone(), self.id);
        PopupMenuItem::new(label).on_click(move |_, window, cx| {
            let f = f.clone();
            ws.update(cx, |ws, cx| ws.select_then(id, f, window, cx));
        })
    }

    fn build(&self, menu: PopupMenu) -> PopupMenu {
        use crate::export::Format;
        if self.deleted {
            return menu
                .item(self.item(t("card.restore"), |p, w, cx| p.restore(w, cx)))
                .separator()
                .item(self.item(t("card.purge.title"), |p, w, cx| p.purge(w, cx)));
        }
        let menu = if self.owner {
            menu.item(self.item(
                t(if self.pinned {
                    "card.unpin"
                } else {
                    "card.pin"
                }),
                |p, _, cx| p.toggle_pin(cx),
            ))
            .item(self.item(
                t(if self.favorite {
                    "card.unfavorite"
                } else {
                    "card.favorite"
                }),
                |p, _, cx| p.toggle_favorite(cx),
            ))
            .separator()
        } else {
            menu
        };
        let menu = if self.can_share {
            menu.item(self.item(t("share.button"), |p, w, cx| p.share(w, cx)))
        } else {
            menu
        };
        let menu = menu
            .item(self.item(t("card.history"), |p, w, cx| p.show_history(w, cx)))
            .separator()
            .item(self.item(t("export.markdown"), |p, w, cx| {
                p.export(Format::Markdown, w, cx)
            }))
            .item(self.item(t("export.html"), |p, w, cx| p.export(Format::Html, w, cx)))
            .item(self.item(t("export.pdf"), |p, w, cx| p.export(Format::Pdf, w, cx)))
            .item(self.item(t("export.json"), |p, w, cx| p.export(Format::Json, w, cx)));
        if self.owner {
            menu.separator()
                .item(self.item(t("card.delete"), |p, w, cx| p.delete(w, cx)))
        } else {
            menu
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.render_sidebar(cx);
        let list = self.render_list(cx);
        let settings = AppState::user_settings(cx);
        // The assistant exists only when an AI service is configured.
        let chat = (settings.ai_panel_open && crate::ai_settings::active_provider(cx).is_some())
            .then(|| self.ensure_chat(window, cx));
        let chat_width = settings.ai_panel_width.clamp(280., 640.);
        let border = cx.theme().border;
        h_flex()
            .id("workspace")
            .key_context(WORKSPACE)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(Self::new_card))
            .on_action(cx.listener(Self::delete_card))
            .on_action(cx.listener(Self::focus_search))
            .on_action(cx.listener(Self::toggle_pin))
            .on_action(cx.listener(Self::toggle_favorite))
            .on_action(cx.listener(Self::export_card))
            .on_action(cx.listener(Self::export_all))
            .on_action(cx.listener(Self::show_history))
            .on_action(cx.listener(Self::show_all))
            .on_action(cx.listener(Self::show_favorites))
            .on_action(cx.listener(Self::show_recycle))
            .on_action(cx.listener(Self::lock_app))
            .on_action(cx.listener(Self::sign_out))
            .on_action(cx.listener(Self::empty_recycle_bin))
            .on_action(cx.listener(Self::toggle_assistant))
            .on_action(cx.listener(Self::share_card))
            .on_action(cx.listener(Self::show_shared))
            .child(sidebar)
            .child(list)
            .child(div().flex_1().min_w_0().h_full().child(self.pane.clone()))
            .children(chat.map(|chat| {
                div()
                    .w(px(chat_width))
                    .flex_shrink_0()
                    .h_full()
                    .border_l_1()
                    .border_color(border)
                    .child(chat)
            }))
    }
}

#[allow(dead_code)]
fn _assert_window_ext(w: &mut Window, cx: &mut App) {
    let _ = w.has_active_dialog(cx);
}
