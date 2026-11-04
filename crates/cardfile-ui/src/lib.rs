//! Cardfile's GPUI application.

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
pub mod keymap;
mod local_config;
mod locks;
mod login;
mod mcp_host;
mod palette;
pub mod paths;
mod secrets;
mod settings;
mod share;
mod spell;
mod state;
mod tags;
pub mod themes;
mod updater;
mod welcome;
mod widgets;
mod wizard;
mod workspace;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, TitlebarOptions, Window, WindowBounds, WindowOptions, div, px, size,
};

pub use paths::DataPaths;

fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    let bytes = include_bytes!("../../../packaging/linux/icons/256x256/ar.fittyar.Cardfile.png");
    image::load_from_memory(bytes)
        .ok()
        .map(|i| std::sync::Arc::new(i.to_rgba8()))
}

/// Start the application with the resolved data location. Blocks until quit.
pub fn run(paths: DataPaths) {
    // Startup disk work precedes the UI event loop.
    let initial_config = local_config::LocalConfig::load(&paths.root);
    let initial_db = if paths.db().exists() || paths.root.join(".restore-pending").exists() {
        Some(
            cardfile_db::Database::open(&paths.db(), Some(&paths.backups()))
                .and_then(|db| db.app_settings().map(|settings| (db, settings))),
        )
    } else {
        None
    };
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cardfile_i18n::set_locale("en");
            themes::load_fonts(cx);
            themes::init(&paths, cx);
            themes::apply(themes::DEFAULT_THEME, cx);
            keymap::init(&paths, cx);

            // Open the database if it already exists; the wizard creates it otherwise.
            let (db, app_settings) = if let Some(initial_db) = initial_db {
                match initial_db {
                    Ok((db, settings)) => (Some(db), settings),
                    Err(e) => {
                        tracing::error!("cannot open database: {e:#}");
                        eprintln!(
                            "Cardfile: cannot open database {}: {e}",
                            paths.db().display()
                        );
                        // Release builds on Windows have no console: say it in a window.
                        show_startup_error(
                            cardfile_i18n::t_args(
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

            let available = cx
                .primary_display()
                .map(|d| d.visible_bounds().size)
                .unwrap_or(size(px(1180.), px(760.)));
            let (width, height) = initial_config
                .window
                .clamped((available.width.into(), available.height.into()));
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
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            // The app draws its own title bar (like Zed): no native frame.
            let options = WindowOptions {
                window_bounds: Some(if maximized {
                    WindowBounds::Maximized(bounds)
                } else {
                    WindowBounds::Windowed(bounds)
                }),
                titlebar: Some(TitlebarOptions {
                    title: Some("Cardfile".into()),
                    ..gpui_kit::component::TitleBar::title_bar_options()
                }),
                window_decorations: Some(gpui_kit::WindowDecorations::Client),
                // Opaque like Zed: the Linux client-side shadow margin is still drawn
                // see-through (Wayland always composites client-decorated surfaces with
                // alpha, X11 uses an ARGB visual), and a non-opaque window would turn
                // off subpixel text rendering.
                window_background: gpui_kit::WindowBackgroundAppearance::Opaque,
                app_id: Some("ar.fittyar.Cardfile".into()),
                window_min_size: Some(size(
                    px(760.).min(available.width),
                    px(480.).min(available.height),
                )),
                // Used by X11; Windows takes the icon from the executable's resources.
                icon: window_icon(),
                ..gpui_kit::component::TitleBar::window_options()
            };
            if let Err(e) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::CardfileApp::new(window, cx))
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

/// A window explaining why Cardfile cannot start; quitting closes the app.
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
                    .child(cardfile_i18n::t("app.db_error_title")),
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
                        .label(cardfile_i18n::t("menu.quit"))
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
            title: Some("Cardfile".into()),
            ..Default::default()
        }),
        app_id: Some("ar.fittyar.Cardfile".into()),
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
