//! NoteChest's GPUI application.

mod ai_settings;
mod app;
mod assets;
mod card_pane;
mod chat;
mod data_operations;
mod dialogs;
mod dictionaries;
mod export;
mod history;
mod keymap;
mod local_config;
mod lock_picker;
mod locks;
mod login;
mod mcp_host;
mod palette;
mod paths;
mod secrets;
mod settings;
mod share;
mod spell;
mod state;
mod tags;
mod themes;
mod updater;
mod welcome;
mod widgets;
mod wizard;
mod workspace;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, point, px, size,
};

pub use paths::{DataOrigin, DataPaths};

fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../../../packaging/linux/icons/256x256/ar.fittyar.NoteChest.png");
    image::load_from_memory(bytes)
        .ok()
        .map(|i| std::sync::Arc::new(i.to_rgba8()))
}

/// Start the application with the resolved data location. Blocks until quit.
pub fn run(paths: DataPaths) {
    // Startup disk work precedes the UI event loop.
    let initial_config = local_config::LocalConfig::load(&paths.root);
    let initial_db = if paths.has_database() || paths.root.join(".restore-pending").exists() {
        Some(
            notechest_db::Database::open(&paths.db(), Some(&paths.backups()))
                .and_then(|db| db.app_settings().map(|settings| (db, settings))),
        )
    } else {
        None
    };
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            // The system language until a signed-in user's own setting applies, so the
            // first-run wizard and the sign-in screen are not always in English.
            let locale = notechest_i18n::system_locale();
            tracing::info!(%locale, "interface language before sign-in");
            notechest_i18n::set_locale(&locale);
            themes::load_fonts(cx);
            themes::init(&paths, cx);
            themes::apply(themes::DEFAULT_THEME, cx);
            keymap::init(&paths, cx);

            // A folder the user moved the data to is gone (a disconnected drive, a
            // renamed folder): say so instead of starting the wizard on an empty one.
            if initial_db.is_none() && paths.origin == paths::DataOrigin::Moved {
                let pointer = DataPaths::location_file()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                show_startup_error(
                    notechest_i18n::t_args(
                        "app.data_missing",
                        &[
                            ("path", &paths.root.display().to_string()),
                            ("pointer", &pointer),
                        ],
                    ),
                    cx,
                );
                return;
            }
            // Open the database if it already exists; the wizard creates it otherwise.
            let (db, app_settings) = if let Some(initial_db) = initial_db {
                match initial_db {
                    Ok((db, settings)) => (Some(db), settings),
                    Err(e) => {
                        tracing::error!("cannot open database: {e:#}");
                        eprintln!(
                            "NoteChest: cannot open database {}: {e}",
                            paths.db().display()
                        );
                        // Release builds on Windows have no console: say it in a window.
                        show_startup_error(
                            notechest_i18n::t_args(
                                "app.db_error_body",
                                &[
                                    ("path", &paths.db().display().to_string()),
                                    ("error", &format!("{e:#}")),
                                ],
                            ),
                            cx,
                        );
                        return;
                    }
                }
            } else {
                (None, Default::default())
            };
            // Background work that must not run on the UI thread later (render).
            updater::detect_kind(cx);
            export::cleanup_stale_attachments(cx);
            cx.set_global(state::AppState {
                paths: paths.clone(),
                db,
                app: app_settings,
                session: None,
            });

            // Visible areas of the connected displays, to reopen the window where it
            // was (if that display is still there) at a size that fits it.
            let screens = cx.displays();
            let displays: Vec<(f32, f32, f32, f32)> = screens
                .iter()
                .map(|d| {
                    let b = d.visible_bounds();
                    (
                        b.origin.x.into(),
                        b.origin.y.into(),
                        b.size.width.into(),
                        b.size.height.into(),
                    )
                })
                .collect();
            // GPUI only honours a position that lies on the display it is told to
            // use (the primary one by default), so name the display explicitly.
            let home = initial_config.window.display(&displays);
            let home_id = home.and_then(|rect| {
                displays
                    .iter()
                    .position(|d| *d == rect)
                    .map(|i| screens[i].id())
            });
            let primary = cx
                .primary_display()
                .map(|d| d.visible_bounds().size)
                .unwrap_or(size(px(1180.), px(760.)));
            let available = home.map_or((primary.width.into(), primary.height.into()), |d| {
                (d.2, d.3)
            });
            let (width, height) = initial_config.window.clamped(available);
            let origin = initial_config.window.origin((width, height), &displays);
            let maximized = initial_config.window.maximized;
            let prefs = local_config::LocalPreferences {
                root: paths.root.clone(),
                config: std::sync::Arc::new(parking_lot::Mutex::new(initial_config)),
                writer: Default::default(),
                generation: Default::default(),
            };
            cx.set_global(prefs.clone());
            // Establish a stable collection ID even before a first backup.
            local_config::LocalPreferences::change(cx, |_| {});
            let bounds = match origin {
                Some((x, y)) => Bounds {
                    origin: point(px(x), px(y)),
                    size: size(px(width), px(height)),
                },
                None => Bounds::centered(None, size(px(width), px(height)), cx),
            };
            // The app draws its own title bar (like Zed): no native frame.
            let options = WindowOptions {
                window_bounds: Some(if maximized {
                    WindowBounds::Maximized(bounds)
                } else {
                    WindowBounds::Windowed(bounds)
                }),
                display_id: home_id,
                titlebar: Some(TitlebarOptions {
                    title: Some("NoteChest".into()),
                    ..gpui_kit::component::TitleBar::title_bar_options()
                }),
                window_decorations: Some(gpui_kit::WindowDecorations::Client),
                // Opaque like Zed: the Linux client-side shadow margin is still drawn
                // see-through (Wayland always composites client-decorated surfaces with
                // alpha, X11 uses an ARGB visual), and a non-opaque window would turn
                // off subpixel text rendering.
                window_background: gpui_kit::WindowBackgroundAppearance::Opaque,
                app_id: Some("ar.fittyar.NoteChest".into()),
                window_min_size: Some(size(
                    px(760.).min(px(available.0)),
                    px(480.).min(px(available.1)),
                )),
                // Used by X11; Windows takes the icon from the executable's resources.
                icon: window_icon(),
                ..gpui_kit::component::TitleBar::window_options()
            };
            if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::NoteChestApp::new(window, cx))
            }) {
                tracing::error!("open window: {e:#}");
                cx.quit();
            }
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}

/// A window explaining why NoteChest cannot start; quitting closes the app.
struct StartupError {
    message: SharedString,
}

impl Render for StartupError {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = cx.theme();
        v_flex()
            .size_full()
            .p(px(20.))
            .gap(px(12.))
            .bg(th.background)
            .text_color(th.foreground)
            .font_family(th.font_family.clone())
            .child(
                div()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(notechest_i18n::t("app.db_error_title")),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.))
                    .child(self.message.clone()),
            )
            .child(
                h_flex().justify_end().child(
                    Button::new("startup-error-quit")
                        .primary()
                        .label(notechest_i18n::t("menu.quit"))
                        .on_click(|_, _, cx| cx.quit()),
                ),
            )
    }
}

/// Show `message` in a small window and quit when it is closed.
fn show_startup_error(message: String, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(560.), px(240.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("NoteChest".into()),
            ..Default::default()
        }),
        app_id: Some("ar.fittyar.NoteChest".into()),
        icon: window_icon(),
        ..Default::default()
    };
    let message = SharedString::from(message);
    if let Err(e) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| StartupError { message }))
    {
        tracing::error!("open error window: {e:#}");
        cx.quit();
        return;
    }
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    cx.activate(true);
}
