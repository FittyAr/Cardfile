//! Settings → AI: AI services for the assistant, the local MCP server and the API keys
//! AI clients use, and what the AI did recently.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::Div;
use gpui_kit::assets::IconName;
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
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};
use notechest_ai::{ApiKind, CUSTOM_PRESET_ID, PRESETS, Preset, ProviderConfig};
use notechest_core::{ActivityEntry, ApiKey, AuthMode, UserId};
use notechest_i18n::{t, t_args};

use crate::mcp_host::{McpServer, Status};
use crate::secrets::AiKeys;
use crate::state::{AppState, run_db};
use crate::widgets::{error, field, hint, relative_time, section};
use zeroize::Zeroizing;

/// The parsed provider list of a settings snapshot. Parsing the JSON on every
/// render was measurable, so the list is parsed once per change of the raw settings.
struct ParsedProviders {
    user: UserId,
    raw: Vec<serde_json::Value>,
    parsed: Rc<[ProviderConfig]>,
}

thread_local! {
    // GPUI runs on one thread; the cache lives where `App` lives.
    static PARSED: RefCell<Option<ParsedProviders>> = const { RefCell::new(None) };
}

/// The signed-in user's providers as stored in the settings (keys kept in the
/// credential store are not filled in). Re-parsed only when the settings change.
fn parsed_providers(cx: &App) -> Rc<[ProviderConfig]> {
    let Some(session) = AppState::session(cx) else {
        return Rc::from([]);
    };
    let (user, raw) = (session.user.id, &session.settings.ai_providers);
    PARSED.with(|cell| {
        let mut cache = cell.borrow_mut();
        if let Some(c) = cache.as_ref().filter(|c| c.user == user && c.raw == *raw) {
            return c.parsed.clone();
        }
        let parsed: Rc<[ProviderConfig]> = raw
            .iter()
            .filter_map(|v| serde_json::from_value::<ProviderConfig>(v.clone()).ok())
            .collect();
        *cache = Some(ParsedProviders {
            user,
            raw: raw.clone(),
            parsed: parsed.clone(),
        });
        parsed
    })
}

/// `p` with its API key from the credential store, if the settings hold none.
fn with_key(mut p: ProviderConfig, cx: &App) -> ProviderConfig {
    if p.api_key.is_none() {
        p.api_key = AiKeys::get(cx, &p.id).map(|k| k.as_str().to_owned());
    }
    p
}

/// Whether `p` can send requests, counting a key held in the credential store
/// without copying it.
fn is_usable(p: &ProviderConfig, cx: &App) -> bool {
    if p.key().is_some() || !p.requires_api_key() {
        return p.is_usable();
    }
    if !AiKeys::has(cx, &p.id) {
        return false;
    }
    let probe = ProviderConfig {
        api_key: Some("set".to_owned()),
        ..p.clone()
    };
    probe.is_usable()
}

/// The user's configured AI services, with their API keys from the credential store.
pub fn providers(cx: &App) -> Vec<ProviderConfig> {
    parsed_providers(cx)
        .iter()
        .map(|p| with_key(p.clone(), cx))
        .collect()
}

/// Load the signed-in user's AI keys from the OS credential store (in the
/// background), moving keys that older versions kept in the settings there.
///
/// A plaintext key is removed from the settings only for the providers whose key
/// the credential store accepted; the others keep it and are retried on the next
/// load.
pub fn load_keys(cx: &mut App) {
    let Some(session) = AppState::session(cx).cloned() else {
        return;
    };
    let configs: Vec<ProviderConfig> = session
        .settings
        .ai_providers
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    let user = session.user.id;
    let task = cx.background_spawn(async move {
        let ok = crate::secrets::store_available();
        let mut keys = std::collections::HashMap::new();
        let mut migrated = HashSet::new();
        for p in &configs {
            if let Some(k) = p.api_key.as_deref().filter(|k| !k.is_empty()) {
                if ok {
                    match crate::secrets::write(user, &p.id, Some(k)) {
                        Ok(()) => {
                            migrated.insert(p.id.clone());
                        }
                        Err(e) => tracing::warn!("credential store, key kept in settings: {e}"),
                    }
                }
                keys.insert(p.id.clone(), Zeroizing::new(k.to_owned()));
            } else if ok && let Some(k) = crate::secrets::read(user, &p.id) {
                keys.insert(p.id.clone(), k);
            }
        }
        (keys, ok, migrated)
    });
    cx.spawn(async move |cx| {
        let (keys, ok, migrated) = task.await;
        cx.update(|cx| {
            AiKeys::replace(cx, keys, ok);
            if !migrated.is_empty() {
                // Drop the plaintext keys that are now in the credential store.
                AppState::update_user_settings(cx, |s| {
                    set_settings_keys(&mut s.ai_providers, |id| {
                        migrated.contains(id).then_some(None)
                    });
                });
            }
            cx.refresh_windows();
        });
    })
    .detach();
}

/// Change the `api_key` of the stored provider entries: `f` returns the new key
/// for an id (`Some(None)` clears it) or `None` to leave the entry alone.
fn set_settings_keys(
    providers: &mut [serde_json::Value],
    f: impl Fn(&str) -> Option<Option<String>>,
) {
    for v in providers {
        let Some(obj) = v.as_object_mut() else {
            continue;
        };
        let Some(key) = obj.get("id").and_then(|id| id.as_str()).and_then(&f) else {
            continue;
        };
        obj.insert(
            "api_key".to_owned(),
            key.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    }
}

/// The service the assistant uses, if one is configured and usable. The assistant
/// UI is shown only when this is `Some`.
pub fn active_provider(cx: &App) -> Option<ProviderConfig> {
    find_active(cx).map(|p| with_key(p, cx))
}

/// Whether an AI service is configured and usable ([`active_provider`] without
/// copying its API key; meant for render paths).
pub fn has_active_provider(cx: &App) -> bool {
    find_active(cx).is_some()
}

/// The active (or else first usable) provider, without its credential-store key.
fn find_active(cx: &App) -> Option<ProviderConfig> {
    let all = parsed_providers(cx);
    let active = AppState::session(cx).and_then(|s| s.settings.ai_active_provider.as_deref());
    pick_active(&all, active, |p| is_usable(p, cx)).cloned()
}

/// The provider with id `active` when it is usable, else the first usable one.
fn pick_active<'a>(
    all: &'a [ProviderConfig],
    active: Option<&str>,
    usable: impl Fn(&ProviderConfig) -> bool,
) -> Option<&'a ProviderConfig> {
    active
        .and_then(|id| all.iter().find(|p| p.id == id && usable(p)))
        .or_else(|| all.iter().find(|p| usable(p)))
}

/// Bumped by every [`save_providers`]; a background save that finishes after a newer
/// one started leaves the settings to the newer one.
static SAVE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Persist the provider list. API keys go to the OS credential store first; the
/// settings are written only afterwards, keeping in plain text just the keys the
/// store refused (or all of them when it is unavailable). If the app dies in between,
/// the old settings stay as they were, so a key is never in neither place.
fn save_providers(list: &[ProviderConfig], active: Option<String>, cx: &mut App) {
    let store_ok = AiKeys::store_ok(cx);
    let old: Vec<String> = parsed_providers(cx).iter().map(|p| p.id.clone()).collect();
    let mut writes: Vec<(String, Option<Zeroizing<String>>)> = Vec::new();
    for p in list {
        AiKeys::set(cx, &p.id, p.api_key.clone());
        writes.push((p.id.clone(), p.api_key.clone().map(Zeroizing::new)));
    }
    for id in old
        .into_iter()
        .filter(|id| !list.iter().any(|p| &p.id == id))
    {
        AiKeys::set(cx, &id, None);
        writes.push((id, None));
    }
    let session_user = AppState::session(cx).map(|s| s.user.id);
    let (true, Some(user)) = (store_ok, session_user) else {
        persist_providers(list, active, &HashSet::new(), cx);
        return;
    };
    let seq = SAVE_SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let list = list.to_vec();
    let task = cx.background_spawn(async move {
        let mut stored = HashSet::new();
        for (id, key) in writes {
            match crate::secrets::write(user, &id, key.as_deref().map(String::as_str)) {
                Ok(()) => {
                    stored.insert(id);
                }
                Err(e) => tracing::warn!("credential store, key kept in settings: {e}"),
            }
        }
        stored
    });
    cx.spawn(async move |cx| {
        let stored = task.await;
        cx.update(|cx| {
            if SAVE_SEQ.load(std::sync::atomic::Ordering::SeqCst) != seq {
                return; // A newer save persists the settings.
            }
            persist_providers(&list, active, &stored, cx);
        });
    })
    .detach();
}

/// Write the provider list to the settings, without the API keys of the providers in
/// `in_store` (those live in the OS credential store).
fn persist_providers(
    list: &[ProviderConfig],
    active: Option<String>,
    in_store: &HashSet<String>,
    cx: &mut App,
) {
    let values: Vec<serde_json::Value> = list
        .iter()
        .map(|p| {
            let mut p = p.clone();
            if in_store.contains(&p.id) {
                p.api_key = None;
            }
            p
        })
        .filter_map(|p| serde_json::to_value(p).ok())
        .collect();
    AppState::update_user_settings(cx, |s| {
        s.ai_providers = values;
        s.ai_active_provider = active;
    });
    cx.refresh_windows();
}

/// Which part of the AI settings is shown (each is its own page in Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiPage {
    Services,
    Assistant,
    Mcp,
}

pub struct AiSettings {
    page: AiPage,
    editing: Option<ProviderConfig>,
    /// Show the endpoint and API format fields.
    advanced: bool,
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
            page: AiPage::Services,
            editing: None,
            advanced: false,
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

    pub fn set_page(&mut self, page: AiPage, cx: &mut Context<Self>) {
        self.page = page;
        if page != AiPage::Services {
            self.editing = None;
        }
        self.load(cx);
        cx.notify();
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
            let result = notechest_ai::test_connection(&cfg);
            let models = notechest_ai::list_models(&cfg).unwrap_or_default();
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
        if self.editing.is_some() {
            return self.render_editor(cx);
        }
        let list = providers(cx);
        let active = active_provider(cx).map(|p| p.id);
        let th = cx.theme().clone();
        let mut col = v_flex().gap(px(8.)).child(hint(t("ai.providers_hint"), cx));

        // 1. What is already connected.
        col = col.child(section(t("ai.services.yours"), cx));
        if list.is_empty() {
            col = col.child(
                div()
                    .p(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(th.border)
                    .text_size(px(13.))
                    .text_color(th.muted_foreground)
                    .child(t("ai.services.none")),
            );
        }
        for p in &list {
            let id = p.id.clone();
            let is_active = active.as_deref() == Some(p.id.as_str());
            let (edit_cfg, del_id, act_id) = (p.clone(), id.clone(), id.clone());
            let ready = p.is_usable();
            let status = if !ready {
                (t("ai.provider.not_ready"), th.warning)
            } else if p.is_local() {
                (t("ai.services.ready_local"), th.success)
            } else {
                (t("ai.services.ready"), th.success)
            };
            col = col.child(
                h_flex()
                    .justify_between()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(if is_active { th.primary } else { th.border })
                    .child(
                        v_flex()
                            .gap(px(2.))
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(gpui_kit::FontWeight::MEDIUM)
                                            .child(p.name.clone()),
                                    )
                                    .when(is_active, |d| {
                                        d.child(
                                            div()
                                                .px(px(6.))
                                                .rounded_full()
                                                .bg(th.primary)
                                                .text_color(th.primary_foreground)
                                                .text_size(px(11.))
                                                .child(t("ai.provider.active")),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .text_size(px(12.))
                                    .child(div().size(px(7.)).rounded_full().bg(status.1))
                                    .child(
                                        div()
                                            .text_color(th.muted_foreground)
                                            .child(format!("{} · {}", status.0, p.model)),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(4.))
                            .flex_shrink_0()
                            .when(!is_active && ready, |d| {
                                d.child(
                                    Button::new(SharedString::from(format!("use-{id}")))
                                        .xsmall()
                                        .outline()
                                        .label(t("ai.provider.use"))
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            let list = providers(cx);
                                            save_providers(&list, Some(act_id.clone()), cx);
                                        })),
                                )
                            })
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

        // 2. Pick a new service: cloud or on this computer.
        col = col.child(section(t("ai.services.add"), cx));
        for (local, title, desc) in [
            (
                false,
                t("ai.services.cloud_title"),
                t("ai.services.cloud_hint"),
            ),
            (
                true,
                t("ai.services.local_title"),
                t("ai.services.local_hint"),
            ),
        ] {
            let mut grid = h_flex().flex_wrap().gap(px(8.));
            for preset in PRESETS
                .iter()
                .filter(|p| p.local == local && p.id != CUSTOM_PRESET_ID)
            {
                grid = grid.child(self.preset_tile(preset, cx));
            }
            if !local && let Some(custom) = PRESETS.iter().find(|p| p.id == CUSTOM_PRESET_ID) {
                grid = grid.child(self.preset_tile(custom, cx));
            }
            col = col
                .child(
                    div()
                        .pt(px(4.))
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(title),
                )
                .child(hint(desc, cx))
                .child(grid);
        }
        if !AiKeys::store_ok(cx) {
            col = col.child(
                div()
                    .pt(px(6.))
                    .text_size(px(12.))
                    .text_color(th.warning)
                    .child(t("ai.services.no_keyring")),
            );
        }
        col.into_any_element()
    }

    fn preset_tile(&self, preset: &'static Preset, cx: &mut Context<Self>) -> AnyElement {
        let th = cx.theme();
        let detail = if preset.id == CUSTOM_PRESET_ID {
            t("ai.services.custom_hint")
        } else if preset.default_model.is_empty() {
            String::new()
        } else {
            preset.default_model.to_owned()
        };
        h_flex()
            .id(SharedString::from(format!("preset-{}", preset.id)))
            .w(px(180.))
            .gap(px(8.))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(8.))
            .border_1()
            .border_color(th.border)
            .cursor_pointer()
            .hover(|d| d.bg(th.accent).border_color(th.primary))
            .child(
                gpui_kit::component::Icon::new(if preset.local {
                    IconName::Bot
                } else {
                    IconName::Sparkles
                })
                .size_4()
                .text_color(th.primary),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .child(div().text_size(px(13.)).truncate().child(
                        if preset.id == CUSTOM_PRESET_ID {
                            t("ai.services.custom")
                        } else {
                            preset.name.to_owned()
                        },
                    ))
                    .child(
                        div()
                            .text_size(px(11.))
                            .truncate()
                            .text_color(th.muted_foreground)
                            .child(detail),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                let mut cfg = ProviderConfig::from_preset(preset);
                cfg.id = notechest_core::UserId::new().to_string();
                this.advanced = preset.id == CUSTOM_PRESET_ID;
                this.edit(cfg, window, cx);
            }))
            .into_any_element()
    }

    fn render_editor(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(cfg) = self.editing.clone() else {
            return div().into_any_element();
        };
        let custom = cfg.preset == CUSTOM_PRESET_ID;
        let preset = cfg.preset_info();
        let is_new = !providers(cx).iter().any(|p| p.id == cfg.id);
        let kind = cfg.kind;
        let needs_key = cfg.requires_api_key() || custom;
        let local = preset.is_some_and(|p| p.local);
        let key_url = preset.map(|p| p.key_url).filter(|u| !u.is_empty());
        let name = preset
            .map(|p| p.name.to_owned())
            .filter(|_| !custom)
            .unwrap_or_else(|| t("ai.services.custom"));

        // Step-by-step form: key, model, name; technical details under "Advanced".
        let mut form = v_flex()
            .gap(px(12.))
            .child(Self::editor_heading(is_new, &name, cx));
        if local {
            form = form.child(hint(t("ai.services.local_steps"), cx));
        }
        if needs_key || !local {
            form = form.child(self.key_step(needs_key, key_url, cx));
        }
        form = form
            .child(step(
                "2",
                t("ai.provider.model"),
                Some(t("ai.services.model_hint")),
                h_flex()
                    .gap(px(8.))
                    .child(div().flex_1().child(Input::new(&self.f_model)))
                    .when(!self.models.is_empty(), |d| d.child(self.model_menu(cx)))
                    .into_any_element(),
                cx,
            ))
            .child(step(
                "3",
                t("ai.provider.name"),
                None,
                Input::new(&self.f_name).into_any_element(),
                cx,
            ));
        // Advanced: endpoint and wire format, needed for custom and Azure.
        let advanced = self.advanced || custom || kind == ApiKind::AzureOpenAi;
        form = form.child(Self::advanced_toggle(advanced, cx));
        if advanced {
            form = form.child(self.advanced_fields(kind, cx));
        }
        if let Some((ok, msg)) = &self.test {
            form = form.child(
                div()
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(if *ok {
                        cx.theme().success
                    } else {
                        cx.theme().danger
                    }
                    .opacity(0.12))
                    .text_size(px(12.))
                    .child(msg.clone()),
            );
        }
        form = form
            .child(error(self.error.as_ref(), cx))
            .child(self.editor_actions(cx));
        form.into_any_element()
    }

    /// The wire format dropdown of the advanced section.
    fn kind_menu(kind: ApiKind, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        Button::new("api-kind")
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
            })
    }

    /// Fill the model field from the models the service listed.
    fn model_menu(&self, cx: &Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let models = self.models.clone();
        Button::new("model-pick")
            .small()
            .outline()
            .label(t("ai.provider.pick_model"))
            .dropdown_caret(true)
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
            })
    }

    /// "Back" and the "Connect / Edit <service>" title.
    fn editor_heading(is_new: bool, name: &str, cx: &Context<Self>) -> Div {
        h_flex()
            .gap(px(8.))
            .child(
                Button::new("back-services")
                    .small()
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .label(t("common.back"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editing = None;
                        this.error = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(if is_new {
                        t_args("ai.services.connect", &[("name", name)])
                    } else {
                        t_args("ai.services.edit", &[("name", name)])
                    }),
            )
    }

    /// Step 1: the API key, with a link to get one when the preset has it.
    fn key_step(
        &self,
        needs_key: bool,
        key_url: Option<&'static str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mut key_row = h_flex()
            .gap(px(8.))
            .child(div().flex_1().child(Input::new(&self.f_key).mask_toggle()));
        if let Some(url) = key_url {
            key_row = key_row.child(
                Button::new("get-key")
                    .small()
                    .outline()
                    .icon(IconName::ExternalLink)
                    .label(t("ai.provider.get_key"))
                    .on_click(move |_, _, cx| cx.open_url(url)),
            );
        }
        step(
            "1",
            if needs_key {
                t("ai.provider.key")
            } else {
                t("ai.provider.key_optional")
            },
            Some(t("ai.services.key_hint")),
            key_row.into_any_element(),
            cx,
        )
    }

    /// Shows or hides the advanced section.
    fn advanced_toggle(advanced: bool, cx: &Context<Self>) -> impl IntoElement {
        Button::new("toggle-advanced")
            .xsmall()
            .ghost()
            .icon(if advanced {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .label(t("ai.services.advanced"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.advanced = !this.advanced;
                cx.notify();
            }))
    }

    /// Endpoint, wire format and (for Azure) API version.
    fn advanced_fields(&self, kind: ApiKind, cx: &Context<Self>) -> Div {
        let kind_menu = Self::kind_menu(kind, cx);
        let th = cx.theme();
        let mut adv = v_flex()
            .gap(px(8.))
            .pl(px(12.))
            .border_l_2()
            .border_color(th.border)
            .child(field(t("ai.provider.url"), Input::new(&self.f_url), cx))
            .child(field(t("ai.provider.kind"), kind_menu, cx));
        if kind == ApiKind::AzureOpenAi {
            adv = adv.child(field(
                t("ai.provider.api_version"),
                Input::new(&self.f_version),
                cx,
            ));
        }
        adv
    }

    /// "Test" and "Save".
    fn editor_actions(&self, cx: &Context<Self>) -> Div {
        h_flex()
            .justify_end()
            .gap(px(6.))
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
            )
    }

    // -----------------------------------------------------------------
    // MCP server and API keys
    // -----------------------------------------------------------------

    fn set_app(cx: &mut App, f: impl FnOnce(&mut notechest_core::AppSettings)) {
        f(&mut AppState::get_mut(cx).app);
        AppState::save_app_settings(cx);
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
            let (secret, prefix, hash) = notechest_crypto::new_api_key()?;
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
                        let snippets = notechest_mcp::client_config_snippets(&url, &secret);
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

/// A numbered step of the "connect a service" form.
fn step(n: &str, label: String, help: Option<String>, control: AnyElement, cx: &App) -> AnyElement {
    let th = cx.theme();
    h_flex()
        .items_start()
        .gap(px(10.))
        .child(
            div()
                .mt(px(2.))
                .size(px(20.))
                .flex_shrink_0()
                .rounded_full()
                .bg(th.primary)
                .text_color(th.primary_foreground)
                .text_size(px(11.))
                .flex()
                .items_center()
                .justify_center()
                .child(n.to_owned()),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(label),
                )
                .children(help.map(|h| hint(h, cx)))
                .child(control),
        )
        .into_any_element()
}

impl Render for AiSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.page {
            AiPage::Services => self.render_providers(cx),
            AiPage::Assistant => v_flex()
                .gap(px(14.))
                .child(self.render_chat_options(cx))
                .child(self.render_activity(cx))
                .into_any_element(),
            AiPage::Mcp => self.render_mcp(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.into(),
            ..Default::default()
        }
    }

    fn ids(p: Option<&ProviderConfig>) -> Option<&str> {
        p.map(|p| p.id.as_str())
    }

    #[test]
    fn the_active_provider_wins_when_usable() {
        let all = [provider("a"), provider("b"), provider("c")];
        assert_eq!(ids(pick_active(&all, Some("b"), |_| true)), Some("b"));
    }

    #[test]
    fn an_unusable_or_unknown_active_provider_falls_back_to_the_first_usable() {
        let all = [provider("a"), provider("b"), provider("c")];
        let usable = |p: &ProviderConfig| p.id != "a" && p.id != "c";
        assert_eq!(ids(pick_active(&all, Some("c"), usable)), Some("b"));
        assert_eq!(ids(pick_active(&all, Some("gone"), usable)), Some("b"));
        assert_eq!(ids(pick_active(&all, None, usable)), Some("b"));
    }

    #[test]
    fn nothing_usable_means_no_provider() {
        let all = [provider("a")];
        assert_eq!(ids(pick_active(&all, Some("a"), |_| false)), None);
        assert_eq!(ids(pick_active(&[], Some("a"), |_| true)), None);
    }
}
