//! Cardfile's GPUI application.

mod app;
mod assets;
mod card_pane;
mod dialogs;
mod export;
mod history;
pub mod keymap;
mod login;
mod palette;
pub mod paths;
mod settings;
mod spell;
mod state;
mod tags;
pub mod themes;
mod updater;
mod welcome;
mod widgets;
mod wizard;
mod workspace;

use gpui_kit::{AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

pub use paths::DataPaths;

/// Start the application with the resolved data location. Blocks until quit.
pub fn run(paths: DataPaths) {
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            cardfile_i18n::set_locale("en");
            themes::init(&paths, cx);
            themes::apply(themes::DEFAULT_THEME, cx);
            keymap::init(&paths, cx);

            // Open the database if it already exists; the wizard creates it otherwise.
            let (db, app_settings) = if paths.db().exists() {
                match cardfile_db::Database::open(&paths.db(), Some(&paths.backups())) {
                    Ok(db) => {
                        let settings = db.app_settings().unwrap_or_default();
                        (Some(db), settings)
                    }
                    Err(e) => {
                        tracing::error!("cannot open database: {e:#}");
                        eprintln!(
                            "Cardfile: cannot open database {}: {e}",
                            paths.db().display()
                        );
                        cx.quit();
                        return;
                    }
                }
            } else {
                (None, Default::default())
            };
            cx.set_global(state::AppState {
                paths: paths.clone(),
                db,
                app: app_settings,
                session: None,
                card_password: None,
            });

            let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Cardfile".into()),
                    ..Default::default()
                }),
                app_id: Some("ar.fittyar.Cardfile".into()),
                window_min_size: Some(size(px(760.), px(480.))),
                ..Default::default()
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
