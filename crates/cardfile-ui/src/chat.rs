//! The assistant panel: chat with the configured AI, which can search, edit, tag,
//! reorder and format cards through the same tools as the MCP server.
//!
//! The model runs on its own thread; its events reach the UI through a channel
//! drained by a short foreground timer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use cardfile_agent::Scope;
use cardfile_ai::{AgentEvent, Message, ProviderConfig, ToolSpec};
use cardfile_core::CardId;
use cardfile_i18n::t;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, px,
};

use crate::ai_settings::{active_provider, providers};
use crate::state::AppState;

/// Most model ↔ tool round trips per request.
const MAX_STEPS: usize = 16;

pub enum ChatEvent {
    /// About to run: unsaved edits should be written first.
    BeforeRun,
    /// The AI changed data; `card` is the card it touched last.
    DataChanged(Option<CardId>),
    Close,
}

enum Item {
    User(SharedString),
    Assistant(SharedString),
    Tool {
        name: String,
        ok: bool,
        detail: String,
    },
    Error(SharedString),
}

enum Msg {
    Event(AgentEvent),
    Changed(Option<CardId>),
    Done(Vec<Message>, Option<String>),
}

pub struct ChatPanel {
    items: Vec<Item>,
    history: Vec<Message>,
    input: Entity<InputState>,
    cancel: Option<Arc<AtomicBool>>,
    poll: Option<Task<()>>,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ChatEvent> for ChatPanel {}

impl ChatPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(t("chat.placeholder")));
        let subs = vec![
            cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = ev {
                    this.send(window, cx);
                }
            }),
        ];
        Self {
            items: Vec::new(),
            history: Vec::new(),
            input,
            cancel: None,
            poll: None,
            scroll: ScrollHandle::new(),
            _subs: subs,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.input.focus_handle(cx);
        focus.focus(window, cx);
    }

    fn running(&self) -> bool {
        self.cancel.is_some()
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        self.items.clear();
        self.history.clear();
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.cancel {
            c.store(true, Ordering::Relaxed);
        }
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        let text = self.input.read(cx).value().trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(cfg) = active_provider(cx) else {
            self.items.push(Item::Error(t("chat.no_provider").into()));
            cx.notify();
            return;
        };
        let Some(session) = AppState::session(cx).cloned() else {
            return;
        };
        cx.emit(ChatEvent::BeforeRun);
        self.input.update(cx, |s, cx| s.set_value("", window, cx));
        self.items.push(Item::User(text.clone().into()));
        self.history.push(Message::user(text));
        let db = AppState::db(cx);
        let allow_private = session.settings.ai_chat_allow_private;
        let system = format!(
            "{}\n\nThe user is {}. Today is {}.",
            cardfile_agent::INSTRUCTIONS,
            session.user.display_name,
            jiff::Zoned::now().strftime("%Y-%m-%d"),
        );
        let mut history = self.history.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let (tx, rx) = mpsc::channel::<Msg>();
        let spawned = std::thread::Builder::new()
            .name("cardfile-assistant".into())
            .spawn(move || {
                let scope = match db.ai_user() {
                    Ok(ai) => Scope {
                        principal: session.user.id,
                        ai: ai.id,
                        via: "chat".into(),
                        allow_private,
                        read_only: false,
                    },
                    Err(e) => {
                        let _ = tx.send(Msg::Done(history, Some(e.to_string())));
                        return;
                    }
                };
                let tools: Vec<ToolSpec> = cardfile_agent::tools(&scope)
                    .into_iter()
                    .map(|t| ToolSpec {
                        name: t.name.to_owned(),
                        description: t.description.to_owned(),
                        parameters: t.schema,
                    })
                    .collect();
                let tx_exec = tx.clone();
                let mut exec = |call: &cardfile_ai::ToolCall| {
                    let outcome = cardfile_agent::call(&db, &scope, &call.name, &call.arguments)?;
                    if outcome.changed {
                        let _ = tx_exec.send(Msg::Changed(outcome.card));
                    }
                    Ok(outcome.value.to_string())
                };
                let tx_ev = tx.clone();
                let mut on_event = |ev| {
                    let _ = tx_ev.send(Msg::Event(ev));
                };
                let result = cardfile_ai::run_agent(
                    &cfg,
                    &system,
                    &mut history,
                    &tools,
                    &mut exec,
                    &mut on_event,
                    MAX_STEPS,
                    &cancel,
                );
                let error = match result {
                    Ok(()) => None,
                    Err(cardfile_ai::AiError::Cancelled) => None,
                    Err(e) => Some(e.to_string()),
                };
                let _ = tx.send(Msg::Done(history, error));
            });
        if let Err(e) = spawned {
            self.cancel = None;
            self.items.push(Item::Error(e.to_string().into()));
            cx.notify();
            return;
        }
        self.poll = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let mut done = false;
                let mut changed: Option<Option<CardId>> = None;
                let mut batch = Vec::new();
                loop {
                    match rx.try_recv() {
                        Ok(msg) => batch.push(msg),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            done = true;
                            break;
                        }
                    }
                }
                let alive = this.update(cx, |this, cx| {
                    for msg in batch {
                        match msg {
                            Msg::Event(ev) => this.on_event(ev),
                            Msg::Changed(card) => {
                                changed = Some(card.or(changed.flatten()));
                            }
                            Msg::Done(history, error) => {
                                this.history = history;
                                if let Some(e) = error {
                                    this.items.push(Item::Error(e.into()));
                                }
                                this.cancel = None;
                                done = true;
                            }
                        }
                    }
                    if let Some(card) = changed {
                        cx.emit(ChatEvent::DataChanged(card));
                    }
                    this.scroll.scroll_to_bottom();
                    cx.notify();
                });
                if alive.is_err() || done {
                    if alive.is_ok() {
                        let _ = this.update(cx, |this, cx| {
                            this.cancel = None;
                            cx.notify();
                        });
                    }
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn on_event(&mut self, ev: AgentEvent) {
        match ev {
            AgentEvent::AssistantText(text) if !text.trim().is_empty() => {
                self.items.push(Item::Assistant(text.into()));
            }
            AgentEvent::AssistantText(_) => {}
            AgentEvent::ToolCall(_) => {}
            AgentEvent::ToolResult {
                call,
                result,
                is_error,
            } => {
                let detail = if is_error {
                    result
                } else {
                    summarize_args(&call.arguments)
                };
                self.items.push(Item::Tool {
                    name: call.name,
                    ok: !is_error,
                    detail,
                });
            }
        }
    }

    fn render_item(&self, ix: usize, item: &Item, cx: &Context<Self>) -> AnyElement {
        let th = cx.theme();
        match item {
            Item::User(text) => h_flex()
                .justify_end()
                .child(
                    div()
                        .max_w(px(280.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(10.))
                        .bg(th.primary)
                        .text_color(th.primary_foreground)
                        .text_size(px(13.))
                        .child(cardfile_editor::bidi::display(text)),
                )
                .into_any_element(),
            // Markdown views lay text out left to right only: show right-to-left
            // replies as plain lines in visual order instead.
            Item::Assistant(text) if cardfile_i18n::has_rtl(text) => v_flex()
                .text_size(px(13.))
                .children(text.lines().map(|l| div().child(cardfile_i18n::visual(l))))
                .into_any_element(),
            Item::Assistant(text) => div()
                .text_size(px(13.))
                .child(TextView::markdown(("chat-md", ix), text.clone()).selectable(true))
                .into_any_element(),
            Item::Tool { name, ok, detail } => h_flex()
                .gap(px(6.))
                .text_size(px(11.))
                .text_color(if *ok { th.muted_foreground } else { th.danger })
                .child(gpui_kit::component::Icon::new(IconName::Sparkles).size_3())
                .child(
                    div()
                        .truncate()
                        .child(format!("{} {detail}", t(&format!("chat.tool.{name}")))),
                )
                .into_any_element(),
            Item::Error(e) => div()
                .text_size(px(12.))
                .text_color(th.danger)
                .child(e.clone())
                .into_any_element(),
        }
    }
}

/// A short, human-readable trace of a tool call's arguments.
fn summarize_args(args: &serde_json::Value) -> String {
    let pick = ["title", "query", "find", "name", "tags"];
    for key in pick {
        match args.get(key) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => {
                let s: String = s.chars().take(60).collect();
                return format!("“{s}”");
            }
            Some(serde_json::Value::Array(a)) if !a.is_empty() => {
                let names: Vec<&str> = a.iter().filter_map(|v| v.as_str()).collect();
                return names.join(", ");
            }
            _ => {}
        }
    }
    String::new()
}

impl Render for ChatPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = cx.theme();
        let active = active_provider(cx);
        let all: Vec<ProviderConfig> = providers(cx)
            .into_iter()
            .filter(|p| p.is_usable())
            .collect();
        let active_id = active.as_ref().map(|p| p.id.clone());
        let provider_menu = Button::new("chat-provider")
            .xsmall()
            .ghost()
            .label(active.as_ref().map(|p| p.name.clone()).unwrap_or_default())
            .dropdown_caret(all.len() > 1)
            .dropdown_menu(move |mut menu, _, _| {
                for p in &all {
                    let id = p.id.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{} · {}", p.name, p.model))
                            .checked(active_id.as_deref() == Some(p.id.as_str()))
                            .on_click(move |_, _, cx| {
                                let id = id.clone();
                                AppState::update_user_settings(cx, |s| {
                                    s.ai_active_provider = Some(id)
                                });
                                cx.refresh_windows();
                            }),
                    );
                }
                menu
            });
        let header = h_flex()
            .justify_between()
            .px(px(12.))
            .py(px(8.))
            .border_b_1()
            .border_color(th.border)
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(gpui_kit::component::Icon::new(IconName::Bot).size_4())
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(t("chat.title")),
                    )
                    .child(provider_menu),
            )
            .child(
                h_flex()
                    .gap(px(2.))
                    .child(
                        Button::new("chat-clear")
                            .xsmall()
                            .ghost()
                            .icon(IconName::Trash)
                            .tooltip(t("chat.clear"))
                            .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                    )
                    .child(
                        Button::new("chat-close")
                            .xsmall()
                            .ghost()
                            .icon(IconName::PanelRightClose)
                            .tooltip(t("chat.close"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(ChatEvent::Close))),
                    ),
            );
        let mut list = v_flex().gap(px(10.)).p(px(12.));
        if self.items.is_empty() {
            list = list.child(
                v_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(th.muted_foreground)
                    .child(t("chat.empty"))
                    .child(t("chat.example1"))
                    .child(t("chat.example2"))
                    .child(t("chat.example3")),
            );
        }
        for (ix, item) in self.items.iter().enumerate() {
            list = list.child(self.render_item(ix, item, cx));
        }
        if self.running() {
            list = list.child(
                div()
                    .text_size(px(12.))
                    .text_color(th.muted_foreground)
                    .child(t("chat.thinking")),
            );
        }
        let running = self.running();
        let footer = h_flex()
            .gap(px(6.))
            .p(px(10.))
            .border_t_1()
            .border_color(th.border)
            .child(
                div()
                    .flex_1()
                    .child(Input::new(&self.input).disabled(running)),
            )
            .child(if running {
                Button::new("chat-stop")
                    .small()
                    .icon(IconName::CircleStop)
                    .tooltip(t("chat.stop"))
                    .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
            } else {
                Button::new("chat-send")
                    .small()
                    .primary()
                    .icon(IconName::SendHorizontal)
                    .tooltip(t("chat.send"))
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
            });
        v_flex()
            .size_full()
            .bg(th.background)
            .child(header)
            .child(
                div()
                    .id("chat-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(list),
            )
            .child(footer)
    }
}
