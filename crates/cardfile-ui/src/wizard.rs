//! First-run wizard: storage location, sign-in mode, card protection and theme.

use cardfile_core::{AppSettings, AuthMode, LockMode, NewCard, UserSettings};
use cardfile_db::Database;
use cardfile_i18n::t;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, img, px,
};

use crate::paths::DataPaths;
use crate::state::{AppState, Session};
use crate::widgets::{error, field, hint};

pub enum WizardEvent {
    Finished(Session),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Welcome,
    Storage,
    Security,
    Protection,
    Appearance,
    Finish,
}

pub struct Wizard {
    steps: Vec<Step>,
    step: usize,
    portable: bool,
    auth: AuthMode,
    allow_registration: bool,
    lock_modes: Vec<LockMode>,
    default_lock: LockMode,
    theme: SharedString,
    username: Entity<InputState>,
    display_name: Entity<InputState>,
    password: Entity<InputState>,
    confirm: Entity<InputState>,
    error: Option<SharedString>,
    busy: bool,
}

impl EventEmitter<WizardEvent> for Wizard {}

impl Wizard {
    /// `ask_storage` is false when a database already exists.
    pub fn new(ask_storage: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut steps = vec![Step::Welcome];
        if ask_storage {
            steps.push(Step::Storage);
        }
        steps.extend([
            Step::Security,
            Step::Protection,
            Step::Appearance,
            Step::Finish,
        ]);
        let input =
            |placeholder: String, masked: bool, window: &mut Window, cx: &mut Context<Self>| {
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .masked(masked)
                })
            };
        let current_user = whoami_name();
        Self {
            steps,
            step: 0,
            portable: AppState::get(cx).paths.portable,
            auth: AuthMode::NoPassword,
            allow_registration: false,
            lock_modes: LockMode::ALL[1..].to_vec(),
            default_lock: LockMode::None,
            theme: crate::themes::DEFAULT_THEME.into(),
            username: {
                let s = input(t("wizard.security.username"), false, window, cx);
                s.update(cx, |s, cx| s.set_value(current_user.clone(), window, cx));
                s
            },
            display_name: {
                let s = input(t("wizard.security.display_name"), false, window, cx);
                s.update(cx, |s, cx| s.set_value(current_user, window, cx));
                s
            },
            password: input(t("wizard.security.password"), true, window, cx),
            confirm: input(t("wizard.security.confirm"), true, window, cx),
            error: None,
            busy: false,
        }
    }

    fn current(&self) -> Step {
        self.steps[self.step]
    }

    fn validate(&mut self, cx: &mut Context<Self>) -> bool {
        self.error = None;
        if self.current() == Step::Security {
            let pw = self.password.read(cx).value().to_string();
            let confirm = self.confirm.read(cx).value().to_string();
            let user = self.username.read(cx).value().trim().to_string();
            let needs_pw = self.auth != AuthMode::NoPassword;
            if self.auth == AuthMode::MultiUser && user.is_empty() {
                self.error = Some(t("wizard.error.username").into());
            } else if needs_pw && pw.chars().count() < 4 {
                self.error = Some(t("wizard.error.password_short").into());
            } else if (needs_pw || !pw.is_empty()) && pw != confirm {
                self.error = Some(t("wizard.error.password_mismatch").into());
            } else if !pw.is_empty() && pw.chars().count() < 4 {
                self.error = Some(t("wizard.error.password_short").into());
            }
        }
        self.error.is_none()
    }

    fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.validate(cx) {
            cx.notify();
            return;
        }
        if self.step + 1 < self.steps.len() {
            self.step += 1;
        } else {
            self.finish(window, cx);
        }
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.step = self.step.saturating_sub(1);
        cx.notify();
    }

    fn finish(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let paths = if self.portable && !AppState::get(cx).paths.portable {
            match DataPaths::make_portable() {
                Ok(p) => p,
                Err(e) => {
                    self.error = Some(format!("{}: {e}", t("wizard.error.portable")).into());
                    self.busy = false;
                    return;
                }
            }
        } else {
            AppState::get(cx).paths.clone()
        };
        let app = AppSettings {
            initialized: true,
            auth_mode: self.auth,
            enabled_lock_modes: self.lock_modes.clone(),
            allow_registration: self.allow_registration && self.auth == AuthMode::MultiUser,
        };
        let password = self.password.read(cx).value().to_string();
        let mut username = self.username.read(cx).value().trim().to_string();
        let mut display = self.display_name.read(cx).value().trim().to_string();
        if username.is_empty() {
            username = whoami_name();
        }
        if display.is_empty() {
            display = username.clone();
        }
        let auth = self.auth;
        let settings = UserSettings {
            theme: self.theme.to_string(),
            default_lock_mode: self.default_lock,
            ..UserSettings::default()
        };
        let existing_db = AppState::get(cx).db.clone();
        let task = cx.background_spawn(async move {
            let db = match existing_db {
                Some(db) => db,
                None => Database::open(&paths.db(), Some(&paths.backups()))?,
            };
            let mut settings = settings;
            let hash = if password.is_empty() {
                None
            } else {
                Some(cardfile_crypto::hash_password(&password)?)
            };
            let account_hash = if auth == AuthMode::NoPassword {
                None
            } else {
                hash.clone()
            };
            if auth == AuthMode::NoPassword {
                settings.lock_password_hash = hash;
            }
            let user = db.create_user(&username, &display, account_hash.as_deref(), true)?;
            db.save_user_settings(user.id, &settings)?;
            db.save_app_settings(&app)?;
            let welcome = crate::welcome::welcome_document();
            db.create_card(
                user.id,
                NewCard {
                    title: t("welcome.title"),
                    plain_text: cardfile_doc::to_plain_text(&welcome),
                    content: welcome.to_json(),
                    lock_mode: LockMode::None,
                },
            )?;
            anyhow::Ok((paths, db, app, Session { user, settings }))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok((paths, db, app, session)) => {
                        let state = AppState::get_mut(cx);
                        state.paths = paths;
                        state.db = Some(db);
                        state.app = app;
                        cx.emit(WizardEvent::Finished(session));
                    }
                    Err(e) => {
                        tracing::error!("wizard: {e:#}");
                        this.error = Some(format!("{e}").into());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn option_card(
        id: &'static str,
        title: String,
        desc: String,
        selected: bool,
        cx: &Context<Self>,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        let th = cx.theme();
        div()
            .id(id)
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(if selected { th.primary } else { th.border })
            .when(selected, |d| d.bg(th.primary.opacity(0.08)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                on_click(this, cx);
                cx.notify();
            }))
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(th.muted_foreground)
                    .child(desc),
            )
    }

    fn render_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let _ = window;
        match self.current() {
            Step::Welcome => v_flex()
                .gap(px(16.))
                .items_center()
                .child(img("cardfile/logo.png").size(px(128.)))
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .child(t("wizard.welcome.title")),
                )
                .child(
                    div()
                        .text_center()
                        .max_w(px(440.))
                        .text_color(cx.theme().muted_foreground)
                        .child(t("wizard.welcome.body")),
                )
                .into_any_element(),
            Step::Storage => {
                let portable_ok = DataPaths::portable_available();
                let standard_path = DataPaths::standard().root.display().to_string();
                v_flex()
                    .gap(px(10.))
                    .child(Self::option_card(
                        "storage-standard",
                        t("wizard.storage.standard"),
                        format!("{} {}", t("wizard.storage.standard_desc"), standard_path),
                        !self.portable,
                        cx,
                        |this, _| this.portable = false,
                    ))
                    .when(portable_ok, |d| {
                        d.child(Self::option_card(
                            "storage-portable",
                            t("wizard.storage.portable"),
                            t("wizard.storage.portable_desc"),
                            self.portable,
                            cx,
                            |this, _| this.portable = true,
                        ))
                    })
                    .when(!portable_ok, |d| {
                        d.child(hint(t("wizard.storage.portable_unavailable"), cx))
                    })
                    .into_any_element()
            }
            Step::Security => {
                let mut col = v_flex()
                    .gap(px(10.))
                    .child(Self::option_card(
                        "auth-none",
                        t("wizard.security.none"),
                        t("wizard.security.none_desc"),
                        self.auth == AuthMode::NoPassword,
                        cx,
                        |this, _| this.auth = AuthMode::NoPassword,
                    ))
                    .child(Self::option_card(
                        "auth-single",
                        t("wizard.security.single"),
                        t("wizard.security.single_desc"),
                        self.auth == AuthMode::SingleUser,
                        cx,
                        |this, _| this.auth = AuthMode::SingleUser,
                    ))
                    .child(Self::option_card(
                        "auth-multi",
                        t("wizard.security.multi"),
                        t("wizard.security.multi_desc"),
                        self.auth == AuthMode::MultiUser,
                        cx,
                        |this, _| this.auth = AuthMode::MultiUser,
                    ));
                if self.auth == AuthMode::MultiUser {
                    col = col.child(
                        h_flex()
                            .gap(px(8.))
                            .child(div().flex_1().child(field(
                                t("wizard.security.username"),
                                Input::new(&self.username),
                                cx,
                            )))
                            .child(div().flex_1().child(field(
                                t("wizard.security.display_name"),
                                Input::new(&self.display_name),
                                cx,
                            ))),
                    );
                }
                let pw_label = if self.auth == AuthMode::NoPassword {
                    t("wizard.security.lock_password")
                } else {
                    t("wizard.security.password")
                };
                col = col.child(
                    h_flex()
                        .gap(px(8.))
                        .child(div().flex_1().child(field(
                            pw_label,
                            Input::new(&self.password).mask_toggle(),
                            cx,
                        )))
                        .child(div().flex_1().child(field(
                            t("wizard.security.confirm"),
                            Input::new(&self.confirm).mask_toggle(),
                            cx,
                        ))),
                );
                if self.auth == AuthMode::NoPassword {
                    col = col.child(hint(t("wizard.security.lock_password_hint"), cx));
                }
                if self.auth == AuthMode::MultiUser {
                    let allow = self.allow_registration;
                    col = col.child(
                        Checkbox::new("allow-registration")
                            .label(t("wizard.security.allow_registration"))
                            .checked(allow)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.allow_registration = *checked;
                                cx.notify();
                            })),
                    );
                }
                col.into_any_element()
            }
            Step::Protection => {
                let mut col = v_flex()
                    .gap(px(8.))
                    .child(hint(t("wizard.protection.intro"), cx));
                for (mode, key) in [
                    (LockMode::ReadOnly, "read_only"),
                    (LockMode::Private, "private"),
                    (LockMode::Encrypted, "encrypted"),
                ] {
                    let checked = self.lock_modes.contains(&mode);
                    col = col.child(
                        v_flex()
                            .child(
                                Checkbox::new(SharedString::from(format!("lock-{key}")))
                                    .label(t(&format!("lock.{key}")))
                                    .checked(checked)
                                    .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                        if *on {
                                            this.lock_modes.push(mode);
                                        } else {
                                            this.lock_modes.retain(|m| *m != mode);
                                            if this.default_lock == mode {
                                                this.default_lock = LockMode::None;
                                            }
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .pl(px(24.))
                                    .child(hint(t(&format!("lock.{key}_desc")), cx)),
                            ),
                    );
                }
                let mut options = vec![LockMode::None];
                options.extend(
                    self.lock_modes
                        .iter()
                        .copied()
                        .filter(|m| *m != LockMode::Encrypted),
                );
                let selected = options.iter().position(|m| *m == self.default_lock);
                let mut group = RadioGroup::horizontal("default-lock").selected_index(selected);
                for m in &options {
                    group = group.child(
                        Radio::new(SharedString::from(format!("default-{}", m.as_str())))
                            .label(t(&format!("lock.{}", m.as_str()))),
                    );
                }
                let opts = options.clone();
                col = col.child(field(
                    t("wizard.protection.default"),
                    group.on_click(cx.listener(move |this, ix: &usize, _, cx| {
                        this.default_lock = opts.get(*ix).copied().unwrap_or_default();
                        cx.notify();
                    })),
                    cx,
                ));
                if self.lock_modes.iter().any(|m| m.hides_content())
                    && self.auth == AuthMode::NoPassword
                    && self.password.read(cx).value().is_empty()
                {
                    col = col.child(hint(t("wizard.protection.needs_password"), cx));
                }
                col.into_any_element()
            }
            Step::Appearance => {
                let mut grid = h_flex().flex_wrap().gap(px(8.));
                for name in crate::themes::names(cx) {
                    let selected = name == self.theme;
                    let n = name.clone();
                    grid = grid.child(
                        Button::new(SharedString::from(format!("theme-{name}")))
                            .label(name.clone())
                            .when(selected, |b| b.primary())
                            .when(!selected, |b| b.outline())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.theme = n.clone();
                                crate::themes::apply(&n, cx);
                                cx.notify();
                            })),
                    );
                }
                v_flex()
                    .gap(px(10.))
                    .child(hint(t("wizard.appearance.intro"), cx))
                    .child(grid)
                    .into_any_element()
            }
            Step::Finish => {
                let auth = t(match self.auth {
                    AuthMode::NoPassword => "wizard.security.none",
                    AuthMode::SingleUser => "wizard.security.single",
                    AuthMode::MultiUser => "wizard.security.multi",
                });
                let storage = if self.portable {
                    t("wizard.storage.portable")
                } else {
                    t("wizard.storage.standard")
                };
                v_flex()
                    .gap(px(6.))
                    .child(hint(t("wizard.finish.intro"), cx))
                    .child(format!("• {}: {storage}", t("wizard.step.storage")))
                    .child(format!("• {}: {auth}", t("wizard.step.security")))
                    .child(format!("• {}: {}", t("wizard.step.appearance"), self.theme))
                    .child(hint(t("wizard.finish.later"), cx))
                    .into_any_element()
            }
        }
    }
}

fn step_title(step: Step) -> String {
    t(match step {
        Step::Welcome => "wizard.step.welcome",
        Step::Storage => "wizard.step.storage",
        Step::Security => "wizard.step.security",
        Step::Protection => "wizard.step.protection",
        Step::Appearance => "wizard.step.appearance",
        Step::Finish => "wizard.step.finish",
    })
}

/// The OS user name, used as the default account name.
fn whoami_name() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "me".into())
}

impl Render for Wizard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let step = self.current();
        let last = self.step + 1 == self.steps.len();
        let body = self.render_step(window, cx);
        let th = cx.theme();
        let mut dots = h_flex().gap(px(6.));
        for (i, _) in self.steps.iter().enumerate() {
            dots = dots.child(div().size(px(8.)).rounded_full().bg(if i <= self.step {
                th.primary
            } else {
                th.border
            }));
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
                    .w(px(560.))
                    .max_h_full()
                    .gap(px(18.))
                    .p(px(28.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(th.border)
                    .bg(th.popover)
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                    .child(step_title(step)),
                            )
                            .child(dots),
                    )
                    .child(body)
                    .child(error(self.error.as_ref(), cx))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                Button::new("wizard-back")
                                    .label(t("common.back"))
                                    .ghost()
                                    .disabled(self.step == 0 || self.busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                            )
                            .child(
                                Button::new("wizard-next")
                                    .label(if last {
                                        t("wizard.finish.start")
                                    } else {
                                        t("common.next")
                                    })
                                    .primary()
                                    .loading(self.busy)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.next(window, cx)),
                                    ),
                            ),
                    ),
            )
    }
}
