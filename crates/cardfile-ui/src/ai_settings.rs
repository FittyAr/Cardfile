//! Settings → AI: AI services for the assistant, the local MCP server and the API keys
//! AI clients use, and what the AI did recently.

use cardfile_ai::{ApiKind, CUSTOM_PRESET_ID, PRESETS, ProviderConfig};
use cardfile_core::{ActivityEntry, ApiKey, AuthMode};
use cardfile_i18n::{t, t_args};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div, px,
};

use crate::mcp_host::{McpServer, Status};
use crate::state::{AppState, TaskExt as _, run_db};
use crate::widgets::{error, field, hint, relative_time, section};

/// The user's configured AI services.
pub fn providers(cx: &App) -> Vec<ProviderConfig> {
    AppState::user_settings(cx)
        .ai_providers
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

/// The service the assistant uses, if one is configured and usable. The assistant
/// UI is shown only when this is `Some`.
pub fn active_provider(cx: &App) -> Option<ProviderConfig> {
    let s = AppState::user_settings(cx);
    let all = providers(cx);
    let usable = |p: &ProviderConfig| p.is_usable();
    s.ai_active_provider
        .as_ref()
        .and_then(|id| all.iter().find(|p| &p.id == id && usable(p)))
        .or_else(|| all.iter().find(|p| usable(p)))
        .cloned()
}

fn save_providers(list: &[ProviderConfig], active: Option<String>, cx: &mut App) {
    let values: Vec<serde_json::Value> = list
        .iter()
        .filter_map(|p| serde_json::to_value(p).ok())
        .collect();
    AppState::update_user_settings(cx, |s| {
        s.ai_providers = values;
        s.ai_active_provider = active;
    });
    cx.refresh_windows();
}

pub struct AiSettings {
    editing: Option<ProviderConfig>,
    f_name: Entity<InputState>,
    f_url: Entity<InputState>,
    f_model: Entity<InputState>,
    f_key: Entity<InputState>,
    f_version: Entity<InputState>,
    models: Vec<String>,
    test: Option<(bool, SharedString)>,
    testing: bool,
    port: Entity<InputState>,
    keys: Vec<ApiKey>,
    key_name: Entity<InputState>,
    key_private: bool,
    key_read_only: bool,
    new_secret: Option<(SharedString, Vec<(String, String)>)>,
    activity: Vec<(ActivityEntry, String)>,
    error: Option<SharedString>,
}

impl AiSettings {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |p: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(p))
        };
        let port = AppState::get(cx).app.mcp_port.to_string();
        let mut this = Self {
            editing: None,
            f_name: input(t("ai.provider.name"), window, cx),
            f_url: input(t("ai.provider.url"), window, cx),
            f_model: input(t("ai.provider.model"), window, cx),
            f_key: cx.new(|cx| {
                InputState::new(window, cx)
                    .masked(true)
                    .placeholder(t("ai.provider.key"))
            }),
            f_version: input(t("ai.provider.api_version"), window, cx),
            models: Vec::new(),
            test: None,
            testing: false,
            port: cx.new(|cx| InputState::new(window, cx).default_value(port)),
            keys: Vec::new(),
            key_name: input(t("ai.keys.name_placeholder"), window, cx),
            key_private: false,
            key_read_only: false,
            new_secret: None,
            activity: Vec::new(),
            error: None,
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(me) = AppState::session(cx).map(|s| s.user.id) else {
            return;
        };
        let db = AppState::db(cx);
        let task = run_db(cx, move |_| {
            let keys = db.list_api_keys(me)?;
            let ai = db.ai_user()?;
            let activity = db
                .activity(None, Some(me), 40)?
                .into_iter()
                .filter(|e| e.actor_id == ai.id)
                .take(15)
                .map(|e| (e, ai.display_name.clone()))
                .collect::<Vec<_>>();
            Ok((keys, activity))
        });
        cx.spawn(async move |this, cx| {
            if let Ok((keys, activity)) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.keys = keys;
                    this.activity = activity;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    // -----------------------------------------------------------------
    // Providers
    // -----------------------------------------------------------------

    fn edit(&mut self, cfg: ProviderConfig, window: &mut Window, cx: &mut Context<Self>) {
        let set = |input: &Entity<InputState>, v: String, window: &mut Window, cx: &mut App| {
            input.update(cx, |s, cx| s.set_value(v, window, cx));
        };
        set(&self.f_name, cfg.name.clone(), window, cx);
        set(&self.f_url, cfg.base_url.clone(), window, cx);
        set(&self.f_model, cfg.model.clone(), window, cx);
        set(
            &self.f_key,
            cfg.api_key.clone().unwrap_or_default(),
            window,
            cx,
        );
        set(
            &self.f_version,
            cfg.api_version.clone().unwrap_or_default(),
            window,
            cx,
        );
        self.models = cfg
            .preset_info()
            .map(|p| p.models.iter().map(|m| (*m).to_owned()).collect())
            .unwrap_or_default();
        self.test = None;
        self.editing = Some(cfg);
        cx.notify();
    }

    /// The provider being edited with the form's values applied.
    fn form(&self, cx: &App) -> Option<ProviderConfig> {
        let mut cfg = self.editing.clone()?;
        let value = |i: &Entity<InputState>| i.read(cx).value().trim().to_string();
        cfg.name = value(&self.f_name);
        cfg.base_url = value(&self.f_url);
        cfg.model = value(&self.f_model);
        let key = value(&self.f_key);
        cfg.api_key = (!key.is_empty()).then_some(key);
        let version = value(&self.f_version);
        cfg.api_version = (!version.is_empty()).then_some(version);
        Some(cfg)
    }

    fn save_provider(&mut self, cx: &mut Context<Self>) {
        let Some(cfg) = self.form(cx) else { return };
        if cfg.name.is_empty() || cfg.base_url.is_empty() || cfg.model.is_empty() {
            self.error = Some(t("ai.provider.incomplete").into());
            cx.notify();
            return;
        }
        let mut list = providers(cx);
        match list.iter_mut().find(|p| p.id == cfg.id) {
            Some(p) => *p = cfg.clone(),
            None => list.push(cfg.clone()),
        }
        let active = AppState::user_settings(cx)
            .ai_active_provider
            .filter(|id| list.iter().any(|p| &p.id == id))
            .or(Some(cfg.id));
        save_providers(&list, active, cx);
        self.editing = None;
        self.error = None;
        cx.notify();
    }

    fn remove_provider(&mut self, id: String, cx: &mut Context<Self>) {
        let list: Vec<_> = providers(cx).into_iter().filter(|p| p.id != id).collect();
        let active = AppState::user_settings(cx)
            .ai_active_provider
            .filter(|a| *a != id)
            .or_else(|| list.first().map(|p| p.id.clone()));
        save_providers(&list, active, cx);
        cx.notify();
    }

    fn test_provider(&mut self, cx: &mut Context<Self>) {
        let Some(cfg) = self.form(cx) else { return };
        self.testing = true;
        self.test = None;
        cx.notify();
        let task = cx.background_spawn(async move {
            let result = cardfile_ai::test_connection(&cfg);
            let models = cardfile_ai::list_models(&cfg).unwrap_or_default();
            (result, models)
        });
        cx.spawn(async move |this, cx| {
            let (result, models) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.testing = false;
                if !models.is_empty() {
                    this.models = models;
                }
                this.test = Some(match result {
                    Ok(()) => (true, t("ai.provider.test_ok").into()),
                    Err(e) => (false, e.to_string().into()),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn render_providers(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let list = providers(cx);
        let active = active_provider(cx).map(|p| p.id);
        let th = cx.theme();
        let mut col = v_flex()
            .gap(px(4.))
            .child(section(t("ai.providers"), cx))
            .child(hint(t("ai.providers_hint"), cx));
        for p in &list {
            let id = p.id.clone();
            let is_active = active.as_deref() == Some(p.id.as_str());
            let (edit_cfg, del_id, act_id) = (p.clone(), id.clone(), id.clone());
            let local = if p.is_local() {
                t("ai.local")
            } else {
                t("ai.cloud")
            };
            col = col.child(
                h_flex()
                    .justify_between()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(th.muted.opacity(0.5))
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .child(format!("{} · {}", p.name, p.model)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(th.muted_foreground)
                                    .child(if p.is_usable() {
                                        local
                                    } else {
                                        t("ai.provider.not_ready")
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .child(
                                Button::new(SharedString::from(format!("use-{id}")))
                                    .xsmall()
                                    .label(if is_active {
                                        t("ai.provider.active")
                                    } else {
                                        t("ai.provider.use")
                                    })
                                    .when(is_active, |b| b.primary())
                                    .when(!is_active, |b| b.outline())
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        let list = providers(cx);
                                        save_providers(&list, Some(act_id.clone()), cx);
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("edit-{id}")))
                                    .xsmall()
                                    .ghost()
                                    .label(t("common.edit"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.edit(edit_cfg.clone(), window, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("del-{id}")))
                                    .xsmall()
                                    .ghost()
                                    .label(t("common.remove"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_provider(del_id.clone(), cx)
                                    })),
                            ),
                    ),
            );
        }
        let view = cx.entity();
        let add = Button::new("add-provider")
            .small()
            .outline()
            .label(t("ai.provider.add"))
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for preset in PRESETS {
                    let view = view.clone();
                    let label = if preset.local {
                        format!("{} ({})", preset.name, t("ai.local"))
                    } else {
                        preset.name.to_owned()
                    };
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                        let mut cfg = ProviderConfig::from_preset(preset);
                        cfg.id = cardfile_core::UserId::new().to_string();
                        view.update(cx, |v, cx| v.edit(cfg, window, cx));
                    }));
                }
                menu
            });
        col = col.child(div().pt(px(6.)).child(add));
        if self.editing.is_some() {
            col = col.child(self.render_editor(cx));
        }
        col.into_any_element()
    }

    fn render_editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(cfg) = self.editing.clone() else {
            return div().into_any_element();
        };
        let custom = cfg.preset == CUSTOM_PRESET_ID;
        let preset = cfg.preset_info();
        let th = cx.theme();
        let view = cx.entity();
        let kind = cfg.kind;
        let kind_menu = Button::new("api-kind")
            .small()
            .outline()
            .label(kind.display_name())
            .dropdown_caret(true)
            .dropdown_menu(move |mut menu, _, _| {
                for k in ApiKind::ALL {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(k.display_name())
                            .checked(k == kind)
                            .on_click(move |_, _, cx| {
                                view.update(cx, |v, cx| {
                                    if let Some(e) = v.editing.as_mut() {
                                        e.kind = k;
                                    }
                                    cx.notify();
                                })
                            }),
                    );
                }
                menu
            });
        let view = cx.entity();
        let models = self.models.clone();
        let model_menu = Button::new("model-pick")
            .small()
            .ghost()
            .icon(gpui_kit::assets::IconName::ChevronDown)
            .tooltip(t("ai.provider.pick_model"))
            .dropdown_menu(move |mut menu, _, _| {
                for m in &models {
                    let view = view.clone();
                    let m = m.clone();
                    menu = menu.item(PopupMenuItem::new(m.clone()).on_click(
                        move |_, window, cx| {
                            view.update(cx, |v, cx| {
                                v.f_model
                                    .update(cx, |s, cx| s.set_value(m.clone(), window, cx))
                            })
                        },
                    ));
                }
                menu
            });
        let needs_key = cfg.requires_api_key();
        let key_url = preset.map(|p| p.key_url).filter(|u| !u.is_empty());
        let mut form = v_flex()
            .gap(px(8.))
            .mt(px(8.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(th.border)
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(preset.map(|p| p.name).unwrap_or("Custom").to_owned()),
            )
            .child(field(t("ai.provider.name"), Input::new(&self.f_name), cx));
        if custom {
            form = form.child(field(t("ai.provider.kind"), kind_menu, cx));
        }
        form = form
            .child(field(t("ai.provider.url"), Input::new(&self.f_url), cx))
            .child(field(
                t("ai.provider.model"),
                h_flex()
                    .gap(px(4.))
                    .child(div().flex_1().child(Input::new(&self.f_model)))
                    .when(!self.models.is_empty(), |d| d.child(model_menu)),
                cx,
            ))
            .child(field(
                if needs_key {
                    t("ai.provider.key")
                } else {
                    t("ai.provider.key_optional")
                },
                Input::new(&self.f_key).mask_toggle(),
                cx,
            ));
        if kind == ApiKind::AzureOpenAi {
            form = form.child(field(
                t("ai.provider.api_version"),
                Input::new(&self.f_version),
                cx,
            ));
        }
        if let Some(url) = key_url {
            form = form.child(
                Button::new("get-key")
                    .xsmall()
                    .link()
                    .label(t("ai.provider.get_key"))
                    .on_click(move |_, _, cx| cx.open_url(url)),
            );
        }
        if let Some((ok, msg)) = &self.test {
            form = form.child(
                div()
                    .text_size(px(12.))
                    .text_color(if *ok { th.success } else { th.danger })
                    .child(msg.clone()),
            );
        }
        form = form.child(error(self.error.as_ref(), cx)).child(
            h_flex()
                .justify_end()
                .gap(px(6.))
                .child(
                    Button::new("cancel-provider")
                        .small()
                        .ghost()
                        .label(t("common.cancel"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editing = None;
                            this.error = None;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("test-provider")
                        .small()
                        .outline()
                        .loading(self.testing)
                        .label(t("ai.provider.test"))
                        .on_click(cx.listener(|this, _, _, cx| this.test_provider(cx))),
                )
                .child(
                    Button::new("save-provider")
                        .small()
                        .primary()
                        .label(t("common.save"))
                        .on_click(cx.listener(|this, _, _, cx| this.save_provider(cx))),
                ),
        );
        form.into_any_element()
    }

    // -----------------------------------------------------------------
    // MCP server and API keys
    // -----------------------------------------------------------------

    fn set_app(cx: &mut App, f: impl FnOnce(&mut cardfile_core::AppSettings)) {
        let state = AppState::get_mut(cx);
        f(&mut state.app);
        let app = state.app.clone();
        let db = AppState::db(cx);
        run_db(cx, move |_| db.save_app_settings(&app)).detach_and_log_err(cx);
        McpServer::sync(cx);
    }

    fn apply_port(&mut self, cx: &mut Context<Self>) {
        let raw = self.port.read(cx).value().trim().to_string();
        match raw.parse::<u16>() {
            Ok(port) if port >= 1024 => {
                self.error = None;
                Self::set_app(cx, |a| a.mcp_port = port);
            }
            _ => self.error = Some(t("ai.mcp.bad_port").into()),
        }
        cx.notify();
    }

    fn create_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(me) = AppState::session(cx).map(|s| s.user.id) else {
            return;
        };
        let name = self.key_name.read(cx).value().trim().to_string();
        let name = if name.is_empty() {
            t("ai.keys.default_name")
        } else {
            name
        };
        let (private, read_only) = (self.key_private, self.key_read_only);
        let db = AppState::db(cx);
        let task = cx.background_spawn(async move {
            let (secret, prefix, hash) = cardfile_crypto::new_api_key()?;
            db.create_api_key(me, &name, &prefix, &hash, private, read_only)?;
            anyhow::Ok(secret.to_string())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(secret) => {
                        let port = AppState::get(cx).app.mcp_port;
                        let url = format!("http://127.0.0.1:{port}/mcp");
                        let snippets = cardfile_mcp::client_config_snippets(&url, &secret);
                        this.new_secret = Some((secret.into(), snippets));
                        this.key_name
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        this.load(cx);
                    }
                    Err(e) => this.error = Some(e.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn revoke(&mut self, key: ApiKey, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        crate::dialogs::confirm(
            t("ai.keys.revoke"),
            t_args("ai.keys.revoke_body", &[("name", &key.name)]),
            t("ai.keys.revoke"),
            window,
            cx,
            move |_, cx| {
                let db = AppState::db(cx);
                let task = run_db(cx, move |_| db.revoke_api_key(key.user_id, key.id));
                let view = view.clone();
                cx.spawn(async move |cx| {
                    let _ = task.await;
                    view.update(cx, |v, cx| v.load(cx));
                })
                .detach();
            },
        );
    }

    fn render_mcp(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let app = AppState::get(cx).app.clone();
        let admin = AppState::session(cx).is_some_and(|s| s.user.is_admin)
            || app.auth_mode != AuthMode::MultiUser;
        let status = McpServer::status(cx);
        let th = cx.theme();
        let status_text = match &status {
            Status::Running(url) => t_args("ai.mcp.running", &[("url", url)]),
            Status::Failed(e) => t_args("ai.mcp.failed", &[("error", e)]),
            Status::Stopped => t("ai.mcp.stopped"),
        };
        let mut col = v_flex()
            .gap(px(6.))
            .child(section(t("ai.mcp.title"), cx))
            .child(hint(t("ai.mcp.hint"), cx))
            .child(
                h_flex()
                    .justify_between()
                    .gap(px(12.))
                    .child(
                        Switch::new("mcp-enabled")
                            .checked(app.mcp_enabled)
                            .label(t("ai.mcp.enable"))
                            .disabled(!admin)
                            .on_click(cx.listener(|_, on: &bool, _, cx| {
                                let on = *on;
                                Self::set_app(cx, |a| a.mcp_enabled = on);
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(div().text_size(px(13.)).child(t("ai.mcp.port")))
                            .child(div().w(px(90.)).child(Input::new(&self.port).small()))
                            .child(
                                Button::new("apply-port")
                                    .xsmall()
                                    .outline()
                                    .disabled(!admin)
                                    .label(t("common.apply"))
                                    .on_click(cx.listener(|this, _, _, cx| this.apply_port(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(match status {
                        Status::Running(_) => th.success,
                        Status::Failed(_) => th.danger,
                        Status::Stopped => th.muted_foreground,
                    })
                    .child(status_text),
            )
            .child(section(t("ai.keys.title"), cx))
            .child(hint(t("ai.keys.hint"), cx));
        for key in &self.keys {
            let k = key.clone();
            let mut flags = Vec::new();
            if key.read_only {
                flags.push(t("ai.keys.read_only"));
            }
            if key.allow_private {
                flags.push(t("ai.keys.private"));
            }
            let used = key
                .last_used_at
                .map(|at| t_args("ai.keys.used", &[("when", &relative_time(at))]))
                .unwrap_or_else(|| t("ai.keys.never_used"));
            col = col.child(
                h_flex()
                    .justify_between()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(th.muted.opacity(0.5))
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .child(format!("{} · {}…", key.name, key.prefix)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(th.muted_foreground)
                                    .child(
                                        std::iter::once(used)
                                            .chain(flags)
                                            .collect::<Vec<_>>()
                                            .join(" · "),
                                    ),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("revoke-{}", key.id)))
                            .xsmall()
                            .ghost()
                            .label(t("ai.keys.revoke"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.revoke(k.clone(), window, cx)
                            })),
                    ),
            );
        }
        col = col.child(
            v_flex()
                .gap(px(6.))
                .pt(px(4.))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(div().flex_1().child(Input::new(&self.key_name).small()))
                        .child(
                            Button::new("create-key")
                                .small()
                                .primary()
                                .label(t("ai.keys.create"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.create_key(window, cx)),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(16.))
                        .child(
                            Checkbox::new("key-read-only")
                                .label(t("ai.keys.read_only_option"))
                                .checked(self.key_read_only)
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    this.key_read_only = *on;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Checkbox::new("key-private")
                                .label(t("ai.keys.private_option"))
                                .checked(self.key_private)
                                .on_click(cx.listener(|this, on: &bool, _, cx| {
                                    this.key_private = *on;
                                    cx.notify();
                                })),
                        ),
                ),
        );
        if let Some((secret, snippets)) = self.new_secret.clone() {
            let mut boxed = v_flex()
                .gap(px(6.))
                .p(px(10.))
                .rounded(px(8.))
                .border_1()
                .border_color(th.warning)
                .child(div().text_size(px(13.)).child(t("ai.keys.copy_now")))
                .child(copy_row("secret", secret.to_string(), cx));
            for (i, (client, snippet)) in snippets.into_iter().enumerate() {
                boxed = boxed
                    .child(
                        div()
                            .pt(px(4.))
                            .text_size(px(12.))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(client),
                    )
                    .child(copy_row(&format!("snippet-{i}"), snippet, cx));
            }
            boxed = boxed.child(
                h_flex().justify_end().child(
                    Button::new("hide-secret")
                        .xsmall()
                        .label(t("common.done"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.new_secret = None;
                            cx.notify();
                        })),
                ),
            );
            col = col.child(boxed);
        }
        col.into_any_element()
    }

    fn render_chat_options(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let s = AppState::user_settings(cx);
        v_flex()
            .gap(px(6.))
            .child(section(t("ai.chat.title"), cx))
            .child(
                Checkbox::new("chat-private")
                    .label(t("ai.chat.allow_private"))
                    .checked(s.ai_chat_allow_private)
                    .on_click(|on: &bool, _, cx| {
                        let on = *on;
                        AppState::update_user_settings(cx, |s| s.ai_chat_allow_private = on);
                    }),
            )
            .child(hint(t("ai.chat.encrypted_note"), cx))
            .into_any_element()
    }

    fn render_activity(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let th = cx.theme();
        let mut col = v_flex()
            .gap(px(2.))
            .child(section(t("ai.activity.title"), cx));
        if self.activity.is_empty() {
            col = col.child(hint(t("ai.activity.empty"), cx));
        }
        for (e, _) in &self.activity {
            let via = e.via.clone().unwrap_or_default();
            col = col.child(
                h_flex()
                    .justify_between()
                    .text_size(px(12.))
                    .child(div().truncate().child(format!(
                        "{} · {} · {}",
                        t(&format!("activity.{}", e.action)),
                        e.summary,
                        via
                    )))
                    .child(
                        div()
                            .text_color(th.muted_foreground)
                            .child(relative_time(e.created_at)),
                    ),
            );
        }
        col.into_any_element()
    }
}

fn copy_row(id: &str, text: String, cx: &App) -> AnyElement {
    let th = cx.theme();
    let copy = text.clone();
    h_flex()
        .gap(px(6.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .bg(th.muted)
                .font_family("monospace")
                .text_size(px(12.))
                .child(text),
        )
        .child(
            Button::new(SharedString::from(format!("copy-{id}")))
                .xsmall()
                .icon(gpui_kit::assets::IconName::Copy)
                .tooltip(t("common.copy"))
                .on_click(move |_, window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                    window.push_notification(Notification::success(t("common.copied")), cx);
                }),
        )
        .into_any_element()
}

impl Render for AiSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap(px(14.))
            .child(self.render_providers(cx))
            .child(self.render_chat_options(cx))
            .child(self.render_mcp(cx))
            .child(self.render_activity(cx))
    }
}
