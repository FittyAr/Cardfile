//! The root view: chooses between the wizard, sign-in and the workspace.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use cardfile_core::{AuthMode, User};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseDownEvent, ParentElement as _, Render, ScrollWheelEvent, Styled as _,
    Subscription, Task, Window, canvas, div, px,
};

use crate::keymap::*;
use crate::login::{LoginEvent, LoginView};
use crate::state::{AppState, Session, run_db};
use crate::wizard::{Wizard, WizardEvent};
use crate::workspace::{Workspace, WorkspaceEvent};

enum Screen {
    /// Before the first screen is known (e.g. while the account loads).
    Loading,
    Wizard(Entity<Wizard>),
    Login(Entity<LoginView>),
    Workspace(Entity<Workspace>),
}

pub struct CardfileApp {
    screen: Screen,
    /// Last keystroke, click or scroll anywhere in the window (dialogs included).
    last_activity: Rc<Cell<Instant>>,
    _idle_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
    _screen_sub: Option<Subscription>,
}

impl CardfileApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Any keystroke counts as activity for the auto-lock timer.
        let subs = vec![
            cx.observe_keystrokes(|this, _, _, _| this.last_activity.set(Instant::now())),
            // Every quit path (menu, Ctrl+Q, the updater's restart) saves the open card
            // and waits (within the toolkit's shutdown timeout) for the write.
            cx.on_app_quit(|this, cx| {
                let saved = match &this.screen {
                    Screen::Workspace(ws) => ws.update(cx, |ws, cx| ws.flush(cx)),
                    _ => Task::ready(()),
                };
                let cleaned = cx.background_spawn(async {
                    crate::export::remove_session_attachments();
                });
                async move {
                    saved.await;
                    cleaned.await;
                }
            }),
        ];
        let mut app = Self {
            screen: Screen::Loading,
            last_activity: Rc::new(Cell::new(Instant::now())),
            _idle_task: None,
            _subs: subs,
            _screen_sub: None,
        };
        app.route(window, cx);
        app.start_idle_timer(cx);
        app
    }

    /// Decide the screen from the current state.
    fn route(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = AppState::get(cx);
        let db_exists = state.db.is_some();
        if !db_exists || !state.app.initialized {
            let wizard = cx.new(|cx| Wizard::new(!db_exists, window, cx));
            self._screen_sub = Some(cx.subscribe_in(
                &wizard,
                window,
                |this, _, ev: &WizardEvent, window, cx| match ev {
                    WizardEvent::Finished(session) => {
                        this.sign_in(Session::clone(session), window, cx)
                    }
                    WizardEvent::UseExisting => this.route(window, cx),
                },
            ));
            self.screen = Screen::Wizard(wizard);
            return;
        }
        match state.app.auth_mode {
            AuthMode::NoPassword => {
                let db = AppState::db(cx);
                let task = cx.background_spawn(async move {
                    let user = db.list_users().ok().and_then(|u| u.into_iter().next())?;
                    let settings = db.user_settings(user.id).unwrap_or_default();
                    Some(Session { user, settings })
                });
                cx.spawn_in(window, async move |this, cx| {
                    let session = task.await;
                    let _ = this.update_in(cx, |this, window, cx| match session {
                        Some(session) => this.sign_in(session, window, cx),
                        None => {
                            // Database without users: run the wizard again.
                            AppState::get_mut(cx).app.initialized = false;
                            this.route(window, cx);
                        }
                    });
                })
                .detach();
            }
            _ => self.show_login(None, window, cx),
        }
    }

    /// Leaving the workspace (lock, sign-out): close what it opened over the window,
    /// so no dialog (settings, history, share…) stays usable behind the sign-in.
    fn leave_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Workspace(_)) {
            return;
        }
        Workspace::set_current(None, cx);
        window.close_all_dialogs(cx);
        if window.has_active_sheet(cx) {
            window.close_sheet(cx);
        }
        // Decrypted attachments opened during the session.
        cx.background_spawn(async {
            crate::export::remove_session_attachments();
        })
        .detach();
    }

    fn show_login(&mut self, locked: Option<User>, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_workspace(window, cx);
        let login = cx.new(|cx| LoginView::new(locked, window, cx));
        self._screen_sub = Some(cx.subscribe_in(
            &login,
            window,
            |this, _, ev: &LoginEvent, window, cx| match ev {
                LoginEvent::SignedIn(session) => this.sign_in(session.clone(), window, cx),
            },
        ));
        self.screen = Screen::Login(login);
        cx.notify();
    }

    fn sign_in(&mut self, session: Session, window: &mut Window, cx: &mut Context<Self>) {
        crate::themes::apply(&session.settings.theme, cx);
        cardfile_i18n::set_locale(&session.settings.language);
        AppState::get_mut(cx).session = Some(session);
        // The MCP server is installation-wide; start it once the database is open.
        crate::mcp_host::McpServer::sync(cx);
        crate::ai_settings::load_keys(cx);
        crate::updater::cleanup(cx);
        let ws = cx.new(|cx| Workspace::new(window, cx));
        self._screen_sub = Some(cx.subscribe_in(
            &ws,
            window,
            |this, ws, ev: &WorkspaceEvent, window, cx| match ev {
                WorkspaceEvent::Lock => {
                    drop(ws.update(cx, |ws, cx| ws.flush(cx)));
                    let user = AppState::session(cx).map(|s| s.user.clone());
                    this.show_login(user, window, cx);
                }
                WorkspaceEvent::SignOut => {
                    drop(ws.update(cx, |ws, cx| ws.flush(cx)));
                    AppState::get_mut(cx).session = None;
                    this.show_login(None, window, cx);
                }
            },
        ));
        let focus = ws.focus_handle(cx);
        focus.focus(window, cx);
        Workspace::set_current(Some(&ws), cx);
        self.screen = Screen::Workspace(ws);
        self.last_activity.set(Instant::now());
        // `sign_in` also runs after every auto-lock unlock; check once per process.
        static UPDATE_CHECKED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !UPDATE_CHECKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            crate::updater::check_on_startup(window, cx);
        }
        cx.notify();
    }

    fn start_idle_timer(&mut self, cx: &mut Context<Self>) {
        self._idle_task = Some(cx.spawn_in_window_free(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                let ok = this.update(cx, |this, cx| this.check_idle(cx)).is_ok();
                if !ok {
                    break;
                }
            }
        }));
    }

    fn check_idle(&mut self, cx: &mut Context<Self>) {
        let Screen::Workspace(ws) = &self.screen else {
            return;
        };
        let state = AppState::get(cx);
        if state.app.auth_mode == AuthMode::NoPassword {
            return;
        }
        let secs = state
            .session
            .as_ref()
            .map_or(0, |s| s.settings.auto_lock_secs);
        if secs > 0 && self.last_activity.get().elapsed() >= Duration::from_secs(u64::from(secs)) {
            ws.update(cx, |ws, cx| ws.dispatch_lock(cx));
        }
    }

    // ---------------------------------------------------------------------
    // Global actions
    // ---------------------------------------------------------------------

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.screen, Screen::Workspace(_)) {
            crate::settings::open(window, cx);
        }
    }

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        // The open card is saved by the `on_app_quit` hook.
        cx.quit();
    }

    fn command_palette(&mut self, _: &CommandPalette, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.screen, Screen::Workspace(_)) {
            crate::palette::open(window, cx);
        }
    }

    fn open_keymap(&mut self, _: &OpenKeymap, window: &mut Window, cx: &mut Context<Self>) {
        let paths = AppState::get(cx).paths.clone();
        match crate::keymap::ensure_keymap_file(&paths, cx) {
            Ok(path) => {
                if let Err(e) = open::that_detached(&path) {
                    window.push_notification(Notification::error(e.to_string()), cx);
                }
            }
            Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
        }
    }

    fn open_data_folder(
        &mut self,
        _: &OpenDataFolder,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Open the folder itself: selecting the database in it (`reveal_path`) did
        // nothing on some systems.
        let root = AppState::get(cx).paths.root.clone();
        if let Err(e) = std::fs::create_dir_all(&root).and_then(|()| open::that_detached(&root)) {
            window.push_notification(Notification::error(e.to_string()), cx);
        }
    }

    fn backup_now(&mut self, _: &BackupNow, window: &mut Window, cx: &mut Context<Self>) {
        let db = AppState::db(cx);
        let dir = AppState::get(cx).paths.backups();
        let task = run_db(cx, move |_| db.backup(&dir));
        cx.spawn_in(window, async move |_, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| match result {
                Ok(path) => window.push_notification(
                    Notification::success(cardfile_i18n::t_args(
                        "backup.done",
                        &[("path", &path.display().to_string())],
                    )),
                    cx,
                ),
                Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
            });
        })
        .detach();
    }

    fn check_updates(&mut self, _: &CheckForUpdates, window: &mut Window, cx: &mut Context<Self>) {
        crate::updater::check_now(window, cx);
    }

    fn about(&mut self, _: &About, window: &mut Window, cx: &mut Context<Self>) {
        crate::settings::open_about(window, cx);
    }
}

impl Workspace {
    pub(crate) fn dispatch_lock(&mut self, cx: &mut Context<Self>) {
        drop(self.flush(cx));
        cx.emit(WorkspaceEvent::Lock);
    }
}

/// Save the open card, if any; the task finishes once the write landed (e.g. before
/// the updater starts the new version).
pub(crate) fn flush_all(cx: &mut App) -> Task<()> {
    match Workspace::current(cx) {
        Some(ws) => ws.update(cx, |ws, cx| ws.flush(cx)),
        None => Task::ready(()),
    }
}

trait SpawnExt {
    fn spawn_in_window_free<R: 'static>(
        &mut self,
        f: impl AsyncFnOnce(gpui_kit::WeakEntity<CardfileApp>, &mut gpui_kit::AsyncApp) -> R + 'static,
    ) -> Task<R>;
}

impl SpawnExt for Context<'_, CardfileApp> {
    fn spawn_in_window_free<R: 'static>(
        &mut self,
        f: impl AsyncFnOnce(gpui_kit::WeakEntity<CardfileApp>, &mut gpui_kit::AsyncApp) -> R + 'static,
    ) -> Task<R> {
        self.spawn(f)
    }
}

impl Render for CardfileApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.screen {
            Screen::Loading => div().into_any_element(),
            Screen::Wizard(v) => v.clone().into_any_element(),
            Screen::Login(v) => v.clone().into_any_element(),
            Screen::Workspace(v) => v.clone().into_any_element(),
        };
        let title_bar = self.render_title_bar(cx);
        let body = gpui_kit::component::v_flex()
            .size_full()
            .child(title_bar)
            .child(div().flex_1().min_h_0().child(content));
        // Activity anywhere in the window, including dialogs and sheets drawn over
        // this view, keeps the auto-lock timer from firing. Window-level listeners
        // are registered again on every paint.
        let activity = self.last_activity.clone();
        let watcher = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let a = activity.clone();
                window.on_mouse_event(move |_: &MouseDownEvent, _, _, _| a.set(Instant::now()));
                let a = activity.clone();
                window.on_mouse_event(move |_: &ScrollWheelEvent, _, _, _| a.set(Instant::now()));
                let a = activity;
                window.on_key_event(move |_: &KeyDownEvent, _, _, _| a.set(Instant::now()));
            },
        )
        .absolute()
        .size_full();
        gpui_kit::component::window_border().child(
            div()
                .id("cardfile")
                .size_full()
                .bg(cx.theme().background)
                .text_color(cx.theme().foreground)
                .font_family(cx.theme().font_family.clone())
                .child(watcher)
                .on_action(cx.listener(Self::open_settings))
                .on_action(cx.listener(Self::quit))
                .on_action(cx.listener(Self::command_palette))
                .on_action(cx.listener(Self::open_keymap))
                .on_action(cx.listener(Self::open_data_folder))
                .on_action(cx.listener(Self::backup_now))
                .on_action(cx.listener(Self::check_updates))
                .on_action(cx.listener(Self::about))
                .child(body),
        )
    }
}

impl CardfileApp {
    /// Cardfile's own title bar: app menu and title on the left, window controls
    /// (drawn by the toolkit on Windows and Linux) on the right.
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::button::{Button, ButtonVariants as _};
        use gpui_kit::component::menu::DropdownMenu as _;
        use gpui_kit::component::{Sizable as _, h_flex};

        let signed_in = matches!(self.screen, Screen::Workspace(_));
        let multi = AppState::get(cx).app.auth_mode != AuthMode::NoPassword;
        let user = AppState::session(cx)
            .filter(|_| signed_in && AppState::get(cx).app.auth_mode == AuthMode::MultiUser)
            .map(|s| s.user.display_name.clone());
        let title = match user {
            Some(u) => format!("Cardfile — {u}"),
            None => "Cardfile".to_owned(),
        };
        let menu = Button::new("app-menu")
            .ghost()
            .xsmall()
            .icon(gpui_kit::assets::IconName::Menu)
            .dropdown_menu(move |menu, _, _| {
                let mut menu = if signed_in {
                    menu.menu(cardfile_i18n::t("card.new"), Box::new(NewCard))
                        .menu(
                            cardfile_i18n::t("menu.command_palette"),
                            Box::new(CommandPalette),
                        )
                        .separator()
                        .menu(cardfile_i18n::t("menu.settings"), Box::new(OpenSettings))
                        .menu(cardfile_i18n::t("menu.keymap"), Box::new(OpenKeymap))
                        .separator()
                        .menu(cardfile_i18n::t("menu.export_all"), Box::new(ExportAll))
                        .menu(cardfile_i18n::t("menu.backup"), Box::new(BackupNow))
                        .menu(
                            cardfile_i18n::t("menu.open_data_folder"),
                            Box::new(OpenDataFolder),
                        )
                        .separator()
                } else {
                    menu
                };
                menu = menu
                    .menu(
                        cardfile_i18n::t("menu.check_updates"),
                        Box::new(CheckForUpdates),
                    )
                    .menu(cardfile_i18n::t("menu.about"), Box::new(About));
                if signed_in && multi {
                    menu = menu
                        .separator()
                        .menu(cardfile_i18n::t("menu.lock"), Box::new(LockApp))
                        .menu(cardfile_i18n::t("menu.sign_out"), Box::new(SignOut));
                }
                menu.separator()
                    .menu(cardfile_i18n::t("menu.quit"), Box::new(Quit))
            });
        gpui_kit::component::TitleBar::new().child(
            h_flex()
                .gap(px(6.))
                .child(menu)
                .child(gpui_kit::img("cardfile/icon.png").size(px(16.)))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(cx.theme().muted_foreground)
                        .child(title),
                ),
        )
    }
}
