//! Installing a verified update, restarting, and cleaning up afterwards.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::archive::{self, ArchiveFormat};
use crate::kind::TEMP_PREFIX;
use crate::{Downloaded, InstallKind, UpdateError};

/// What the app must do after [`install`] succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    /// The new version is in place. Call [`restart`] with `exe`, then quit.
    /// (Quitting without restarting is fine too: the next launch runs the new version.)
    Restart {
        /// The program to launch: the replaced executable or AppImage.
        exe: PathBuf,
    },
    /// The Inno Setup installer is running. Quit now, without restarting: the installer
    /// waits for (and if needed closes) Cardfile, replaces it and launches it again.
    QuitForInstaller,
}

/// Folder that holds a downloaded installer until it has run.
pub(crate) fn installer_dir() -> PathBuf {
    std::env::temp_dir().join("cardfile-update")
}

/// Install a verified download.
///
/// - Standalone and portable copies: the archive is unpacked next to the executable and
///   every file it contains replaces its installed copy (files that are not in the
///   archive are left alone). On Windows the running executable is renamed to
///   `cardfile.exe.old` first ([`cleanup_after_update`] deletes it on the next start);
///   on Linux the new executable is renamed over the old one atomically.
/// - AppImage: the `.AppImage` file is replaced atomically.
/// - Inno Setup: the installer is started with `/VERYSILENT /SUPPRESSMSGBOXES
///   /NORESTART /CLOSEAPPLICATIONS /RELAUNCH` (and `/CURRENTUSER` or `/ALLUSERS`, which
///   may show a UAC prompt for a per-machine install).
///
/// Blocking: call off the UI thread.
pub fn install(downloaded: Downloaded) -> Result<InstallOutcome, UpdateError> {
    let Downloaded { kind, asset, path } = downloaded;
    match kind {
        InstallKind::Standalone { exe } | InstallKind::Portable { exe } => {
            let format = ArchiveFormat::from_name(&asset.name)
                .ok_or_else(|| UpdateError::Archive(format!("unknown format: {}", asset.name)))?;
            install_archive(&path, format, &exe)?;
            Ok(InstallOutcome::Restart { exe })
        }
        InstallKind::AppImage { appimage } => {
            make_executable(&path)?;
            path.persist(&appimage)
                .map_err(|e| UpdateError::Io(e.error))?;
            Ok(InstallOutcome::Restart { exe: appimage })
        }
        InstallKind::InnoSetup { per_user, .. } => {
            // The installer must outlive this process: keep the file. The next start's
            // `cleanup_after_update` deletes it.
            let setup = path.keep().map_err(|e| UpdateError::Io(e.error))?;
            Command::new(&setup)
                .args(installer_args(per_user))
                .spawn()?;
            Ok(InstallOutcome::QuitForInstaller)
        }
        other => Err(UpdateError::Unsupported(format!(
            "{other:?} is updated by the platform"
        ))),
    }
}

/// Command line for a silent Inno Setup update. `/RELAUNCH` is Cardfile's own switch:
/// `installer.iss` starts the app again when it is present.
pub(crate) fn installer_args(per_user: bool) -> [&'static str; 6] {
    [
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/CLOSEAPPLICATIONS",
        "/RELAUNCH",
        if per_user {
            "/CURRENTUSER"
        } else {
            "/ALLUSERS"
        },
    ]
}

/// Unpack `archive` next to `exe` and move its files into place, the executable last.
///
/// All or nothing: every installed file that is replaced is first moved into a backup
/// folder, and if any step fails (including the executable swap) the new files are
/// removed and the originals put back, so the installation is never left half-updated.
pub(crate) fn install_archive(
    archive: &Path,
    format: ArchiveFormat,
    exe: &Path,
) -> Result<(), UpdateError> {
    let (Some(dir), Some(exe_name)) = (exe.parent(), exe.file_name()) else {
        return Err(UpdateError::Unsupported(format!(
            "unexpected executable path {}",
            exe.display()
        )));
    };
    // Staging inside the install folder keeps every rename on one file system.
    let staging = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempdir_in(dir)?;
    archive::extract(archive, format, staging.path())?;
    let root = archive::find_parent_of(staging.path(), exe_name).ok_or_else(|| {
        UpdateError::Archive(format!(
            "{} is not in the archive",
            exe_name.to_string_lossy()
        ))
    })?;
    let mut files = Vec::new();
    for rel in archive::list_files(&root)? {
        if rel.as_os_str() == exe_name {
            continue;
        }
        let dest = dir.join(&rel);
        // Only refresh what this installation already has: a bare binary installed
        // into a shared folder (e.g. ~/.cargo/bin) must not gain README or LICENSE
        // files. Inside folders the installation ships (dictionaries/), new files
        // are welcome.
        let in_shipped_dir = dest.parent().is_some_and(|p| p != dir && p.is_dir());
        if !dest.exists() && !in_shipped_dir {
            continue;
        }
        files.push(rel);
    }
    let backup = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempdir_in(dir)?;
    let new_exe = root.join(exe_name);
    replace_files(&root, dir, backup.path(), &files, || {
        replace_executable(&new_exe, exe)
    })?;
    Ok(())
}

/// What [`replace_files`] did to one file, so it can be undone.
enum Step {
    /// The new file was moved in where there was none.
    Added(PathBuf),
    /// The installed file was moved into the backup folder (and maybe replaced).
    BackedUp(PathBuf),
}

/// Move each `files` entry (relative paths) from `src` into `dir`, moving the files it
/// replaces into `backup` first, then run `finish`. If anything fails, every change is
/// undone (best effort) and the first error is returned.
fn replace_files(
    src: &Path,
    dir: &Path,
    backup: &Path,
    files: &[PathBuf],
    finish: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let mut done = Vec::with_capacity(files.len());
    let result = (|| {
        for rel in files {
            let dest = dir.join(rel);
            if dest.symlink_metadata().is_ok() {
                let saved = backup.join(rel);
                if let Some(parent) = saved.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&dest, &saved)?;
                done.push(Step::BackedUp(rel.clone()));
            } else {
                done.push(Step::Added(rel.clone()));
            }
            if let Err(e) = fs::rename(src.join(rel), &dest) {
                // Nothing was moved in for this entry: only the backup (if any) to undo.
                if let Some(Step::Added(_)) = done.last() {
                    done.pop();
                }
                return Err(e);
            }
        }
        finish()
    })();
    if result.is_err() {
        for step in done.iter().rev() {
            match step {
                Step::Added(rel) => {
                    let _ = fs::remove_file(dir.join(rel));
                }
                Step::BackedUp(rel) => {
                    let dest = dir.join(rel);
                    let saved = backup.join(rel);
                    if saved.symlink_metadata().is_ok() {
                        let _ = fs::remove_file(&dest);
                        let _ = fs::rename(&saved, &dest);
                    }
                }
            }
        }
    }
    result
}

/// Give `path` the executable bits (no-op on Windows).
fn make_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Put `new` in place of the (possibly running) executable `exe`.
pub(crate) fn replace_executable(new: &Path, exe: &Path) -> io::Result<()> {
    make_executable(new)?;
    if cfg!(windows) {
        // A running .exe cannot be overwritten or deleted, but it can be renamed.
        replace_by_renaming_aside(new, exe).map(|_| ())
    } else {
        // The running process keeps the old inode; the rename is atomic.
        fs::rename(new, exe)
    }
}

/// `<exe>.old`, or `<exe>.old.N` when an older leftover cannot be deleted (e.g. it is
/// still running).
fn free_old_path(exe: &Path) -> PathBuf {
    let base = {
        let mut name = exe.as_os_str().to_os_string();
        name.push(".old");
        PathBuf::from(name)
    };
    if !base.exists() || fs::remove_file(&base).is_ok() {
        return base;
    }
    let mut n = 1u32;
    loop {
        let mut name = base.as_os_str().to_os_string();
        name.push(format!(".{n}"));
        let candidate = PathBuf::from(name);
        if !candidate.exists() || fs::remove_file(&candidate).is_ok() {
            return candidate;
        }
        n += 1;
    }
}

/// Rename `exe` to `<exe>.old` and move `new` into its place, undoing the first rename
/// if the second fails. Returns where the old executable went.
pub(crate) fn replace_by_renaming_aside(new: &Path, exe: &Path) -> io::Result<PathBuf> {
    let old = free_old_path(exe);
    let had_exe = exe.exists();
    if had_exe {
        fs::rename(exe, &old)?;
    }
    if let Err(e) = fs::rename(new, exe) {
        if had_exe {
            let _ = fs::rename(&old, exe);
        }
        return Err(e);
    }
    Ok(old)
}

/// Launch `exe` (from [`InstallOutcome::Restart`]) with this process's arguments.
/// Returns once the new process is started; the caller then quits.
pub fn restart(exe: &Path) -> Result<(), UpdateError> {
    Command::new(exe)
        .args(std::env::args_os().skip(1))
        .spawn()?;
    Ok(())
}

/// Delete what a previous update left behind: the renamed `cardfile.exe.old`,
/// interrupted downloads and staging folders next to the executable (or AppImage),
/// and a downloaded installer. Best effort; call once at startup, ideally on a
/// background thread. Returns how many entries were removed.
pub fn cleanup_after_update() -> usize {
    let mut removed = 0;
    if let Ok(exe) = std::env::current_exe() {
        removed += cleanup_next_to(&exe);
    }
    if let Some(appimage) = std::env::var_os("APPIMAGE").filter(|v| !v.is_empty()) {
        removed += cleanup_next_to(Path::new(&appimage));
    }
    let dir = installer_dir();
    if dir.exists() && fs::remove_dir_all(&dir).is_ok() {
        removed += 1;
    }
    removed
}

fn cleanup_next_to(file: &Path) -> usize {
    match (file.parent(), file.file_name()) {
        (Some(dir), Some(name)) => cleanup_dir(dir, name),
        _ => 0,
    }
}

/// Remove `<exe_name>.old*` files and updater temporaries from `dir`.
pub(crate) fn cleanup_dir(dir: &Path, exe_name: &OsStr) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let old_prefix = format!("{}.old", exe_name.to_string_lossy());
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();
        let ok = if name.starts_with(&old_prefix) {
            fs::remove_file(&path).is_ok()
        } else if name.starts_with(TEMP_PREFIX) {
            match entry.file_type() {
                Ok(t) if t.is_dir() => fs::remove_dir_all(&path).is_ok(),
                Ok(_) => fs::remove_file(&path).is_ok(),
                Err(_) => false,
            }
        } else {
            false
        };
        removed += usize::from(ok);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::{tar_xz_of, zip_of};

    #[test]
    fn rename_dance() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("cardfile.exe");
        let new = dir.path().join("staged.exe");
        fs::write(&exe, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        // A stale leftover from an earlier update is replaced.
        fs::write(dir.path().join("cardfile.exe.old"), b"older").unwrap();

        let old = replace_by_renaming_aside(&new, &exe).unwrap();
        assert_eq!(old, dir.path().join("cardfile.exe.old"));
        assert_eq!(fs::read(&exe).unwrap(), b"new");
        assert_eq!(fs::read(&old).unwrap(), b"old");
        assert!(!new.exists());

        // A failed move puts the original back.
        let missing = dir.path().join("missing.exe");
        assert!(replace_by_renaming_aside(&missing, &exe).is_err());
        assert_eq!(fs::read(&exe).unwrap(), b"new");

        // Startup cleanup removes the leftovers but nothing else.
        fs::create_dir(dir.path().join(format!("{TEMP_PREFIX}abc"))).unwrap();
        fs::write(dir.path().join("notes.txt"), b"keep").unwrap();
        fs::write(dir.path().join("cardfile.exe.old.1"), b"older").unwrap();
        let removed = cleanup_dir(dir.path(), OsStr::new("cardfile.exe"));
        assert_eq!(removed, 2);
        let mut left: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["cardfile.exe", "notes.txt"]);
    }

    #[cfg(windows)]
    #[test]
    fn rename_dance_while_open() {
        // An open handle stands in for the running image: overwriting fails, renaming works.
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("cardfile.exe");
        let new = dir.path().join("staged.exe");
        fs::write(&exe, b"old").unwrap();
        fs::write(&new, b"new").unwrap();
        let _handle = fs::File::open(&exe).unwrap();
        replace_executable(&new, &exe).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"new");
    }

    fn installs(archive: Vec<u8>, name: &str, exe_name: &str) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(exe_name);
        fs::write(&exe, b"old binary").unwrap();
        fs::write(dir.path().join("THIRD_PARTY_LICENSES.html"), b"old").unwrap();
        fs::write(dir.path().join("cards.db"), b"user data").unwrap();
        // The installation already ships a dictionaries folder; new files there are added.
        fs::create_dir(dir.path().join("dictionaries")).unwrap();
        let file = dir.path().join(format!("{TEMP_PREFIX}{name}"));
        fs::write(&file, archive).unwrap();

        install_archive(&file, ArchiveFormat::from_name(name).unwrap(), &exe).unwrap();
        fs::remove_file(&file).unwrap();

        assert_eq!(fs::read(&exe).unwrap(), b"new binary");
        assert_eq!(
            fs::read(dir.path().join("THIRD_PARTY_LICENSES.html")).unwrap(),
            b"<p>"
        );
        assert_eq!(
            fs::read(dir.path().join("dictionaries").join("en_US.dic")).unwrap(),
            b"words"
        );
        // A file the installation never had is not added next to the binary.
        assert!(!dir.path().join("README.md").exists());
        // Files that are not in the archive are untouched.
        assert_eq!(fs::read(dir.path().join("cards.db")).unwrap(), b"user data");
        // The staging folder is gone.
        let staging_left = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with(TEMP_PREFIX));
        assert!(!staging_left);
        cleanup_dir(dir.path(), OsStr::new(exe_name));
        assert!(!dir.path().join(format!("{exe_name}.old")).exists());
    }

    #[test]
    fn installs_zip_with_top_level_folder() {
        let files: &[(&str, &[u8])] = &[
            (
                "cardfile-x86_64-pc-windows-msvc/cardfile.exe",
                b"new binary",
            ),
            (
                "cardfile-x86_64-pc-windows-msvc/THIRD_PARTY_LICENSES.html",
                b"<p>",
            ),
            (
                "cardfile-x86_64-pc-windows-msvc/dictionaries/en_US.dic",
                b"words",
            ),
        ];
        installs(
            zip_of(files),
            "cardfile-x86_64-pc-windows-msvc.zip",
            "cardfile.exe",
        );
    }

    #[test]
    fn installs_flat_tar_xz() {
        let files: &[(&str, &[u8])] = &[
            ("cardfile", b"new binary"),
            ("THIRD_PARTY_LICENSES.html", b"<p>"),
            ("dictionaries/en_US.dic", b"words"),
            ("README.md", b"not installed"),
        ];
        installs(
            tar_xz_of(files),
            "cardfile-x86_64-unknown-linux-gnu.tar.xz",
            "cardfile",
        );
    }

    #[test]
    fn archive_without_executable_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("cardfile");
        fs::write(&exe, b"old").unwrap();
        let file = dir.path().join("a.zip");
        fs::write(&file, zip_of(&[("README.md", b"hi")])).unwrap();
        let err = install_archive(&file, ArchiveFormat::Zip, &exe).unwrap_err();
        assert!(matches!(err, UpdateError::Archive(_)));
        assert_eq!(fs::read(&exe).unwrap(), b"old");
        assert!(!dir.path().join("README.md").exists());
    }

    /// A tree of `files` (relative path, contents) under `dir`.
    fn tree(dir: &Path, files: &[(&str, &[u8])]) {
        for (rel, data) in files {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, data).unwrap();
        }
    }

    fn read(dir: &Path, rel: &str) -> Option<Vec<u8>> {
        fs::read(dir.join(rel)).ok()
    }

    #[test]
    fn failed_executable_swap_restores_every_file() {
        let base = tempfile::tempdir().unwrap();
        let (src, dir, backup) = (
            base.path().join("src"),
            base.path().join("inst"),
            base.path().join("bak"),
        );
        tree(
            &src,
            &[
                ("LICENSE.html", b"new"),
                ("dictionaries/en.dic", b"new dic"),
                ("dictionaries/fr.dic", b"added"),
            ],
        );
        tree(
            &dir,
            &[
                ("LICENSE.html", b"old"),
                ("dictionaries/en.dic", b"old dic"),
                ("cards.db", b"data"),
            ],
        );
        fs::create_dir(&backup).unwrap();
        let files: Vec<PathBuf> = ["LICENSE.html", "dictionaries/en.dic", "dictionaries/fr.dic"]
            .iter()
            .map(PathBuf::from)
            .collect();

        let err = replace_files(&src, &dir, &backup, &files, || {
            // The new files are in place when the executable is swapped...
            assert_eq!(read(&dir, "LICENSE.html").unwrap(), b"new");
            Err(io::Error::other("executable is locked"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "executable is locked");
        // ...and all of them are rolled back when that fails.
        assert_eq!(read(&dir, "LICENSE.html").unwrap(), b"old");
        assert_eq!(read(&dir, "dictionaries/en.dic").unwrap(), b"old dic");
        assert_eq!(read(&dir, "dictionaries/fr.dic"), None);
        assert_eq!(read(&dir, "cards.db").unwrap(), b"data");
    }

    #[test]
    fn failure_midway_restores_the_files_already_moved() {
        let base = tempfile::tempdir().unwrap();
        let (src, dir, backup) = (
            base.path().join("src"),
            base.path().join("inst"),
            base.path().join("bak"),
        );
        tree(&src, &[("a.txt", b"new a")]);
        tree(&dir, &[("a.txt", b"old a"), ("b.txt", b"old b")]);
        fs::create_dir(&backup).unwrap();
        // b.txt is missing from the staged files: its move fails after a.txt is done.
        let files = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        let mut finished = false;
        assert!(
            replace_files(&src, &dir, &backup, &files, || {
                finished = true;
                Ok(())
            })
            .is_err()
        );
        assert!(!finished);
        assert_eq!(read(&dir, "a.txt").unwrap(), b"old a");
        assert_eq!(read(&dir, "b.txt").unwrap(), b"old b");
    }

    #[test]
    fn installer_command_line() {
        let args = installer_args(true);
        assert!(args.contains(&"/VERYSILENT") && args.contains(&"/RELAUNCH"));
        assert!(args.contains(&"/CURRENTUSER"));
        assert!(installer_args(false).contains(&"/ALLUSERS"));
    }
}
