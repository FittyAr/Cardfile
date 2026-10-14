//! The root view: chooses between the wizard, sign-in and the workspace.

use std::time::{Duration, Instant};

use cardfile_core::{AuthMode, User};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Task, Window, div,
};

use crate::keymap::*;
use crate::login::{LoginEvent, LoginView};
use crate::state::{AppState, Session, run_db};
use crate::wizard::{Wizard, WizardEvent};
use crate::workspace::{Workspace, WorkspaceEvent};

enum Screen {
    Wizard(Entity<Wizard>),
    Login(Entity<LoginView>),
    Workspace(Entity<Workspace>),
}

pub struct CardfileApp {
    screen: Screen,
    last_activity: Instant,
    _idle_task: Option<Task<()>>,
    _subs: Vec<Subscription>,
    _screen_sub: Option<Subscription>,
}

impl CardfileApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Any keystroke counts as activity for the auto-lock timer.
        let subs = vec![cx.observe_keystrokes(|this, _, _, _| this.last_activity = Instant::now())];
        let placeholder = Screen::Wizard(cx.new(|cx| Wizard::new(false, window, cx)));
        let mut app = Self {
            screen: placeholder,
            last_activity: Instant::now(),
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
                    WizardEvent::Finished(session) => this.sign_in(session.clone(), window, cx),
                },
            ));
            self.screen = Screen::Wizard(wizard);
            return;
        }
        match state.app.auth_mode {
            AuthMode::NoPassword => {
                let db = AppState::db(cx);
                let user = db.list_users().ok().and_then(|u| u.into_iter().next());
                match user {
                    Some(user) => {
                        let settings = db.user_settings(user.id).unwrap_or_default();
                        self.sign_in(Session { user, settings }, window, cx);
                    }
                    None => {
                        // Database without users: run the wizard again.
                        AppState::get_mut(cx).app.initialized = false;
                        self.route(window, cx);
                    }
                }
            }
            _ => self.show_login(None, window, cx),
        }
    }

    fn show_login(&mut self, locked: Option<User>, window: &mut Window, cx: &mut Context<Self>) {
        AppState::get_mut(cx).card_password = None;
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
        let ws = cx.new(|cx| Workspace::new(window, cx));
        self._screen_sub = Some(cx.subscribe_in(
            &ws,
            window,
            |this, ws, ev: &WorkspaceEvent, window, cx| match ev {
                WorkspaceEvent::Lock => {
                    ws.update(cx, |ws, cx| ws.flush(cx));
                    let user = AppState::session(cx).map(|s| s.user.clone());
                    this.show_login(user, window, cx);
                }
                WorkspaceEvent::SignOut => {
                    ws.update(cx, |ws, cx| ws.flush(cx));
                    AppState::get_mut(cx).session = None;
                    this.show_login(None, window, cx);
                }
            },
        ));
        let focus = ws.focus_handle(cx);
        focus.focus(window, cx);
        self.screen = Screen::Workspace(ws);
        self.last_activity = Instant::now();
        crate::updater::check_on_startup(window, cx);
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
        if secs > 0 && self.last_activity.elapsed() >= Duration::from_secs(u64::from(secs)) {
            ws.update(cx, |ws, cx| ws.dispatch_lock(cx));
        }
    }

    // ---------------------------------------------------------------------
    // Global actions
    // ---------------------------------------------------------------------

    fn open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Workspace(ws) = &self.screen {
            crate::settings::open(ws.clone(), window, cx);
        }
    }

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        if let Screen::Workspace(ws) = &self.screen {
            ws.update(cx, |ws, cx| ws.flush(cx));
        }
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

    fn open_data_folder(&mut self, _: &OpenDataFolder, _: &mut Window, cx: &mut Context<Self>) {
        let root = AppState::get(cx).paths.root.clone();
        cx.reveal_path(&root.join("cardfile.db"));
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
        self.flush(cx);
        cx.emit(WorkspaceEvent::Lock);
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
            Screen::Wizard(v) => v.clone().into_any_element(),
            Screen::Login(v) => v.clone().into_any_element(),
            Screen::Workspace(v) => v.clone().into_any_element(),
        };
        div()
            .id("cardfile")
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .on_any_mouse_down(cx.listener(|this, _, _, _| this.last_activity = Instant::now()))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::command_palette))
            .on_action(cx.listener(Self::open_keymap))
            .on_action(cx.listener(Self::open_data_folder))
            .on_action(cx.listener(Self::backup_now))
            .on_action(cx.listener(Self::check_updates))
            .on_action(cx.listener(Self::about))
            .child(content)
    }
}
