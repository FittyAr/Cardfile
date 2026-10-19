//! Settings dialog.

use cardfile_core::{AuthMode, LockMode, SearchScope, User, UserId};
use cardfile_i18n::{t, t_args};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    div, img, px,
};

use cardfile_db::CredentialUpdate;
use zeroize::Zeroizing;

use crate::keymap::*;
use crate::state::{AppState, TaskExt as _, run_db};
use crate::widgets::{error, field, hint, section};
use crate::workspace::Workspace;

pub fn open(workspace: Entity<Workspace>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SettingsView::new(workspace, window, cx));
    // Large enough for the sidebar layout, never wider than the window.
    let width = (window.viewport_size().width * 0.9).clamp(px(640.), px(1080.));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(t("settings.title"))
            .w(width)
            .child(view.clone())
    });
}

pub fn open_about(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| dialog.w(px(560.)).child(about(cx)))
}

fn about(cx: &App) -> AnyElement {
    let th = cx.theme();
    let version = env!("CARGO_PKG_VERSION");
    let channel = if version.contains('-') {
        t("about.channel_preview")
    } else {
        t("about.channel_stable")
    };
    let platform = format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH);
    let link = |id: &'static str, label: String, url: &'static str| {
        Button::new(id)
            .small()
            .ghost()
            .label(label)
            .on_click(move |_, _, cx| cx.open_url(url))
    };
    let info = |label: String, value: String| {
        h_flex()
            .justify_between()
            .py(px(4.))
            .border_b_1()
            .border_color(th.border.opacity(0.6))
            .text_size(px(12.))
            .child(div().text_color(th.muted_foreground).child(label))
            .child(div().child(value))
    };
    v_flex()
        .gap(px(14.))
        .child(
            h_flex()
                .gap(px(16.))
                .items_center()
                .child(img("cardfile/icon.png").size(px(72.)))
                .child(
                    v_flex()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(24.))
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .child(t("app.name")),
                        )
                        .child(
                            div()
                                .text_color(th.muted_foreground)
                                .child(t("about.description")),
                        ),
                ),
        )
        .child(
            v_flex()
                .child(info(t("about.version_label"), version.to_owned()))
                .child(info(t("about.channel"), channel))
                .child(info(t("about.platform"), platform))
                .child(info(
                    t("about.license_label"),
                    "MIT OR Apache-2.0".to_owned(),
                ))
                .child(info(t("about.copyright"), "© 2026 FittyAr".to_owned())),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(th.muted_foreground)
                .child(t("about.built_with")),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(th.muted_foreground)
                .child(t("about.translations")),
        )
        .child(
            h_flex()
                .flex_wrap()
                .gap(px(4.))
                .child(link(
                    "about-site",
                    t("about.website"),
                    "https://cardfile.fitty.ar/",
                ))
                .child(link(
                    "about-guide",
                    t("about.guide"),
                    "https://github.com/FittyAr/Cardfile/blob/main/docs/USER_GUIDE.md",
                ))
                .child(link(
                    "about-issue",
                    t("about.report"),
                    "https://github.com/FittyAr/Cardfile/issues/new",
                ))
                .child(link(
                    "about-changelog",
                    t("about.changelog"),
                    "https://github.com/FittyAr/Cardfile/blob/main/CHANGELOG.md",
                ))
                .child(
                    Button::new("about-licenses")
                        .small()
                        .ghost()
                        .label(t("about.third_party"))
                        .on_click(|_, _, cx| open_third_party(cx)),
                ),
        )
        .child(
            h_flex().justify_end().child(
                Button::new("about-update")
                    .small()
                    .primary()
                    .label(t("menu.check_updates"))
                    .on_click(|_, window, cx| crate::updater::check_now(window, cx)),
            ),
        )
        .into_any_element()
}

fn open_third_party(cx: &mut App) {
    let local = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("THIRD_PARTY_LICENSES.html")))
        .filter(|p| p.exists());
    match local {
        Some(path) => {
            let _ = open::that_detached(path);
        }
        None => cx.open_url("https://github.com/FittyAr/Cardfile/blob/main/docs/CREDITS.md"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab_ {
    General,
    Editor,
    Security,
    Users,
    AiServices,
    AiAssistant,
    Mcp,
    Data,
}

/// Which password a change applies to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Secret {
    Login,
    Lock,
}

pub struct SettingsView {
    workspace: Entity<Workspace>,
    tab: Tab_,
    current_pw: Entity<InputState>,
    new_pw: Entity<InputState>,
    confirm_pw: Entity<InputState>,
    pw_error: Option<SharedString>,
    pw_busy: bool,
    lock_current: Entity<InputState>,
    lock_new: Entity<InputState>,
    lock_confirm: Entity<InputState>,
    lock_error: Option<SharedString>,
    ai: Entity<crate::ai_settings::AiSettings>,
    users: Vec<User>,
    new_user: Entity<InputState>,
    new_user_pw: Entity<InputState>,
    user_error: Option<SharedString>,
    auth_error: Option<SharedString>,
}

const AUTO_LOCK: &[u32] = &[0, 60, 300, 900, 1800, 3600];
const RELOCK: &[u32] = &[0, 30, 60, 300, 900];
const HISTORY: &[u32] = &[0, 10, 25, 50, 100, 250];

impl SettingsView {
    fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let masked = |p: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).masked(true).placeholder(p))
        };
        let mut v = Self {
            workspace,
            tab: Tab_::General,
            current_pw: masked(t("settings.security.current_password"), window, cx),
            new_pw: masked(t("settings.security.new_password"), window, cx),
            confirm_pw: masked(t("wizard.security.confirm"), window, cx),
            pw_error: None,
            pw_busy: false,
            lock_current: masked(t("settings.security.current_lock_password"), window, cx),
            lock_new: masked(t("settings.security.new_lock_password"), window, cx),
            lock_confirm: masked(t("wizard.security.confirm"), window, cx),
            lock_error: None,
            ai: cx.new(|cx| crate::ai_settings::AiSettings::new(window, cx)),
            users: Vec::new(),
            new_user: cx.new(|cx| InputState::new(window, cx).placeholder(t("login.username"))),
            new_user_pw: masked(t("login.password"), window, cx),
            user_error: None,
            auth_error: None,
        };
        v.load_users(cx);
        v
    }

    fn select(&mut self, tab: Tab_, _window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        let page = match tab {
            Tab_::AiAssistant => Some(crate::ai_settings::AiPage::Assistant),
            Tab_::Mcp => Some(crate::ai_settings::AiPage::Mcp),
            Tab_::AiServices => Some(crate::ai_settings::AiPage::Services),
            _ => None,
        };
        if let Some(page) = page {
            self.ai.update(cx, |a, cx| a.set_page(page, cx));
        }
        cx.notify();
    }

    fn is_admin(cx: &App) -> bool {
        AppState::session(cx).is_some_and(|s| s.user.is_admin)
    }

    fn load_users(&mut self, cx: &mut Context<Self>) {
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| db.list_users());
        cx.spawn(async move |this, cx| {
            if let Ok(users) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.users = users;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Persist a user setting and refresh the workspace.
    fn set(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut cardfile_core::UserSettings),
    ) {
        AppState::update_user_settings(cx, f);
        self.workspace
            .update(cx, |ws, cx| ws.apply_settings(window, cx));
        cx.notify();
    }

    fn set_app(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut cardfile_core::AppSettings)) {
        let state = AppState::get_mut(cx);
        f(&mut state.app);
        let app = state.app.clone();
        let db = AppState::db(cx);
        run_db(cx, move |_| db.save_app_settings(&app)).detach_and_log_err(cx);
        cx.notify();
    }

    /// Click handler running `action` from the workspace. Actions dispatched from inside
    /// this dialog bubble up to the window root, not through the workspace and the app
    /// view where their handlers are, so they would do nothing.
    fn dispatcher<A: gpui_kit::Action>(
        &self,
        action: A,
        cx: &App,
    ) -> impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static {
        let focus = gpui_kit::Focusable::focus_handle(self.workspace.read(cx), cx);
        move |_, window, cx| focus.dispatch_action(&action, window, cx)
    }

    fn row(label: String, desc: Option<String>, control: impl IntoElement, cx: &App) -> AnyElement {
        h_flex()
            .justify_between()
            .gap(px(16.))
            .py(px(6.))
            .child(
                v_flex()
                    .flex_1()
                    .child(div().text_size(px(14.)).child(label))
                    .children(desc.map(|d| hint(d, cx))),
            )
            .child(control)
            .into_any_element()
    }

    fn choice<T: Copy + PartialEq + 'static>(
        id: &'static str,
        current: T,
        options: Vec<(T, String)>,
        on_pick: impl Fn(T, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let label = options
            .iter()
            .find(|(v, _)| *v == current)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        let on_pick = std::rc::Rc::new(on_pick);
        Button::new(id)
            .small()
            .outline()
            .label(label)
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for (value, label) in &options {
                    let f = on_pick.clone();
                    let value = *value;
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(value == current)
                            .on_click(move |_, window, cx| f(value, window, cx)),
                    );
                }
                menu
            })
    }

    fn duration_label(secs: u32) -> String {
        match secs {
            0 => t("settings.never"),
            s if s < 60 => t_args("settings.seconds", &[("n", &s.to_string())]),
            s => t_args("settings.minutes", &[("n", &(s / 60).to_string())]),
        }
    }

    fn general(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = AppState::user_settings(cx);
        let view = cx.entity();
        let themes: Vec<(usize, String)> = crate::themes::names(cx)
            .into_iter()
            .enumerate()
            .map(|(i, n)| (i, n.to_string()))
            .collect();
        let current_theme = themes
            .iter()
            .find(|(_, n)| *n == s.theme)
            .map_or(0, |(i, _)| *i);
        let theme_names = themes.clone();
        let languages: Vec<(usize, String)> = cardfile_i18n::available_locales()
            .into_iter()
            .enumerate()
            .collect();
        let current_lang = languages
            .iter()
            .find(|(_, l)| *l == s.language)
            .map_or(0, |(i, _)| *i);
        let langs = languages.clone();
        let v1 = view.clone();
        let v2 = view.clone();
        v_flex()
            .child(Self::row(
                t("settings.general.theme"),
                None,
                Self::choice("theme", current_theme, themes, move |i, window, cx| {
                    let name = theme_names[i].1.clone();
                    crate::themes::apply(&name, cx);
                    v1.update(cx, |v, cx| v.set(window, cx, |s| s.theme = name));
                }),
                cx,
            ))
            .child(Self::row(
                t("settings.general.language"),
                Some(t("about.translations")),
                Self::choice(
                    "language",
                    current_lang,
                    languages
                        .iter()
                        .map(|(i, l)| (*i, language_name(l)))
                        .collect(),
                    move |i, window, cx| {
                        let lang = langs[i].1.clone();
                        cardfile_i18n::set_locale(&lang);
                        v2.update(cx, |v, cx| v.set(window, cx, |s| s.language = lang));
                        window.refresh();
                    },
                ),
                cx,
            ))
            .child(Self::row(
                t("settings.general.font_size"),
                None,
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .child(
                        Button::new("font-dec")
                            .small()
                            .outline()
                            .label("A−")
                            .on_click(cx.listener(|v, _, window, cx| {
                                v.set(window, cx, |s| {
                                    s.editor_font_size = (s.editor_font_size - 1.).max(10.)
                                })
                            })),
                    )
                    .child(
                        div()
                            .w(px(32.))
                            .text_center()
                            .child(format!("{}", s.editor_font_size)),
                    )
                    .child(
                        Button::new("font-inc")
                            .small()
                            .outline()
                            .label("A+")
                            .on_click(cx.listener(|v, _, window, cx| {
                                v.set(window, cx, |s| {
                                    s.editor_font_size = (s.editor_font_size + 1.).min(28.)
                                })
                            })),
                    ),
                cx,
            ))
            .child(Self::row(
                t("settings.general.confirm_delete"),
                None,
                Switch::new("confirm-delete")
                    .checked(s.confirm_delete)
                    .on_click(cx.listener(|v, on: &bool, window, cx| {
                        let on = *on;
                        v.set(window, cx, |s| s.confirm_delete = on);
                    })),
                cx,
            ))
            .when(crate::updater::enabled(), |d| {
                d.child(Self::row(
                    t("settings.general.check_updates"),
                    Some(t("settings.general.check_updates_hint")),
                    Switch::new("check-updates")
                        .checked(s.check_for_updates)
                        .on_click(cx.listener(|v, on: &bool, window, cx| {
                            let on = *on;
                            v.set(window, cx, |s| s.check_for_updates = on);
                        })),
                    cx,
                ))
            })
            .child(Self::row(
                t("settings.general.keymap"),
                Some(t("settings.general.keymap_hint")),
                Button::new("keymap")
                    .small()
                    .outline()
                    .label(t("settings.open"))
                    .on_click(self.dispatcher(OpenKeymap, cx)),
                cx,
            ))
            .into_any_element()
    }

    fn editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = AppState::user_settings(cx);
        let view = cx.entity();
        let enabled = AppState::get(cx).app.enabled_lock_modes.clone();
        let mut lock_options = vec![(LockMode::None, t("lock.none"))];
        for m in [LockMode::ReadOnly, LockMode::Private] {
            if enabled.contains(&m) {
                lock_options.push((m, t(&format!("lock.{}", m.as_str()))));
            }
        }
        let dict_langs: Vec<(usize, String)> = crate::spell::available_languages(cx)
            .into_iter()
            .enumerate()
            .collect();
        let current_dict = dict_langs
            .iter()
            .find(|(_, l)| *l == s.spellcheck_language)
            .map_or(usize::MAX, |(i, _)| *i);
        let dl = dict_langs.clone();
        let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
        v_flex()
            .child(Self::row(
                t("settings.editor.markdown_shortcuts"),
                Some(t("settings.editor.markdown_shortcuts_hint")),
                Switch::new("md-shortcuts")
                    .checked(s.markdown_shortcuts)
                    .on_click(cx.listener(|v, on: &bool, window, cx| {
                        let on = *on;
                        v.set(window, cx, |s| s.markdown_shortcuts = on);
                    })),
                cx,
            ))
            .child(Self::row(
                t("settings.editor.spellcheck"),
                Some(if dict_langs.is_empty() {
                    t("settings.editor.no_dictionaries")
                } else {
                    t("settings.editor.spellcheck_hint")
                }),
                Switch::new("spellcheck")
                    .checked(s.spellcheck)
                    .on_click(cx.listener(|v, on: &bool, window, cx| {
                        let on = *on;
                        v.set(window, cx, |s| s.spellcheck = on);
                    })),
                cx,
            ))
            .when(!dict_langs.is_empty(), |d| {
                d.child(Self::row(
                    t("settings.editor.spell_language"),
                    None,
                    Self::choice(
                        "dict",
                        current_dict,
                        dict_langs.clone(),
                        move |i, window, cx| {
                            let lang = dl[i].1.clone();
                            v1.update(cx, |v, cx| {
                                v.set(window, cx, |s| s.spellcheck_language = lang)
                            });
                        },
                    ),
                    cx,
                ))
            })
            .child(Self::row(
                t("settings.editor.search_scope"),
                None,
                Self::choice(
                    "scope",
                    s.search_scope,
                    vec![
                        (SearchScope::TitleOnly, t("settings.editor.scope_title")),
                        (
                            SearchScope::TitleAndContent,
                            t("settings.editor.scope_content"),
                        ),
                        (SearchScope::Everything, t("settings.editor.scope_all")),
                    ],
                    move |sc, window, cx| {
                        v2.update(cx, |v, cx| v.set(window, cx, |s| s.search_scope = sc))
                    },
                ),
                cx,
            ))
            .child(Self::row(
                t("settings.editor.default_lock"),
                Some(t("settings.editor.default_lock_hint")),
                Self::choice(
                    "default-lock",
                    s.default_lock_mode,
                    lock_options,
                    move |m, window, cx| {
                        v3.update(cx, |v, cx| v.set(window, cx, |s| s.default_lock_mode = m))
                    },
                ),
                cx,
            ))
            .child(Self::row(
                t("settings.editor.history"),
                Some(t("settings.editor.history_hint")),
                Self::choice(
                    "history",
                    s.max_revisions,
                    HISTORY
                        .iter()
                        .map(|n| {
                            (
                                *n,
                                if *n == 0 {
                                    t("settings.off")
                                } else {
                                    n.to_string()
                                },
                            )
                        })
                        .collect(),
                    move |n, window, cx| {
                        v4.update(cx, |v, cx| v.set(window, cx, |s| s.max_revisions = n))
                    },
                ),
                cx,
            ))
            .into_any_element()
    }

    fn security(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = AppState::user_settings(cx);
        let app = AppState::get(cx).app.clone();
        let admin = Self::is_admin(cx);
        let view = cx.entity();
        let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
        let has_lock = AppState::lock_password_hash(cx).is_some();
        let separate_lock = AppState::session(cx).is_some_and(|s| s.user.lock_hash.is_some());
        let mut col = v_flex();
        if app.auth_mode != AuthMode::NoPassword {
            col = col.child(Self::row(
                t("settings.security.auto_lock"),
                Some(t("settings.security.auto_lock_hint")),
                Self::choice(
                    "auto-lock",
                    s.auto_lock_secs,
                    AUTO_LOCK
                        .iter()
                        .map(|n| (*n, Self::duration_label(*n)))
                        .collect(),
                    move |n, window, cx| {
                        v1.update(cx, |v, cx| v.set(window, cx, |s| s.auto_lock_secs = n))
                    },
                ),
                cx,
            ));
        }
        col = col
            .child(Self::row(
                t("settings.security.relock"),
                Some(t("settings.security.relock_hint")),
                Self::choice(
                    "relock",
                    s.relock_card_secs,
                    RELOCK
                        .iter()
                        .map(|n| (*n, Self::duration_label(*n)))
                        .collect(),
                    move |n, window, cx| {
                        v2.update(cx, |v, cx| v.set(window, cx, |s| s.relock_card_secs = n))
                    },
                ),
                cx,
            ))
            .child(Self::row(
                t("settings.security.mask"),
                Some(t("settings.security.mask_hint")),
                Self::choice(
                    "mask",
                    s.mask_visible_chars,
                    (0..=6).map(|n| (n, n.to_string())).collect(),
                    move |n, window, cx| {
                        v3.update(cx, |v, cx| v.set(window, cx, |s| s.mask_visible_chars = n))
                    },
                ),
                cx,
            ));
        if admin {
            let modes = [
                (AuthMode::NoPassword, "wizard.security.none"),
                (AuthMode::SingleUser, "wizard.security.single"),
                (AuthMode::MultiUser, "wizard.security.multi"),
            ];
            let mut row = h_flex().gap(px(6.));
            for (mode, key) in modes {
                let current = app.auth_mode == mode;
                row = row.child(
                    Button::new(SharedString::from(format!("auth-{key}")))
                        .small()
                        .label(t(key))
                        .when(current, |b| b.primary())
                        .when(!current, |b| b.outline())
                        .on_click(
                            cx.listener(move |v, _, window, cx| v.set_auth_mode(mode, window, cx)),
                        ),
                );
            }
            col = col
                .child(section(t("settings.security.auth_mode"), cx))
                .child(hint(t("settings.security.auth_mode_hint"), cx))
                .child(div().pt(px(4.)).child(row))
                .child(error(self.auth_error.as_ref(), cx));
            col = col.child(section(t("settings.security.lock_modes"), cx));
            for m in [LockMode::ReadOnly, LockMode::Private, LockMode::Encrypted] {
                let on = app.enabled_lock_modes.contains(&m);
                col = col.child(
                    Checkbox::new(SharedString::from(format!("enabled-{}", m.as_str())))
                        .label(t(&format!("lock.{}", m.as_str())))
                        .checked(on)
                        .on_click(cx.listener(move |v, checked: &bool, _, cx| {
                            let checked = *checked;
                            v.set_app(cx, |a| {
                                a.enabled_lock_modes.retain(|x| *x != m);
                                if checked {
                                    a.enabled_lock_modes.push(m);
                                }
                            });
                        })),
                );
            }
            if app.auth_mode == AuthMode::MultiUser {
                col = col.child(
                    Checkbox::new("allow-registration")
                        .label(t("wizard.security.allow_registration"))
                        .checked(app.allow_registration)
                        .on_click(cx.listener(|v, on: &bool, _, cx| {
                            let on = *on;
                            v.set_app(cx, |a| a.allow_registration = on);
                        })),
                );
            }
        }
        if app.auth_mode != AuthMode::NoPassword {
            col = col
                .child(section(t("settings.security.change_password"), cx))
                .child(hint(t("settings.security.password_hint"), cx))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .pt(px(6.))
                        .child(div().flex_1().child(Input::new(&self.current_pw)))
                        .child(div().flex_1().child(Input::new(&self.new_pw)))
                        .child(div().flex_1().child(Input::new(&self.confirm_pw))),
                )
                .child(error(self.pw_error.as_ref(), cx))
                .child(
                    h_flex().justify_end().child(
                        Button::new("change-pw")
                            .small()
                            .primary()
                            .loading(self.pw_busy)
                            .label(t("settings.security.save_password"))
                            .on_click(cx.listener(|v, _, window, cx| {
                                v.change_secret(Secret::Login, window, cx)
                            })),
                    ),
                );
        }
        let lock_hint = if app.auth_mode == AuthMode::NoPassword {
            t("settings.security.lock_hint_nopw")
        } else if separate_lock {
            t("settings.security.lock_hint_separate")
        } else {
            t("settings.security.lock_hint_same")
        };
        col = col
            .child(section(t("settings.security.lock_password"), cx))
            .child(hint(lock_hint, cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .pt(px(6.))
                    .when(has_lock, |d| {
                        d.child(div().flex_1().child(Input::new(&self.lock_current)))
                    })
                    .child(div().flex_1().child(Input::new(&self.lock_new)))
                    .child(div().flex_1().child(Input::new(&self.lock_confirm))),
            )
            .child(error(self.lock_error.as_ref(), cx))
            .child(
                h_flex().justify_end().child(
                    Button::new("change-lock-pw")
                        .small()
                        .primary()
                        .loading(self.pw_busy)
                        .label(t("settings.security.save_lock_password"))
                        .on_click(cx.listener(|v, _, window, cx| {
                            v.change_secret(Secret::Lock, window, cx)
                        })),
                ),
            );
        col.into_any_element()
    }

    /// Change the sign-in password (`Secret::Login`) or the lock password that
    /// protects private and encrypted cards (`Secret::Lock`).
    ///
    /// Only the user's secret key is re-sealed: card keys stay as they are, so cards
    /// shared with others keep working. Legacy encrypted cards are upgraded on the way.
    fn change_secret(&mut self, which: Secret, window: &mut Window, cx: &mut Context<Self>) {
        let [cur, new, confirm] = match which {
            Secret::Login => [&self.current_pw, &self.new_pw, &self.confirm_pw],
            Secret::Lock => [&self.lock_current, &self.lock_new, &self.lock_confirm],
        };
        let current = Zeroizing::new(cur.read(cx).value().to_string());
        let new = Zeroizing::new(new.read(cx).value().to_string());
        let confirm = Zeroizing::new(confirm.read(cx).value().to_string());
        let set_error = |this: &mut Self, msg: String| match which {
            Secret::Login => this.pw_error = Some(msg.into()),
            Secret::Lock => this.lock_error = Some(msg.into()),
        };
        if new.chars().count() < 4 {
            set_error(self, t("wizard.error.password_short"));
            cx.notify();
            return;
        }
        if *new != *confirm {
            set_error(self, t("wizard.error.password_mismatch"));
            cx.notify();
            return;
        }
        let Some(session) = AppState::session(cx).cloned() else {
            return;
        };
        let auth = AppState::get(cx).app.auth_mode;
        let db = AppState::db(cx);
        self.pw_busy = true;
        self.pw_error = None;
        self.lock_error = None;
        cx.notify();
        let task = cx.background_spawn(async move {
            let user = db.user(session.user.id)?;
            let new_hash = cardfile_crypto::hash_password(&new)?;
            let mut update = match which {
                Secret::Login => {
                    // Argon2 is slow by design: verify here, off the UI thread.
                    if let Some(h) = &user.password_hash
                        && !cardfile_crypto::verify_password(h, &current)
                    {
                        return Err(anyhow::anyhow!(t("card.unlock.wrong")));
                    }
                    // Locks follow the sign-in password unless a separate one is set.
                    let mut update = if user.lock_hash.is_none() && user.password_hash.is_some() {
                        crate::locks::relock_user(&db, &user, &current, &new)?
                    } else {
                        CredentialUpdate::default()
                    };
                    update.password_hash = Some(Some(new_hash));
                    update
                }
                Secret::Lock => {
                    let mut update = if user.effective_lock_hash().is_some() {
                        crate::locks::relock_user(&db, &user, &current, &new)?
                    } else {
                        let (public, sealed, _) =
                            cardfile_crypto::generate_user_keys(&new, &user.id.to_string())?;
                        CredentialUpdate {
                            keys: Some((public, sealed)),
                            ..Default::default()
                        }
                    };
                    // Typing the sign-in password as the lock password goes back to
                    // one password for both.
                    let same_as_login = auth != AuthMode::NoPassword
                        && user
                            .password_hash
                            .as_deref()
                            .is_some_and(|h| cardfile_crypto::verify_password(h, &new));
                    update.lock_hash = Some((!same_as_login).then_some(new_hash));
                    update
                }
            };
            update.rekey.card_keys.sort_by_key(|(c, _, _)| *c);
            db.update_credentials(user.id, &update)?;
            anyhow::Ok(db.user(user.id)?)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.pw_busy = false;
                match result {
                    Ok(user) => {
                        AppState::refresh_user(cx, user);
                        let inputs = match which {
                            Secret::Login => [&this.current_pw, &this.new_pw, &this.confirm_pw],
                            Secret::Lock => {
                                [&this.lock_current, &this.lock_new, &this.lock_confirm]
                            }
                        };
                        for input in inputs {
                            input.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                        window.push_notification(
                            Notification::success(t("settings.security.password_saved")),
                            cx,
                        );
                    }
                    Err(e) => match which {
                        Secret::Login => this.pw_error = Some(e.to_string().into()),
                        Secret::Lock => this.lock_error = Some(e.to_string().into()),
                    },
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Switch the installation's sign-in mode (admins only).
    fn set_auth_mode(&mut self, mode: AuthMode, window: &mut Window, cx: &mut Context<Self>) {
        let current = AppState::get(cx).app.auth_mode;
        if mode == current {
            return;
        }
        self.auth_error = None;
        let live_users = self.users.len();
        if mode != AuthMode::MultiUser && live_users > 1 {
            self.auth_error = Some(t("settings.security.auth_one_user").into());
            cx.notify();
            return;
        }
        let Some(session) = AppState::session(cx).cloned() else {
            return;
        };
        // Moving between "no password" and password modes moves the lock password
        // into (or out of) the sign-in password; the secret key stays sealed under it.
        let hash = AppState::lock_password_hash(cx);
        if mode != AuthMode::NoPassword && hash.is_none() {
            self.auth_error = Some(t("settings.security.auth_needs_password").into());
            cx.notify();
            return;
        }
        let db = AppState::db(cx);
        let user_id = session.user.id;
        let to_none = mode == AuthMode::NoPassword;
        let from_none = current == AuthMode::NoPassword;
        let multi_only = mode != AuthMode::MultiUser;
        let task = cx.background_spawn(async move {
            // `self.users` loads asynchronously and may still be empty: count accounts
            // from the database before allowing a single-user mode.
            if multi_only && db.list_users()?.len() > 1 {
                return Err(anyhow::anyhow!(t("settings.security.auth_one_user")));
            }
            if to_none {
                db.set_lock_hash(user_id, hash.as_deref())?;
                db.set_password_hash(user_id, None)?;
            } else if from_none {
                db.set_password_hash(user_id, hash.as_deref())?;
                db.set_lock_hash(user_id, None)?;
            }
            anyhow::Ok(db.user(user_id)?)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(user) => {
                        AppState::refresh_user(cx, user);
                        this.set_app(cx, |a| a.auth_mode = mode);
                        window.push_notification(
                            Notification::success(t("settings.security.auth_changed")),
                            cx,
                        );
                    }
                    Err(e) => this.auth_error = Some(e.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn users(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let me = AppState::session(cx).map(|s| s.user.id);
        let th = cx.theme();
        let mut list = v_flex().gap(px(4.));
        for user in &self.users {
            let id = user.id;
            let is_me = Some(id) == me;
            list = list.child(
                h_flex()
                    .justify_between()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(th.muted)
                    .child(
                        v_flex()
                            .child(div().child(format!(
                                "{}{}",
                                user.display_name,
                                if user.is_admin {
                                    format!(" · {}", t("settings.users.admin"))
                                } else {
                                    String::new()
                                }
                            )))
                            .child(hint(user.username.clone(), cx)),
                    )
                    .when(!is_me, |d| {
                        d.child(
                            Button::new(SharedString::from(format!("del-{id}")))
                                .small()
                                .ghost()
                                .danger()
                                .label(t("settings.users.delete"))
                                .on_click(cx.listener(move |v, _, window, cx| {
                                    v.delete_user(id, window, cx)
                                })),
                        )
                    }),
            );
        }
        v_flex()
            .gap(px(8.))
            .child(list)
            .child(section(t("settings.users.add"), cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(div().flex_1().child(Input::new(&self.new_user)))
                    .child(div().flex_1().child(Input::new(&self.new_user_pw)))
                    .child(
                        Button::new("add-user")
                            .small()
                            .primary()
                            .label(t("settings.users.create"))
                            .on_click(cx.listener(|v, _, window, cx| v.add_user(window, cx))),
                    ),
            )
            .child(error(self.user_error.as_ref(), cx))
            .into_any_element()
    }

    fn add_user(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.new_user.read(cx).value().trim().to_string();
        let pw = self.new_user_pw.read(cx).value().to_string();
        if name.is_empty() || pw.chars().count() < 4 {
            self.user_error = Some(t("settings.users.invalid").into());
            cx.notify();
            return;
        }
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            if db.user_by_username(&name)?.is_some() {
                anyhow::bail!(t("login.error.taken"));
            }
            let hash = cardfile_crypto::hash_password(&pw)?;
            db.create_user(&name, &name, Some(&hash), false)?;
            anyhow::Ok(())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(()) => {
                        this.user_error = None;
                        this.new_user
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        this.new_user_pw
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        this.load_users(cx);
                    }
                    Err(e) => this.user_error = Some(e.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn delete_user(&mut self, id: UserId, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::dialogs::confirm(
            t("settings.users.delete"),
            t("settings.users.delete_body"),
            t("settings.users.delete"),
            window,
            cx,
            move |_, cx| {
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| db.delete_user(id));
                let view = view.clone();
                cx.spawn(async move |cx| {
                    let _ = task.await;
                    view.update(cx, |v, cx| v.load_users(cx));
                })
                .detach();
            },
        );
    }

    fn data(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let paths = AppState::get(cx).paths.clone();
        v_flex()
            .gap(px(4.))
            .child(field(
                t("settings.data.location"),
                div()
                    .text_size(px(13.))
                    .child(paths.root.display().to_string()),
                cx,
            ))
            .child(hint(
                if paths.portable {
                    t("settings.data.portable")
                } else {
                    t("settings.data.standard")
                },
                cx,
            ))
            .child(hint(t("settings.data.files"), cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .pt(px(8.))
                    .flex_wrap()
                    .child(
                        Button::new("open-folder")
                            .small()
                            .outline()
                            .label(t("menu.open_data_folder"))
                            .on_click(self.dispatcher(OpenDataFolder, cx)),
                    )
                    .child(
                        Button::new("backup")
                            .small()
                            .outline()
                            .label(t("menu.backup"))
                            .on_click(self.dispatcher(BackupNow, cx)),
                    ),
            )
            .child(section(t("settings.data.export"), cx))
            .child(hint(t("settings.data.export_hint"), cx))
            .child(
                h_flex()
                    .gap(px(8.))
                    .pt(px(4.))
                    .child(
                        Button::new("export-md")
                            .small()
                            .outline()
                            .label(t("export.markdown"))
                            .on_click(|_, window, cx| {
                                export_all(crate::export::Format::Markdown, window, cx)
                            }),
                    )
                    .child(
                        Button::new("export-html")
                            .small()
                            .outline()
                            .label(t("export.html"))
                            .on_click(|_, window, cx| {
                                export_all(crate::export::Format::Html, window, cx)
                            }),
                    )
                    .child(
                        Button::new("export-pdf")
                            .small()
                            .outline()
                            .label(t("export.pdf"))
                            .on_click(|_, window, cx| {
                                export_all(crate::export::Format::Pdf, window, cx)
                            }),
                    )
                    .child(
                        Button::new("export-json")
                            .small()
                            .outline()
                            .label(t("export.json"))
                            .on_click(|_, window, cx| {
                                export_all(crate::export::Format::Json, window, cx)
                            }),
                    ),
            )
            .child(section(t("nav.recycle_bin"), cx))
            .child(
                h_flex().pt(px(4.)).child(
                    Button::new("empty-bin")
                        .small()
                        .danger()
                        .label(t("recycle.empty"))
                        .on_click(self.dispatcher(EmptyRecycleBin, cx)),
                ),
            )
            .into_any_element()
    }
}

fn export_all(format: crate::export::Format, window: &mut Window, cx: &mut App) {
    if let Some(owner) = AppState::session(cx).map(|s| s.user.id) {
        crate::export::export_all(owner, format, window, cx);
    }
}

fn language_name(code: &str) -> String {
    match code {
        "en" => "English".into(),
        "es" => "Español".into(),
        "pt-BR" | "pt_BR" => "Português (Brasil)".into(),
        "fr" => "Français".into(),
        "de" => "Deutsch".into(),
        "ru" => "Русский".into(),
        "zh" | "zh-CN" => "中文".into(),
        "hi" => "हिन्दी".into(),
        "ar" => "العربية".into(),
        "bn" => "বাংলা".into(),
        "ja" => "日本語".into(),
        "ko" => "한국어".into(),
        "it" => "Italiano".into(),
        "tr" => "Türkçe".into(),
        "vi" => "Tiếng Việt".into(),
        "id" => "Bahasa Indonesia".into(),
        "pl" => "Polski".into(),
        "uk" => "Українська".into(),
        "nl" => "Nederlands".into(),
        "th" => "ไทย".into(),
        "fa" => "فارسی".into(),
        other => other.to_owned(),
    }
}

/// A titled group of sidebar entries: (section, icon, label).
type NavGroup = (String, Vec<(Tab_, IconName, String)>);

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let multi_admin =
            AppState::get(cx).app.auth_mode == AuthMode::MultiUser && Self::is_admin(cx);
        // Sections grouped in a sidebar, so new settings get a place without crowding.
        let mut groups: Vec<NavGroup> = vec![
            (
                t("settings.group.app"),
                vec![
                    (Tab_::General, IconName::Settings, t("settings.tab.general")),
                    (
                        Tab_::Editor,
                        IconName::NotebookPen,
                        t("settings.tab.editor"),
                    ),
                ],
            ),
            (
                t("settings.group.privacy"),
                vec![(Tab_::Security, IconName::Lock, t("settings.tab.security"))],
            ),
            (
                t("settings.group.ai"),
                vec![
                    (
                        Tab_::AiServices,
                        IconName::Sparkles,
                        t("settings.tab.ai_services"),
                    ),
                    (
                        Tab_::AiAssistant,
                        IconName::Bot,
                        t("settings.tab.ai_assistant"),
                    ),
                    (Tab_::Mcp, IconName::KeyRound, t("settings.tab.mcp")),
                ],
            ),
            (
                t("settings.group.data"),
                vec![(Tab_::Data, IconName::Inbox, t("settings.tab.data"))],
            ),
        ];
        if multi_admin {
            groups[1]
                .1
                .push((Tab_::Users, IconName::Users, t("settings.tab.users")));
        }
        let th = cx.theme();
        let mut nav = v_flex()
            .w(px(210.))
            .flex_shrink_0()
            .h_full()
            .py(px(6.))
            .pr(px(10.))
            .gap(px(2.))
            .border_r_1()
            .border_color(th.border);
        let mut title = String::new();
        for (group, items) in groups {
            nav = nav.child(
                div()
                    .px(px(10.))
                    .pt(px(10.))
                    .pb(px(2.))
                    .text_size(px(11.))
                    .text_color(th.muted_foreground)
                    .child(group.to_uppercase()),
            );
            for (tab, icon, label) in items {
                let active = self.tab == tab;
                if active {
                    title = label.clone();
                }
                nav = nav.child(
                    h_flex()
                        .id(SharedString::from(format!("nav-{label}")))
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .text_size(px(13.))
                        .when(active, |d| {
                            d.bg(th.accent).font_weight(gpui_kit::FontWeight::MEDIUM)
                        })
                        .hover(|d| d.bg(th.accent.opacity(0.6)))
                        .child(
                            gpui_kit::component::Icon::new(icon)
                                .size_4()
                                .text_color(th.muted_foreground),
                        )
                        .child(label)
                        .on_click(cx.listener(move |v, _, window, cx| v.select(tab, window, cx))),
                );
            }
        }
        let body = match self.tab {
            Tab_::General => self.general(cx),
            Tab_::Editor => self.editor(cx),
            Tab_::Security => self.security(cx),
            Tab_::Users => self.users(cx),
            Tab_::AiServices | Tab_::AiAssistant | Tab_::Mcp => self.ai.clone().into_any_element(),
            Tab_::Data => self.data(cx),
        };
        let height = (window.viewport_size().height * 0.8).clamp(px(420.), px(760.));
        h_flex()
            .h(height)
            .items_start()
            .gap(px(18.))
            .child(nav)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .pr(px(8.))
                            .child(body),
                    ),
            )
    }
}

#[allow(dead_code)]
fn _sizes(b: Button) -> Button {
    b.xsmall()
}
