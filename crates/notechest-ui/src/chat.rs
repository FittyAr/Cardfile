//! The assistant panel: chat with the configured AI, which can search, edit, tag,
//! reorder and format cards through the same tools as the MCP server.
//!
//! The model runs on its own thread; its events reach the UI through a channel
//! drained by a short foreground timer.
//!
//! Tools that remove or overwrite data ([`notechest_agent::ToolDef::destructive`])
//! only run after the user allows them in the panel: card text is untrusted input
//! for the model, and a card could otherwise talk it into mass edits or deletions.
//! The assistant thread waits for the answer on a channel; the UI thread never
//! blocks.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Task, Window, div, px,
};
use notechest_agent::Scope;
use notechest_ai::{AgentEvent, Message, ProviderConfig, ToolSpec};
use notechest_core::{CardId, LockMode, mask_title};
use notechest_editor::{EditorEvent, RichEditor};
use notechest_i18n::{t, t_args};

use crate::ai_settings::{active_provider, providers};
use crate::state::AppState;

/// Most model ↔ tool round trips per request.
const MAX_STEPS: usize = 16;

/// Tool result sent to the model when the user does not allow a call.
const DECLINED: &str = "The user did not allow this action. Do not retry it unless the user asks.";

/// How often the assistant thread re-checks the cancel flag while it waits for the
/// user to allow or deny a tool call.
const APPROVAL_POLL: Duration = Duration::from_millis(100);

/// Where users report an inappropriate or harmful assistant answer. Opens a blank
/// issue form: nothing from the conversation is sent automatically.
const REPORT_URL: &str = "https://github.com/FittyAr/NoteChest/issues/new?labels=ai-content&title=Assistant%20response%20report";

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
    /// A translated message, with technical details shown smaller below it.
    Error {
        message: SharedString,
        detail: Option<SharedString>,
    },
}

impl Item {
    fn error(message: String, detail: Option<String>) -> Self {
        Self::Error {
            message: message.into(),
            detail: detail.map(Into::into),
        }
    }
}

/// A tool call waiting for the user's permission.
struct Approval {
    /// Tool name (for the translated label).
    name: String,
    /// What the call touches, e.g. the card title.
    detail: String,
    /// `true` runs the call, `false` refuses it. Dropping the sender refuses too.
    reply: mpsc::Sender<bool>,
}

enum Msg {
    Event(AgentEvent),
    Changed(Option<CardId>),
    Approve(Approval),
    Done(Vec<Message>, Option<Item>),
}

pub struct ChatPanel {
    items: Vec<Item>,
    history: Vec<Message>,
    /// One-line editor: unlike the toolkit's input it handles right-to-left text.
    input: Entity<RichEditor>,
    cancel: Option<Arc<AtomicBool>>,
    /// The tool call the assistant is waiting to have allowed, if any.
    pending: Option<Approval>,
    /// The user allowed every remaining call of this conversation.
    approve_all: Arc<AtomicBool>,
    poll: Option<Task<()>>,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl EventEmitter<ChatEvent> for ChatPanel {}

impl ChatPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut e = RichEditor::new(notechest_doc::Document::default(), cx);
            e.set_single_line(true);
            e.set_font_size(px(13.), cx);
            e.set_placeholder(Some(notechest_i18n::t_raw("chat.placeholder")));
            e
        });
        let subs =
            vec![
                cx.subscribe_in(&input, window, |this, _, ev: &EditorEvent, window, cx| {
                    if let EditorEvent::Submit = ev {
                        this.send(window, cx);
                    }
                }),
            ];
        Self {
            items: Vec::new(),
            history: Vec::new(),
            input,
            cancel: None,
            pending: None,
            approve_all: Arc::new(AtomicBool::new(false)),
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
        // A new conversation asks again before destructive changes.
        self.approve_all = Arc::new(AtomicBool::new(false));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.cancel_run();
        cx.notify();
    }

    /// Ask the assistant thread to stop and release it if it waits for an approval.
    fn cancel_run(&mut self) {
        if let Some(c) = &self.cancel {
            c.store(true, Ordering::Relaxed);
        }
        // Dropping the reply sender wakes the waiting thread (as a refusal).
        self.pending = None;
    }

    /// Answer the pending approval: `allow` runs the call, `all` also allows every
    /// later call of this conversation.
    fn decide(&mut self, allow: bool, all: bool, cx: &mut Context<Self>) {
        if let Some(p) = self.pending.take() {
            if allow && all {
                self.approve_all.store(true, Ordering::Relaxed);
            }
            let _ = p.reply.send(allow);
        }
        cx.notify();
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            return;
        }
        let text = self.input.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        let Some(cfg) = active_provider(cx) else {
            self.items.push(Item::error(t("chat.no_provider"), None));
            cx.notify();
            return;
        };
        let Some(session) = AppState::session(cx).cloned() else {
            return;
        };
        cx.emit(ChatEvent::BeforeRun);
        self.input.update(cx, |e, cx| e.set_text("", cx));
        self.items.push(Item::User(text.clone().into()));
        self.history.push(Message::user(text));
        let db = AppState::db(cx);
        let allow_private = session.settings.ai_chat_allow_private;
        let system = format!(
            "{}\n\nThe user is {}. Today is {}.",
            notechest_agent::INSTRUCTIONS,
            session.user.display_name,
            jiff::Zoned::now().strftime("%Y-%m-%d"),
        );
        let mut history = self.history.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let approve_all = self.approve_all.clone();
        let (tx, rx) = mpsc::channel::<Msg>();
        let spawned = std::thread::Builder::new()
            .name("notechest-assistant".into())
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
                        let error = Item::error(t("chat.error.start"), Some(e.to_string()));
                        let _ = tx.send(Msg::Done(history, Some(error)));
                        return;
                    }
                };
                let defs = notechest_agent::tools(&scope);
                let destructive: HashSet<&'static str> = defs
                    .iter()
                    .filter(|t| t.destructive)
                    .map(|t| t.name)
                    .collect();
                let tools: Vec<ToolSpec> = defs
                    .into_iter()
                    .map(|t| ToolSpec {
                        name: t.name.to_owned(),
                        description: t.description.to_owned(),
                        parameters: t.schema,
                    })
                    .collect();
                let tx_exec = tx.clone();
                let mut exec = |call: &notechest_ai::ToolCall| {
                    if destructive.contains(call.name.as_str())
                        && !approve_all.load(Ordering::Relaxed)
                    {
                        let (reply, answer) = mpsc::channel();
                        let ask = Approval {
                            name: call.name.clone(),
                            detail: describe_call(&db, &scope, &call.arguments),
                            reply,
                        };
                        if tx_exec.send(Msg::Approve(ask)).is_err()
                            || !wait_for_answer(&answer, &cancel)
                        {
                            return Err(DECLINED.to_owned());
                        }
                    }
                    let outcome = notechest_agent::call(&db, &scope, &call.name, &call.arguments)?;
                    if outcome.changed {
                        let _ = tx_exec.send(Msg::Changed(outcome.card));
                    }
                    Ok(outcome.value.to_string())
                };
                let tx_ev = tx.clone();
                let mut on_event = |ev| {
                    let _ = tx_ev.send(Msg::Event(ev));
                };
                let result = notechest_ai::run_agent(
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
                    Ok(notechest_ai::AgentOutcome::Finished) => None,
                    // The model still wanted to work: say so instead of stopping
                    // silently, so the user knows to ask it to continue.
                    Ok(notechest_ai::AgentOutcome::StepLimit) => {
                        Some(Item::error(t("chat.step_limit"), None))
                    }
                    Err(notechest_ai::AiError::Cancelled) => None,
                    Err(e) => Some(Item::error(t(ai_error_key(&e)), Some(e.to_string()))),
                };
                let _ = tx.send(Msg::Done(history, error));
            });
        if let Err(e) = spawned {
            self.cancel = None;
            self.items
                .push(Item::error(t("chat.error.start"), Some(e.to_string())));
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
                            Msg::Approve(ask) => this.pending = Some(ask),
                            Msg::Done(history, error) => {
                                this.history = history;
                                if let Some(e) = error {
                                    this.items.push(e);
                                }
                                this.cancel = None;
                                this.pending = None;
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
                            this.pending = None;
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
                let detail = if is_error && result == DECLINED {
                    t("chat.approval.declined")
                } else if is_error {
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
                        .child(notechest_editor::bidi::display(text)),
                )
                .into_any_element(),
            // Markdown views lay text out left to right only: show right-to-left
            // replies as plain lines in visual order instead.
            Item::Assistant(text) if notechest_i18n::has_rtl(text) => v_flex()
                .text_size(px(13.))
                .children(text.lines().map(|l| div().child(notechest_i18n::visual(l))))
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
            Item::Error { message, detail } => v_flex()
                .gap(px(2.))
                .text_size(px(12.))
                .text_color(th.danger)
                .child(message.clone())
                .when_some(detail.clone(), |d, detail| {
                    d.child(
                        div()
                            .text_size(px(11.))
                            .text_color(th.muted_foreground)
                            .child(detail),
                    )
                })
                .into_any_element(),
        }
    }

    /// The inline "allow this?" prompt for the pending tool call.
    fn render_approval(&self, ask: &Approval, cx: &Context<Self>) -> AnyElement {
        let th = cx.theme();
        let action = t(&format!("chat.tool.{}", ask.name));
        let what = if ask.detail.is_empty() {
            action
        } else {
            format!("{action} {}", ask.detail)
        };
        v_flex()
            .gap(px(8.))
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(th.warning)
            .bg(th.muted.opacity(0.5))
            .text_size(px(12.))
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(t("chat.approval.title")),
            )
            .child(div().child(what))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(
                        Button::new("chat-approve")
                            .xsmall()
                            .primary()
                            .label(t("chat.approval.approve"))
                            .on_click(cx.listener(|this, _, _, cx| this.decide(true, false, cx))),
                    )
                    .child(
                        Button::new("chat-approve-all")
                            .xsmall()
                            .outline()
                            .label(t("chat.approval.approve_all"))
                            .on_click(cx.listener(|this, _, _, cx| this.decide(true, true, cx))),
                    )
                    .child(
                        Button::new("chat-reject")
                            .xsmall()
                            .ghost()
                            .label(t("chat.approval.reject"))
                            .on_click(cx.listener(|this, _, _, cx| this.decide(false, false, cx))),
                    ),
            )
            .into_any_element()
    }
}

impl Drop for ChatPanel {
    /// Closing the panel (or the window) stops a running assistant instead of
    /// leaving its thread calling the model and changing cards in the background.
    fn drop(&mut self) {
        self.cancel_run();
    }
}

/// Block the assistant thread until the user answers an approval. `false` when the
/// user refused, the panel went away or the run was cancelled.
fn wait_for_answer(answer: &mpsc::Receiver<bool>, cancel: &AtomicBool) -> bool {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        match answer.recv_timeout(APPROVAL_POLL) {
            Ok(allow) => return allow,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
        }
    }
}

/// What a tool call touches, for the approval prompt: the card title(s) when the
/// call names cards, otherwise a short trace of its arguments. Private titles are
/// masked like in the card list. Runs on the assistant thread (database access).
fn describe_call(db: &notechest_db::Database, scope: &Scope, args: &serde_json::Value) -> String {
    let title = |raw: &str| -> Option<String> {
        let id: CardId = raw.trim().parse().ok()?;
        db.card_access(id, scope.principal).ok()??;
        let card = db.card(id).ok()?;
        Some(if card.lock_mode == LockMode::Private {
            mask_title(&card.title, 3)
        } else {
            card.title
        })
    };
    if let Some(t) = args.get("id").and_then(|v| v.as_str()).and_then(title) {
        let t: String = t.chars().take(60).collect();
        return format!("“{t}”");
    }
    if let Some(ids) = args.get("ids").and_then(|v| v.as_array()) {
        let names: Vec<String> = ids
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(title)
            .take(5)
            .map(|t| format!("“{}”", t.chars().take(40).collect::<String>()))
            .collect();
        let mut out = names.join(", ");
        if ids.len() > names.len() {
            let rest = (ids.len() - names.len()).to_string();
            out.push(' ');
            out.push_str(&t_args("chat.approval.more", &[("count", &rest)]));
        }
        return out;
    }
    summarize_args(args)
}

/// The translation key that explains `e` to the user.
fn ai_error_key(e: &notechest_ai::AiError) -> &'static str {
    use notechest_ai::AiError;
    match e {
        AiError::Network(_) => "chat.error.network",
        AiError::Timeout => "chat.error.timeout",
        AiError::Http {
            status: 401 | 403, ..
        }
        | AiError::MissingApiKey => "chat.error.auth",
        AiError::InsecureUrl => "chat.error.insecure_url",
        _ => "chat.error.generic",
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
                        // Store policy 11.16: users can report generated content.
                        Button::new("chat-report")
                            .xsmall()
                            .ghost()
                            .label(t("chat.report"))
                            .tooltip(t("chat.report_tooltip"))
                            .on_click(|_, _, cx| cx.open_url(REPORT_URL)),
                    )
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
        if let Some(ask) = &self.pending {
            list = list.child(self.render_approval(ask, cx));
        } else if self.running() {
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
                    .h(px(30.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(th.input)
                    .when(running, |d| d.opacity(0.5))
                    .child(self.input.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use notechest_ai::AiError;
    use serde_json::json;

    #[test]
    fn summary_prefers_the_first_known_non_empty_field() {
        assert_eq!(summarize_args(&json!({})), "");
        assert_eq!(
            summarize_args(&json!({"title": "", "query": "rust"})),
            "“rust”"
        );
        assert_eq!(
            summarize_args(&json!({"query": "q", "title": "Groceries"})),
            "“Groceries”"
        );
        assert_eq!(summarize_args(&json!({"tags": ["a", 1, "b"]})), "a, b");
        assert_eq!(summarize_args(&json!({"tags": [], "name": "x"})), "“x”");
        assert_eq!(summarize_args(&json!({"other": "ignored"})), "");
        assert_eq!(summarize_args(&json!("not an object")), "");
    }

    #[test]
    fn summary_truncates_by_characters_not_bytes() {
        let long = "ñ".repeat(80);
        let out = summarize_args(&json!({ "title": long }));
        assert_eq!(out.chars().count(), 62);
        assert_eq!(out, format!("“{}”", "ñ".repeat(60)));
    }

    #[test]
    fn auth_failures_map_to_the_auth_message() {
        let http = |status| AiError::Http {
            status,
            message: String::new(),
        };
        assert_eq!(ai_error_key(&http(401)), "chat.error.auth");
        assert_eq!(ai_error_key(&http(403)), "chat.error.auth");
        assert_eq!(ai_error_key(&AiError::MissingApiKey), "chat.error.auth");
        assert_eq!(ai_error_key(&http(500)), "chat.error.generic");
        assert_eq!(ai_error_key(&http(404)), "chat.error.generic");
    }

    #[test]
    fn transport_failures_have_their_own_messages() {
        assert_eq!(
            ai_error_key(&AiError::Network("reset".into())),
            "chat.error.network"
        );
        assert_eq!(ai_error_key(&AiError::Timeout), "chat.error.timeout");
        assert_eq!(
            ai_error_key(&AiError::InsecureUrl),
            "chat.error.insecure_url"
        );
        assert_eq!(
            ai_error_key(&AiError::Decode("bad".into())),
            "chat.error.generic"
        );
    }
}
