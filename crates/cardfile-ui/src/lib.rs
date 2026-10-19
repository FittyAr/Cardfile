//! Cardfile's GPUI application.

mod ai_settings;
mod app;
mod assets;
mod card_pane;
mod chat;
mod dialogs;
mod export;
mod history;
pub mod keymap;
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
            let (db, app_settings) = if paths.db().exists() {
                match cardfile_db::Database::open(&paths.db(), Some(&paths.backups())) {
                    Ok(db) => match db.app_settings() {
                        Ok(settings) => (Some(db), settings),
                        // Defaults would re-run the first-run wizard on existing data.
                        Err(e) => {
                            tracing::error!("cannot read settings: {e:#}");
                            eprintln!("Cardfile: cannot read settings: {e}");
                            cx.quit();
                            return;
                        }
                    },
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

            let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
            // The app draws its own title bar (like Zed): no native frame.
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Cardfile".into()),
                    ..gpui_kit::component::TitleBar::title_bar_options()
                }),
                window_decorations: Some(gpui_kit::WindowDecorations::Client),
                app_id: Some("ar.fittyar.Cardfile".into()),
                window_min_size: Some(size(px(760.), px(480.))),
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
