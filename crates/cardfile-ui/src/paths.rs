//! Where Cardfile keeps its data.
//!
//! - Standard mode: a `FittyAr/Cardfile` folder, shared layout of every FittyAr
//!   app: `%APPDATA%\FittyAr\Cardfile\data` on Windows,
//!   `$XDG_DATA_HOME/FittyAr/Cardfile` (`~/.local/share` when unset) on Linux and
//!   `~/Library/Application Support/FittyAr/Cardfile` on macOS. The folder of the
//!   earlier layout (`$XDG_DATA_HOME/cardfile`, `ar.FittyAr.Cardfile` on macOS)
//!   is renamed into place on the first start: never deleted or merged, and kept
//!   in use when it cannot be moved.
//! - Portable mode: a `data` folder next to the executable, enabled by a
//!   `portable.flag` file there, the `--portable` flag, or a `data/cardfile.db`
//!   already next to the executable (a copied or synced folder, e.g. in OneDrive,
//!   keeps working without the flag).
//!
//! Every setting lives in `cardfile.db` (installation settings in `app_settings`,
//! preferences on each user row) next to `keymap.json`, so the folder is the whole
//! state of Cardfile.

use std::path::{Path, PathBuf};

pub const PORTABLE_FLAG: &str = "portable.flag";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataPaths {
    pub root: PathBuf,
    pub portable: bool,
}

impl DataPaths {
    pub fn db(&self) -> PathBuf {
        self.root.join("cardfile.db")
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn themes(&self) -> PathBuf {
        self.root.join("themes")
    }
    pub fn keymap(&self) -> PathBuf {
        self.root.join("keymap.json")
    }
    pub fn dictionaries(&self) -> PathBuf {
        self.root.join("dictionaries")
    }

    /// Resolve the data location. `override_dir` (from `--data-dir`) wins.
    pub fn resolve(override_dir: Option<PathBuf>, force_portable: bool) -> Self {
        if let Some(root) = override_dir {
            return Self {
                root,
                portable: false,
            };
        }
        let exe_dir = exe_dir();
        let portable = force_portable || exe_dir.as_deref().is_some_and(is_portable_dir);
        if portable && let Some(dir) = exe_dir {
            return Self::portable_at(&dir);
        }
        Self::standard()
    }

    pub fn standard() -> Self {
        Self {
            root: standard_root().unwrap_or_else(|| PathBuf::from(".cardfile")),
            portable: false,
        }
    }

    pub fn portable_at(exe_dir: &Path) -> Self {
        Self {
            root: exe_dir.join("data"),
            portable: true,
        }
    }

    /// Switch to portable mode: write the flag next to the executable.
    pub fn make_portable() -> std::io::Result<Self> {
        let dir = exe_dir().ok_or_else(|| std::io::Error::other("cannot locate the executable"))?;
        std::fs::write(
            dir.join(PORTABLE_FLAG),
            b"Cardfile portable mode: data is stored in ./data\n",
        )?;
        Ok(Self::portable_at(&dir))
    }

    /// Whether the executable's folder is writable (portable mode needs it).
    pub fn portable_available() -> bool {
        let Some(dir) = exe_dir() else { return false };
        let probe = dir.join(".cardfile-write-test");
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    }
}

/// The standard data folder of this platform (see the module docs).
#[cfg(target_os = "windows")]
fn standard_root() -> Option<PathBuf> {
    // Already `%APPDATA%\FittyAr\Cardfile\data`.
    directories::ProjectDirs::from("ar", "FittyAr", "Cardfile").map(|d| d.data_dir().to_owned())
}

/// The standard data folder of this platform (see the module docs).
#[cfg(not(target_os = "windows"))]
fn standard_root() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    let (new, previous) = unix_candidates(dirs.data_dir(), cfg!(target_os = "macos"));
    Some(adopt_previous(&new, &previous))
}

/// `(folder, folder of the previous layout)` under the platform data folder
/// `base` (`$XDG_DATA_HOME`, or `~/Library/Application Support` on macOS).
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn unix_candidates(base: &Path, macos: bool) -> (PathBuf, PathBuf) {
    let previous = if macos {
        "ar.FittyAr.Cardfile"
    } else {
        "cardfile"
    };
    (base.join("FittyAr").join("Cardfile"), base.join(previous))
}

/// The folder to use: `new`, after renaming `previous` (the folder of the
/// layout before the shared FittyAr one) into place when `new` does not exist
/// yet. Nothing is deleted or merged; if `previous` cannot be moved (another
/// file system, permissions) it keeps being used.
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn adopt_previous(new: &Path, previous: &Path) -> PathBuf {
    if new.exists() || !previous.is_dir() {
        return new.to_path_buf();
    }
    let moved = new
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::rename(previous, new));
    match moved {
        Ok(()) => {
            tracing::info!("moved {} to {}", previous.display(), new.display());
            new.to_path_buf()
        }
        Err(e) => {
            tracing::warn!(
                "could not move {} to {} ({e}); it keeps being used",
                previous.display(),
                new.display()
            );
            previous.to_path_buf()
        }
    }
}

/// Whether the executable folder `dir` holds a portable copy: the flag, or the
/// database of an earlier portable run in `data/`.
fn is_portable_dir(dir: &Path) -> bool {
    dir.join(PORTABLE_FLAG).exists() || dir.join("data").join("cardfile.db").is_file()
}

fn exe_dir() -> Option<PathBuf> {
    // Inside an AppImage the executable lives in a temporary read-only mount; the
    // portable data belongs next to the `.AppImage` file itself.
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Path::new(&appimage).parent().map(Path::to_owned);
    }
    std::env::current_exe().ok()?.parent().map(Path::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let p = DataPaths::resolve(Some(PathBuf::from("x")), true);
        assert_eq!(p.root, PathBuf::from("x"));
        assert_eq!(p.db(), PathBuf::from("x").join("cardfile.db"));
    }

    #[test]
    fn a_data_folder_with_the_database_means_portable() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_portable_dir(tmp.path()));
        // An empty `data` folder is not enough: it may belong to something else.
        std::fs::create_dir(tmp.path().join("data")).unwrap();
        assert!(!is_portable_dir(tmp.path()));
        std::fs::write(tmp.path().join("data").join("cardfile.db"), b"").unwrap();
        assert!(is_portable_dir(tmp.path()));
        let flag_only = tempfile::tempdir().unwrap();
        std::fs::write(flag_only.path().join(PORTABLE_FLAG), b"").unwrap();
        assert!(is_portable_dir(flag_only.path()));
    }

    #[test]
    fn linux_and_macos_use_the_fittyar_folder() {
        let base = Path::new("/home/u/.local/share");
        let (new, previous) = unix_candidates(base, false);
        assert_eq!(new, base.join("FittyAr").join("Cardfile"));
        assert_eq!(previous, base.join("cardfile"));
        let base = Path::new("/Users/u/Library/Application Support");
        let (new, previous) = unix_candidates(base, true);
        assert_eq!(new, base.join("FittyAr").join("Cardfile"));
        assert_eq!(previous, base.join("ar.FittyAr.Cardfile"));
    }

    #[test]
    fn the_previous_folder_is_moved_into_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        std::fs::create_dir_all(&previous).unwrap();
        std::fs::write(previous.join("cardfile.db"), b"cards").unwrap();
        assert_eq!(adopt_previous(&new, &previous), new);
        assert_eq!(std::fs::read(new.join("cardfile.db")).unwrap(), b"cards");
        assert!(!previous.exists());
    }

    #[test]
    fn an_existing_new_folder_is_never_merged() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&previous).unwrap();
        std::fs::write(previous.join("cardfile.db"), b"old").unwrap();
        assert_eq!(adopt_previous(&new, &previous), new);
        assert!(!new.join("cardfile.db").exists());
        assert!(previous.join("cardfile.db").exists(), "old data kept");
    }

    #[test]
    fn without_a_previous_folder_the_new_one_is_used() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        assert_eq!(adopt_previous(&new, &previous), new);
        assert!(!new.exists(), "created later, by the app");
    }

    #[test]
    fn a_previous_folder_that_cannot_move_keeps_being_used() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        std::fs::create_dir_all(&previous).unwrap();
        // `FittyAr` is a file, so the new folder cannot be created.
        std::fs::write(tmp.path().join("FittyAr"), b"").unwrap();
        assert_eq!(adopt_previous(&new, &previous), previous);
        assert!(previous.is_dir());
    }
}
