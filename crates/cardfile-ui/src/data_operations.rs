//! Administrator-only collection operations, serialized with scheduled snapshots.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use cardfile_db::{BackupKind, Database};
use cardfile_i18n::{t, t_args};
use gpui_kit::component::{WindowExt as _, notification::Notification};
use gpui_kit::{App, AppContext as _, PathPromptOptions, Window};

use crate::local_config::{LocalConfig, LocalPreferences};
use crate::paths::DataPaths;
use crate::state::AppState;

static BUSY: AtomicBool = AtomicBool::new(false);
pub(crate) fn busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}

struct Operation;
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

fn administrator(cx: &App) -> bool {
    AppState::session(cx).is_some_and(|s| s.user.is_admin)
}

fn begin(window: &mut Window, cx: &mut App) -> Option<Operation> {
    if !administrator(cx) || crate::updater::busy() || BUSY.swap(true, Ordering::SeqCst) {
        return None;
    }
    window.close_all_dialogs(cx);
    window.blur(cx);
    cx.refresh_windows();
    Some(Operation)
}

fn flush(cx: &mut App) -> gpui_kit::Task<anyhow::Result<()>> {
    crate::workspace::Workspace::current(cx).map_or_else(
        || gpui_kit::Task::ready(Ok(())),
        |ws| ws.update(cx, |ws, cx| ws.flush_checked(cx)),
    )
}

fn finished(window: &mut Window, cx: &mut App, result: anyhow::Result<PathBuf>, automatic: bool) {
    let now = cardfile_core::now_millis();
    let success = result.is_ok();
    let message = match result {
        Ok(path) => t_args("backup.done", &[("path", &path.display().to_string())]),
        Err(e) => t_args("data_operations.failed", &[("error", &e.to_string())]),
    };
    LocalPreferences::change(cx, |s| {
        s.last_result = Some(message.clone());
        if automatic && success {
            s.last_automatic = Some(now);
        }
    });
    window.push_notification(
        if success {
            Notification::success(message)
        } else {
            Notification::error(message)
        },
        cx,
    );
    cx.refresh_windows();
}

pub(crate) fn backup(window: &mut Window, cx: &mut App, automatic: bool) {
    if !administrator(cx) {
        if !automatic {
            window.push_notification(Notification::error(t("data_operations.admin_required")), cx);
        }
        return;
    }
    let config = LocalPreferences::snapshot(cx);
    if automatic && !config.automatic_due(cardfile_core::now_millis()) {
        return;
    }
    let Some(operation) = begin(window, cx) else {
        return;
    };
    let saved = flush(cx);
    let db = AppState::db(cx);
    let dir = config.backup_dir(&AppState::get(cx).paths.root);
    window
        .spawn(cx, async move |cx| {
            let result = match saved.await {
                Err(e) => Err(e),
                Ok(()) => {
                    cx.background_spawn(async move {
                        db.backup_collection(
                            &dir,
                            &config.collection,
                            if automatic {
                                BackupKind::Automatic
                            } else {
                                BackupKind::Manual
                            },
                            config.keep_automatic,
                        )
                        .map_err(anyhow::Error::from)
                    })
                    .await
                }
            };
            drop(operation);
            let _ = cx.update(|window, cx| finished(window, cx, result, automatic));
        })
        .detach();
}

pub(crate) fn start_scheduler(window: &mut Window, cx: &mut App) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    backup(window, cx, true);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    window
        .spawn(cx, async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(3600))
                    .await;
                if cx.update(|window, cx| backup(window, cx, true)).is_err() {
                    break;
                }
            }
        })
        .detach();
}

pub(crate) fn choose_backup_folder(window: &mut Window, cx: &mut App) {
    if !administrator(cx) {
        return;
    }
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(t("data_operations.choose_backup").into()),
    });
    window
        .spawn(cx, async move |cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                let _ = cx.update(|_, cx| {
                    if !administrator(cx) {
                        return;
                    }
                    let root = &AppState::get(cx).paths.root;
                    let stored = path.strip_prefix(root).unwrap_or(path).to_owned();
                    LocalPreferences::change(cx, |s| s.backup_directory = stored);
                });
            }
        })
        .detach();
}

pub(crate) fn open_backup_folder(window: &mut Window, cx: &mut App) {
    if !administrator(cx) {
        return;
    }
    let path = LocalPreferences::snapshot(cx).backup_dir(&AppState::get(cx).paths.root);
    let task = cx.background_spawn(async move {
        std::fs::create_dir_all(&path)?;
        open::that_detached(path)
    });
    window
        .spawn(cx, async move |cx| {
            if let Err(e) = task.await {
                let _ = cx.update(|window, cx| {
                    window.push_notification(Notification::error(e.to_string()), cx)
                });
            }
        })
        .detach();
}

pub(crate) fn restore(window: &mut Window, cx: &mut App) {
    if !administrator(cx) || busy() {
        return;
    }
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some(t("data_operations.choose_restore").into()),
    });
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            let check_source = source.clone();
            let check = cx
                .background_spawn(async move { cardfile_db::validate_backup(&check_source) })
                .await;
            let _ = cx.update(|window, cx| {
                if !administrator(cx) {
                    return;
                }
                if let Err(e) = check {
                    window.push_notification(Notification::error(e.to_string()), cx);
                    return;
                }
                crate::dialogs::confirm(
                    t("data_operations.restore"),
                    t("data_operations.restore_confirm"),
                    t("data_operations.restore"),
                    window,
                    cx,
                    move |window, cx| restore_confirmed(source.clone(), window, cx),
                );
            });
        })
        .detach();
}

fn restore_confirmed(source: PathBuf, window: &mut Window, cx: &mut App) {
    let Some(operation) = begin(window, cx) else {
        return;
    };
    let saved = flush(cx);
    let db = AppState::db(cx);
    let safety = AppState::get(cx).paths.backups();
    // Stopping joins active MCP calls before the safety snapshot is taken.
    let stopped = crate::mcp_host::McpServer::stop(cx);
    window
        .spawn(cx, async move |cx| {
            stopped.await;
            let result = match saved.await {
                Err(e) => Err(e),
                Ok(()) => {
                    cx.background_spawn(async move {
                        db.prepare_restore(&source, &safety)
                            .map_err(anyhow::Error::from)
                    })
                    .await
                }
            };
            let _ = cx.update(|window, cx| {
                match result {
                    Ok(()) => {
                        // Replacement is performed only by the next process under its lock.
                        let exe = std::env::var_os("APPIMAGE")
                            .map(PathBuf::from)
                            .or_else(|| std::env::current_exe().ok());
                        if let Some(exe) = exe
                            && let Err(e) = cardfile_updater::restart(&exe)
                        {
                            tracing::error!("restart for restore: {e}");
                        }
                        cx.quit();
                    }
                    Err(e) => {
                        drop(operation);
                        crate::mcp_host::McpServer::sync(cx);
                        finished(window, cx, Err(e), false);
                    }
                }
            });
        })
        .detach();
}

fn copy_tree(source: &Path, destination: &Path) -> std::io::Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if source.symlink_metadata()?.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "symbolic links cannot be included in a portable copy",
        ));
    }
    if source.is_dir() {
        std::fs::create_dir_all(destination)?;
        for item in std::fs::read_dir(source)? {
            let item = item?;
            copy_tree(&item.path(), &destination.join(item.file_name()))?;
        }
    } else {
        std::fs::copy(source, destination)?;
    }
    Ok(())
}

pub(crate) fn portable_copy(
    db: &Database,
    paths: &DataPaths,
    executable: &Path,
    destination: &Path,
    mut config: LocalConfig,
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !destination.exists(),
        "{}",
        t("data_operations.destination_exists")
    );
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!(t("data_operations.invalid_destination")))?;
    let canonical_parent = parent.canonicalize()?;
    let data_root = paths.root.canonicalize()?;
    anyhow::ensure!(
        !canonical_parent.starts_with(&data_root),
        "{}",
        t("data_operations.nested_destination")
    );
    if let Some(resources) = executable.parent() {
        let dictionaries = resources.join("dictionaries");
        if dictionaries.exists() {
            anyhow::ensure!(
                !canonical_parent.starts_with(dictionaries.canonicalize()?),
                "{}",
                t("data_operations.nested_destination")
            );
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(".cardfile-portable-")
        .tempdir_in(parent)?;
    let root = staging.path();
    let is_appimage = std::env::var_os("APPIMAGE").is_some();
    let file_name = executable
        .file_name()
        .ok_or_else(|| anyhow::anyhow!(t("data_operations.invalid_destination")))?;
    std::fs::copy(executable, root.join(file_name))?;
    if !is_appimage {
        let resources = executable
            .parent()
            .ok_or_else(|| anyhow::anyhow!(t("data_operations.invalid_destination")))?;
        for name in [
            "dictionaries",
            "THIRD_PARTY_LICENSES.html",
            "LICENSE-MIT",
            "LICENSE-APACHE",
        ] {
            copy_tree(&resources.join(name), &root.join(name))?;
        }
    }
    std::fs::write(root.join("portable.flag"), b"Cardfile portable\n")?;
    let data = root.join("data");
    let snapshot = db.backup(&data)?;
    std::fs::rename(snapshot, data.join("cardfile.db"))?;
    {
        let copied = Database::open(&data.join("cardfile.db"), None)?;
        let mut app = copied.app_settings()?;
        app.mcp_enabled = false;
        copied.save_app_settings(&app)?;
    }
    for name in ["themes", "keymap.json", "dictionaries"] {
        copy_tree(&paths.root.join(name), &data.join(name))?;
    }
    config.collection = uuid::Uuid::now_v7().to_string();
    config.daily_backup = false;
    config.backup_directory = "backups".into();
    config.last_automatic = None;
    config.last_result = None;
    config.save(&data)?;
    // The destination must be new. Recheck immediately before publication.
    anyhow::ensure!(
        !destination.exists(),
        "{}",
        t("data_operations.destination_exists")
    );
    std::fs::rename(root, destination)?;
    Ok(destination.to_owned())
}

pub(crate) fn create_portable(window: &mut Window, cx: &mut App) {
    if !administrator(cx) || busy() {
        return;
    }
    let executable = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    let Some(executable) = executable else {
        return;
    };
    let task = cx.background_spawn(async move {
        let kind = cardfile_updater::InstallKind::detect();
        let supported = matches!(
            kind,
            cardfile_updater::InstallKind::Portable { .. }
                | cardfile_updater::InstallKind::Standalone { .. }
                | cardfile_updater::InstallKind::InnoSetup { .. }
                | cardfile_updater::InstallKind::AppImage { .. }
        );
        (supported, executable)
    });
    window
        .spawn(cx, async move |cx| {
            let (supported, executable) = task.await;
            let _ = cx.update(|window, cx| {
                if !administrator(cx) {
                    return;
                }
                if !supported {
                    window.push_notification(
                        Notification::error(t("data_operations.portable_unsupported")),
                        cx,
                    );
                    return;
                }
                crate::dialogs::confirm(
                    t("data_operations.portable"),
                    t("data_operations.portable_confirm"),
                    t("common.ok"),
                    window,
                    cx,
                    move |window, cx| choose_portable_destination(executable.clone(), window, cx),
                );
            });
        })
        .detach();
}

fn choose_portable_destination(executable: PathBuf, window: &mut Window, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(t("data_operations.choose_portable").into()),
    });
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(base) = paths.into_iter().next() else {
                return;
            };
            let _ = cx.update(|window, cx| {
                let Some(operation) = begin(window, cx) else {
                    return;
                };
                let saved = flush(cx);
                let db = AppState::db(cx);
                let paths = AppState::get(cx).paths.clone();
                let config = LocalPreferences::snapshot(cx);
                window
                    .spawn(cx, async move |cx| {
                        let result = match saved.await {
                            Err(e) => Err(e),
                            Ok(()) => {
                                cx.background_spawn(async move {
                                    portable_copy(
                                        &db,
                                        &paths,
                                        &executable,
                                        &base.join("Cardfile portable"),
                                        config,
                                    )
                                })
                                .await
                            }
                        };
                        drop(operation);
                        let _ = cx.update(|window, cx| finished(window, cx, result, false));
                    })
                    .detach();
            });
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_copy_preserves_collection_and_disables_services_without_touching_source() {
        let temp = tempfile::tempdir().unwrap();
        let program = temp.path().join("program");
        std::fs::create_dir(&program).unwrap();
        let executable = program.join("cardfile.exe");
        std::fs::write(&executable, b"test executable, never launched").unwrap();
        let paths = DataPaths {
            root: temp.path().join("original-data"),
            portable: false,
        };
        let db = Database::open(&paths.db(), None).unwrap();
        db.create_user("owner", "Owner", Some("password hash"), true)
            .unwrap();
        let mut app = db.app_settings().unwrap();
        app.mcp_enabled = true;
        db.save_app_settings(&app).unwrap();
        std::fs::write(paths.keymap(), b"custom shortcuts").unwrap();
        std::fs::create_dir_all(paths.backups()).unwrap();
        std::fs::write(paths.backups().join("keep.db"), b"existing backup").unwrap();
        let config = LocalConfig {
            daily_backup: true,
            ..Default::default()
        };
        let old_id = config.collection.clone();
        let destination = temp.path().join("Cardfile portable");
        portable_copy(&db, &paths, &executable, &destination, config).unwrap();
        let copied = Database::open(&destination.join("data/cardfile.db"), None).unwrap();
        assert_eq!(
            copied.list_users().unwrap()[0].password_hash.as_deref(),
            Some("password hash")
        );
        assert!(!copied.app_settings().unwrap().mcp_enabled);
        assert!(db.app_settings().unwrap().mcp_enabled);
        let local = LocalConfig::load(&destination.join("data"));
        assert!(!local.daily_backup);
        assert_ne!(local.collection, old_id);
        assert_eq!(
            std::fs::read(destination.join("data/keymap.json")).unwrap(),
            b"custom shortcuts"
        );
        assert!(!destination.join("data/backups").exists());
        assert!(paths.backups().join("keep.db").exists());
        assert!(destination.join("portable.flag").exists());
        assert!(!destination.join("unins000.exe").exists());
    }

    #[test]
    fn portable_copy_rejects_existing_and_nested_destinations() {
        let temp = tempfile::tempdir().unwrap();
        let paths = DataPaths {
            root: temp.path().join("data"),
            portable: true,
        };
        let db = Database::open(&paths.db(), None).unwrap();
        let destination = temp.path().join("existing");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("keep.txt"), b"do not replace").unwrap();
        let missing_exe = temp.path().join("missing.exe");
        assert!(
            portable_copy(
                &db,
                &paths,
                &missing_exe,
                &destination,
                LocalConfig::default()
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(destination.join("keep.txt")).unwrap(),
            b"do not replace"
        );
        assert!(
            portable_copy(
                &db,
                &paths,
                &missing_exe,
                &paths.root.join("nested"),
                LocalConfig::default()
            )
            .is_err()
        );
        assert!(!paths.root.join("nested").exists());
    }
}
