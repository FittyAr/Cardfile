//! Updates: check GitHub Releases and, where Cardfile installed itself (installer,
//! standalone binary, AppImage, portable), download, verify and install the new
//! version, then restart. Store and package-manager installs only get a notice.
//! Everything can be disabled at compile time (feature `update-check`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use cardfile_i18n::{t, t_args};
use cardfile_updater::{InstallKind, InstallOutcome, Release};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, AppContext as _, Entity, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::state::AppState;

/// How Cardfile was installed. Detecting it touches the file system (it creates a
/// probe file), so it runs once, on the background executor (see [`detect_kind`]).
static KIND: OnceLock<InstallKind> = OnceLock::new();

/// The install kind, detecting it on first use. Blocking: background executor only.
fn kind() -> &'static InstallKind {
    KIND.get_or_init(InstallKind::detect)
}

/// Detect the install kind in the background at startup, so views can ask
/// [`enabled`] without touching the file system.
pub fn detect_kind(cx: &App) {
    cx.background_spawn(async {
        kind();
    })
    .detach();
}

/// Whether updates are handled by Cardfile. Never blocks: `false` until the
/// install kind is known (see [`detect_kind`]).
pub fn enabled() -> bool {
    KIND.get().is_some_and(updates_by_us)
}

fn updates_by_us(kind: &InstallKind) -> bool {
    cfg!(feature = "update-check") && !managed_install(kind)
}

/// Installed by the Microsoft Store (MSIX) or a Linux package manager, which update
/// the app themselves.
fn managed_install(kind: &InstallKind) -> bool {
    matches!(
        kind,
        InstallKind::Store | InstallKind::PackageManager | InstallKind::Flatpak | InstallKind::Snap
    )
}

fn current() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| semver::Version::new(0, 0, 0))
}

/// Remove files left by a previous update (the old executable, temp downloads).
pub fn cleanup(cx: &App) {
    cx.background_spawn(async {
        let n = cardfile_updater::cleanup_after_update();
        if n > 0 {
            tracing::info!("removed {n} leftover update files");
        }
    })
    .detach();
}

/// Check in the background after sign-in if the user allows it.
pub fn check_on_startup(window: &mut Window, cx: &mut App) {
    if AppState::user_settings(cx).check_for_updates {
        run(false, window, cx);
    }
}

/// Check now and always report the result.
pub fn check_now(window: &mut Window, cx: &mut App) {
    run(true, window, cx);
}

/// Result of [`run`]'s background work.
enum Check {
    /// Store or package-manager install: it updates the app itself.
    Managed,
    Done(Result<Option<Release>, cardfile_updater::UpdateError>),
}

fn run(verbose: bool, window: &mut Window, cx: &mut App) {
    let task = cx.background_spawn(async move {
        if !updates_by_us(kind()) {
            return Check::Managed;
        }
        Check::Done(cardfile_updater::check(&current()))
    });
    window
        .spawn(cx, async move |cx| {
            let result = match task.await {
                Check::Managed => {
                    if verbose {
                        let _ = cx.update(|window, cx| {
                            window.push_notification(Notification::info(t("updates.managed")), cx)
                        });
                    }
                    return;
                }
                Check::Done(result) => result,
            };
            let _ = cx.update(|window, cx| match result {
                Ok(Some(release)) => {
                    let version = release.version.to_string();
                    let release = Arc::new(release);
                    window.push_notification(
                        Notification::info(t_args("updates.available", &[("version", &version)]))
                            .title(t("updates.title"))
                            .autohide(false)
                            .on_click(move |_, window, cx| {
                                show_dialog(release.clone(), window, cx)
                            }),
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

#[derive(Clone, PartialEq)]
enum Phase {
    Ready,
    Downloading { done: u64, total: Option<u64> },
    Installing,
    Failed(String),
}

struct UpdateState {
    phase: Phase,
    cancel: Arc<AtomicBool>,
}

/// The "new version" dialog: release notes, and Update / Later.
pub fn show_dialog(release: Arc<Release>, window: &mut Window, cx: &mut App) {
    // Known by now: the check that offered this release detected it.
    let can_install = KIND.get().is_some_and(InstallKind::is_self_updatable);
    let state = cx.new(|_| UpdateState {
        phase: Phase::Ready,
        cancel: Arc::new(AtomicBool::new(false)),
    });
    window.open_dialog(cx, move |dialog, _, cx| {
        let th = cx.theme();
        let phase = state.read(cx).phase.clone();
        let notes = release_notes_body(&release.notes);
        let mut body = v_flex().gap(px(10.)).w_full().min_w_0().child(
            div().text_color(th.muted_foreground).child(t_args(
                "updates.current",
                &[("version", env!("CARGO_PKG_VERSION"))],
            )),
        );
        if !notes.is_empty() {
            body = body.child(
                div()
                    .id("release-notes")
                    .w_full()
                    .min_w_0()
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(th.muted)
                    .text_size(px(13.))
                    .child(TextView::markdown("release-notes-md", notes)),
            );
        }
        match &phase {
            Phase::Downloading { done, total } => {
                let pct = total
                    .filter(|t| *t > 0)
                    .map(|t| (*done as f32 / t as f32 * 100.).min(100.))
                    .unwrap_or(0.);
                body = body
                    .child(Progress::new("update-progress").value(pct))
                    .child(div().text_size(px(12.)).child(t_args(
                        "updates.downloading",
                        &[("mb", &format!("{:.1}", *done as f64 / 1_048_576.))],
                    )));
            }
            Phase::Installing => {
                body = body.child(div().text_size(px(12.)).child(t("updates.installing")));
            }
            Phase::Failed(e) => {
                body = body.child(
                    div()
                        .text_size(px(12.))
                        .text_color(th.danger)
                        .child(t_args("updates.failed", &[("error", e)])),
                );
            }
            Phase::Ready if !can_install => {
                body = body.child(
                    div()
                        .text_size(px(12.))
                        .text_color(th.muted_foreground)
                        .child(t("updates.manual")),
                );
            }
            Phase::Ready => {}
        }
        let busy = matches!(phase, Phase::Downloading { .. } | Phase::Installing);
        let (st, st2, rel, rel2) = (
            state.clone(),
            state.clone(),
            release.clone(),
            release.clone(),
        );
        dialog
            // Fits the release notes; shrinks with small windows.
            .w(px(600.))
            .title(t_args(
                "updates.dialog_title",
                &[("version", &release.version.to_string())],
            ))
            .child(body)
            .footer(
                h_flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("update-notes")
                            .small()
                            .ghost()
                            .label(t("updates.open_page"))
                            .on_click(move |_, _, cx| cx.open_url(&rel2.url)),
                    )
                    .child(
                        Button::new("update-later")
                            .small()
                            .outline()
                            .label(if busy {
                                t("common.cancel")
                            } else {
                                t("updates.later")
                            })
                            .on_click(move |_, window, cx| {
                                st2.read(cx).cancel.store(true, Ordering::Relaxed);
                                window.close_dialog(cx);
                            }),
                    )
                    .when(can_install, |d| {
                        d.child(
                            Button::new("update-now")
                                .small()
                                .primary()
                                .loading(busy)
                                .label(t("updates.install"))
                                .on_click(move |_, window, cx| {
                                    if !busy {
                                        start(rel.clone(), st.clone(), window, cx);
                                    }
                                }),
                        )
                    }),
            )
    });
}

/// The release notes without their leading "Release Notes" heading, which only
/// repeats the dialog title.
fn release_notes_body(notes: &str) -> String {
    let notes = notes.trim();
    let first = notes.lines().next().unwrap_or_default();
    let heading = first.trim_start_matches('#').trim();
    if first.starts_with('#') && heading.eq_ignore_ascii_case("release notes") {
        notes[first.len()..].trim().to_owned()
    } else {
        notes.to_owned()
    }
}

fn start(release: Arc<Release>, state: Entity<UpdateState>, window: &mut Window, cx: &mut App) {
    let cancel = Arc::new(AtomicBool::new(false));
    state.update(cx, |s, cx| {
        s.phase = Phase::Downloading {
            done: 0,
            total: None,
        };
        s.cancel = cancel.clone();
        cx.notify();
    });
    // Progress comes from the download thread through a shared counter.
    let progress = Arc::new(parking_lot::Mutex::new((0u64, None::<u64>)));
    let progress_dl = progress.clone();
    let task = cx.background_spawn(async move {
        let downloaded = cardfile_updater::download(
            &release,
            kind(),
            |done, total| *progress_dl.lock() = (done, total),
            &cancel,
        )?;
        cardfile_updater::install(downloaded)
    });
    let poll_state = state.clone();
    let poll = window.spawn(cx, async move |cx| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            let (done, total) = *progress.lock();
            let keep = cx.update(|window, cx| {
                poll_state.update(cx, |s, cx| {
                    if let Phase::Downloading { .. } = s.phase {
                        s.phase = if total.is_some_and(|t| done >= t) {
                            Phase::Installing
                        } else {
                            Phase::Downloading { done, total }
                        };
                        cx.notify();
                    }
                    matches!(s.phase, Phase::Downloading { .. } | Phase::Installing)
                });
                window.refresh();
                true
            });
            let still = cx
                .update(|_, cx| {
                    matches!(
                        poll_state.read(cx).phase,
                        Phase::Downloading { .. } | Phase::Installing
                    )
                })
                .unwrap_or(false);
            if keep.is_err() || !still {
                break;
            }
        }
    });
    poll.detach();
    window
        .spawn(cx, async move |cx| {
            let result = task.await;
            // Save the open card before the new version starts (it would read the
            // database while the old one is still writing) or the installer runs.
            if matches!(
                result,
                Ok(InstallOutcome::Restart { .. } | InstallOutcome::QuitForInstaller)
            ) && let Ok(saved) = cx.update(|_, cx| crate::app::flush_all(cx))
            {
                saved.await;
            }
            let _ = cx.update(|window, cx| match result {
                Ok(InstallOutcome::Restart { exe }) => {
                    if let Err(e) = cardfile_updater::restart(&exe) {
                        tracing::error!("restart after update: {e}");
                        window.push_notification(
                            Notification::success(t("updates.installed_restart")),
                            cx,
                        );
                        state.update(cx, |s, cx| {
                            s.phase = Phase::Failed(e.to_string());
                            cx.notify();
                        });
                        return;
                    }
                    // Quit directly: a `Quit` action dispatched while the dialog has
                    // focus never reaches the app view, which left the old version open.
                    cx.quit();
                }
                Ok(InstallOutcome::QuitForInstaller) => cx.quit(),
                Err(cardfile_updater::UpdateError::Cancelled) => {
                    state.update(cx, |s, cx| {
                        s.phase = Phase::Ready;
                        cx.notify();
                    });
                }
                Err(e) => {
                    tracing::error!("update failed: {e}");
                    state.update(cx, |s, cx| {
                        s.phase = Phase::Failed(e.to_string());
                        cx.notify();
                    });
                }
            });
        })
        .detach();
}

#[allow(dead_code)]
fn _sizable(b: Button) -> Button {
    b.xsmall()
}

#[cfg(test)]
mod tests {
    use super::release_notes_body;

    #[test]
    fn drops_only_the_release_notes_heading() {
        assert_eq!(
            release_notes_body("## Release Notes\n\n### Bug Fixes\n- x\n"),
            "### Bug Fixes\n- x"
        );
        assert_eq!(release_notes_body("### Features\n- y"), "### Features\n- y");
    }
}
