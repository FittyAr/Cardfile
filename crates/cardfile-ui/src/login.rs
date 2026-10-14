//! Sign-in, registration and session unlock.

use cardfile_core::{AuthMode, User};
use cardfile_i18n::{t, t_args};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div, img, px,
};

use zeroize::Zeroizing;

use crate::state::{AppState, Session};
use crate::widgets::{error, field};

pub enum LoginEvent {
    SignedIn(Session),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    SignIn,
    Register,
}

pub struct LoginView {
    mode: Mode,
    /// Set when unlocking a locked session: only the password is asked.
    locked_user: Option<User>,
    username: Entity<InputState>,
    display_name: Entity<InputState>,
    password: Entity<InputState>,
    confirm: Entity<InputState>,
    error: Option<SharedString>,
    busy: bool,
    _subs: Vec<Subscription>,
}

impl EventEmitter<LoginEvent> for LoginView {}

impl LoginView {
    pub fn new(locked_user: Option<User>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let make =
            |placeholder: String, masked: bool, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .masked(masked)
                })
            };
        let username = make(t("login.username"), false, window, cx);
        let display_name = make(t("login.display_name"), false, window, cx);
        let password = make(t("login.password"), true, window, cx);
        let confirm = make(t("login.confirm"), true, window, cx);
        let mut subs = Vec::new();
        for input in [&username, &password, &confirm] {
            subs.push(
                cx.subscribe_in(input, window, |this, _, ev: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.submit(window, cx);
                    }
                }),
            );
        }
        let multi = AppState::get(cx).app.auth_mode == AuthMode::MultiUser;
        let focus = if multi && locked_user.is_none() {
            username.focus_handle(cx)
        } else {
            password.focus_handle(cx)
        };
        focus.focus(window, cx);
        Self {
            mode: Mode::SignIn,
            locked_user,
            username,
            display_name,
            password,
            confirm,
            error: None,
            busy: false,
            _subs: subs,
        }
    }

    fn submit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let db = AppState::db(cx);
        let auth = AppState::get(cx).app.auth_mode;
        let username = self.username.read(cx).value().trim().to_string();
        let display = self.display_name.read(cx).value().trim().to_string();
        let password = self.password.read(cx).value().to_string();
        let confirm = self.confirm.read(cx).value().to_string();
        let mode = self.mode;
        let locked = self.locked_user.clone();
        if mode == Mode::Register {
            if username.is_empty() {
                self.error = Some(t("wizard.error.username").into());
                cx.notify();
                return;
            }
            if password.chars().count() < 4 {
                self.error = Some(t("wizard.error.password_short").into());
                cx.notify();
                return;
            }
            if password != confirm {
                self.error = Some(t("wizard.error.password_mismatch").into());
                cx.notify();
                return;
            }
        }
        self.busy = true;
        self.error = None;
        cx.notify();
        let settings_db = db.clone();
        let lock_password = Zeroizing::new(password.clone());
        let task = cx.background_spawn(async move {
            let authenticate = move || -> anyhow::Result<Option<User>> {
                let user = match (mode, locked) {
                    (_, Some(user)) => Some(user),
                    (Mode::Register, None) => {
                        if db.user_by_username(&username)?.is_some() {
                            return Err(anyhow::anyhow!(t("login.error.taken")));
                        }
                        let hash = cardfile_crypto::hash_password(&password)?;
                        let display = if display.is_empty() {
                            username.clone()
                        } else {
                            display
                        };
                        return Ok(Some(db.create_user(
                            &username,
                            &display,
                            Some(&hash),
                            false,
                        )?));
                    }
                    (Mode::SignIn, None) if auth == AuthMode::MultiUser => {
                        db.user_by_username(&username)?
                    }
                    // Single-user: the only account.
                    (Mode::SignIn, None) => db.list_users()?.into_iter().next(),
                };
                let ok = user.as_ref().is_some_and(|u| match &u.password_hash {
                    Some(hash) => cardfile_crypto::verify_password(hash, &password),
                    None => true,
                });
                Ok(if ok { user } else { None })
            };
            // Settings are read here too, so the UI thread never touches the database.
            let Some(mut user) = authenticate()? else {
                return anyhow::Ok(None);
            };
            // When locks use the sign-in password, this is the moment to give users
            // from older versions a key pair (needed to receive shared encrypted cards).
            if user.lock_hash.is_none()
                && user.password_hash.is_some()
                && let Ok(Some(updated)) =
                    crate::locks::ensure_keys(&settings_db, &user, &lock_password)
            {
                user = updated;
            }
            let settings = settings_db.user_settings(user.id).unwrap_or_default();
            Ok(Some(Session { user, settings }))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(Some(session)) => cx.emit(LoginEvent::SignedIn(session)),
                    Ok(None) => this.error = Some(t("login.error.invalid").into()),
                    Err(e) => this.error = Some(e.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = AppState::get(cx).app.clone();
        let multi = app.auth_mode == AuthMode::MultiUser;
        let registering = self.mode == Mode::Register;
        let th = cx.theme();
        let title = match (&self.locked_user, registering) {
            (Some(u), _) => t_args("login.locked_title", &[("name", &u.display_name)]),
            (None, true) => t("login.register_title"),
            (None, false) => t("login.title"),
        };
        let mut form = v_flex().gap(px(12.)).w_full();
        if multi && self.locked_user.is_none() {
            form = form.child(field(t("login.username"), Input::new(&self.username), cx));
        }
        if registering {
            form = form.child(field(
                t("login.display_name"),
                Input::new(&self.display_name),
                cx,
            ));
        }
        form = form.child(field(
            t("login.password"),
            Input::new(&self.password).mask_toggle(),
            cx,
        ));
        if registering {
            form = form.child(field(
                t("login.confirm"),
                Input::new(&self.confirm).mask_toggle(),
                cx,
            ));
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(th.background)
            .text_color(th.foreground)
            .child(
                v_flex()
                    .w(px(360.))
                    .gap(px(16.))
                    .items_center()
                    .p(px(28.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(th.border)
                    .bg(th.popover)
                    .child(img("cardfile/logo.png").size(px(84.)))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(form)
                    .child(error(self.error.as_ref(), cx))
                    .child(
                        Button::new("login-submit")
                            .w_full()
                            .primary()
                            .loading(self.busy)
                            .label(if registering {
                                t("login.create")
                            } else if self.locked_user.is_some() {
                                t("login.unlock")
                            } else {
                                t("login.sign_in")
                            })
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    )
                    .when(
                        multi && app.allow_registration && self.locked_user.is_none(),
                        |d| {
                            d.child(
                                h_flex().child(
                                    Button::new("login-toggle")
                                        .ghost()
                                        .label(if registering {
                                            t("login.have_account")
                                        } else {
                                            t("login.need_account")
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.mode = if this.mode == Mode::SignIn {
                                                Mode::Register
                                            } else {
                                                Mode::SignIn
                                            };
                                            this.error = None;
                                            cx.notify();
                                        })),
                                ),
                            )
                        },
                    ),
            )
    }
}
