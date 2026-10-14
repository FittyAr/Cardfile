//! New-version notifications. Disabled at compile time for store/package builds
//! (feature `update-check`), where the platform handles updates.

use cardfile_i18n::{t, t_args};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, AppContext as _, Window};

use crate::state::AppState;

pub fn enabled() -> bool {
    cfg!(feature = "update-check") && !managed_install()
}

/// Installed by the Microsoft Store (MSIX) or a Linux package manager, which update
/// the app themselves.
fn managed_install() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let path = exe.to_string_lossy().to_string();
    path.contains(r"\WindowsApps\")
        || path.starts_with("/usr/")
        || std::env::var_os("FLATPAK_ID").is_some()
        || std::env::var_os("SNAP").is_some()
}

fn current() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// Check in the background after sign-in if the user allows it.
pub fn check_on_startup(window: &mut Window, cx: &mut App) {
    if enabled() && AppState::user_settings(cx).check_for_updates {
        run(false, window, cx);
    }
}

/// Check now and always report the result.
pub fn check_now(window: &mut Window, cx: &mut App) {
    if !enabled() {
        window.push_notification(Notification::info(t("updates.managed")), cx);
        return;
    }
    run(true, window, cx);
}

fn run(verbose: bool, window: &mut Window, cx: &mut App) {
    let task = cx.background_spawn(async move { cardfile_updater::check(&current()) });
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| match result {
                Ok(Some(release)) => {
                    let url = release.url.clone();
                    window.push_notification(
                        Notification::info(t_args(
                            "updates.available",
                            &[("version", &release.version.to_string())],
                        ))
                        .title(t("updates.title"))
                        .autohide(false)
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                        cx,
                    );
                }
                Ok(None) if verbose => {
                    window.push_notification(Notification::success(t("updates.latest")), cx)
                }
                Err(e) if verbose => window.push_notification(
                    Notification::error(t_args("updates.failed", &[("error", &e.to_string())])),
                    cx,
                ),
                _ => {}
            });
        })
        .detach();
}
