//! Where NoteChest keeps its data.
//!
//! - Standard mode: a `FittyAr/NoteChest` folder, shared layout of every FittyAr
//!   app: `%APPDATA%\FittyAr\NoteChest\data` on Windows,
//!   `$XDG_DATA_HOME/FittyAr/NoteChest` (`~/.local/share` when unset) on Linux and
//!   `~/Library/Application Support/FittyAr/NoteChest` on macOS. The folder of the
//!   earlier layout (`$XDG_DATA_HOME/notechest`, `ar.FittyAr.NoteChest` on macOS)
//!   is renamed into place on the first start: never deleted or merged, and kept
//!   in use when it cannot be moved.
//! - Portable mode: a `data` folder next to the executable, enabled by a
//!   `portable.flag` file there, the `--portable` flag, or a `data/notechest.db`
//!   already next to the executable (a copied folder keeps working without the
//!   flag). Choosing the standard location in the first-run wizard removes the
//!   flag, so that choice sticks. A folder kept in sync by OneDrive, Dropbox and
//!   the like works but is risky (see [`DataPaths::sync_service`]); the app warns.
//!
//! Collection settings live in `notechest.db` (installation settings in
//! `app_settings`, preferences on each user row). Window and backup preferences
//! live in `local-config.json` and survive database restoration.

use std::path::{Path, PathBuf};

pub const PORTABLE_FLAG: &str = "portable.flag";

/// Name of the file, in the user's configuration folder, that points to a data
/// folder moved away from the standard location (Settings → Data → Move).
const LOCATION_FILE: &str = "data-location.txt";

/// Application name (folder under `FittyAr`, and `ProjectDirs` name) before the project
/// was renamed from Cardfile to NoteChest (2.0.0).
const LEGACY_APP_DIR: &str = "Cardfile";

/// How the data folder was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataOrigin {
    /// The standard per-user folder.
    Standard,
    /// A `data` folder next to the executable.
    Portable,
    /// A folder the user moved the data to from inside NoteChest.
    Moved,
    /// The `--data-dir` command-line option.
    CommandLine,
}

/// The data folder NoteChest uses and the files inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataPaths {
    /// The data folder; every other path is inside it.
    pub root: PathBuf,
    /// Whether the folder sits next to the executable (portable mode).
    pub portable: bool,
    /// How the folder was chosen; only standard and moved folders can be moved.
    pub origin: DataOrigin,
}

impl DataPaths {
    /// An installer or explicit CLI option has already selected storage.
    pub fn storage_preselected() -> bool {
        std::env::args().any(|a| a == "--portable" || a == "--data-dir")
            || exe_dir().is_some_and(|dir| {
                dir.join("portable.flag").exists() || dir.join("installed.flag").exists()
            })
            || location_file().is_some_and(|f| f.exists())
    }
    /// The SQLite database, `notechest.db`.
    pub fn db(&self) -> PathBuf {
        self.root.join(notechest_db::DATABASE_FILE)
    }
    /// Whether the folder holds a collection: `notechest.db`, or the `cardfile.db` of a
    /// version from before the rename, which opening it adopts under the new name.
    pub fn has_database(&self) -> bool {
        notechest_db::has_collection(&self.root)
    }
    /// Folder of database backups.
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    /// Folder of log files.
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    /// Folder of user theme files.
    pub fn themes(&self) -> PathBuf {
        self.root.join("themes")
    }
    /// The user's key binding overrides, `keymap.json`.
    pub fn keymap(&self) -> PathBuf {
        self.root.join("keymap.json")
    }
    /// Folder of spell-checking dictionaries.
    pub fn dictionaries(&self) -> PathBuf {
        self.root.join("dictionaries")
    }

    /// Resolve the data location. `override_dir` (from `--data-dir`) wins.
    pub fn resolve(override_dir: Option<PathBuf>, force_portable: bool) -> Self {
        if let Some(root) = override_dir {
            return Self {
                root,
                portable: false,
                origin: DataOrigin::CommandLine,
            };
        }
        let exe_dir = exe_dir();
        let portable = force_portable || exe_dir.as_deref().is_some_and(is_portable_dir);
        if portable && let Some(dir) = exe_dir {
            return Self::portable_at(&dir);
        }
        if let Some(root) = moved_location() {
            return Self {
                root,
                portable: false,
                origin: DataOrigin::Moved,
            };
        }
        Self::standard()
    }

    /// The standard location. Blocking on Linux and macOS (it may move the folder of
    /// the earlier layout into place): never call it while rendering.
    pub fn standard() -> Self {
        Self {
            root: standard_root().unwrap_or_else(|| PathBuf::from(".notechest")),
            portable: false,
            origin: DataOrigin::Standard,
        }
    }

    /// Portable layout for an executable in `exe_dir`: data in `exe_dir/data`.
    /// Does not touch the disk.
    pub fn portable_at(exe_dir: &Path) -> Self {
        Self {
            root: exe_dir.join("data"),
            portable: true,
            origin: DataOrigin::Portable,
        }
    }

    /// Switch to portable mode: write the flag next to the executable.
    pub fn make_portable() -> std::io::Result<Self> {
        let dir = exe_dir().ok_or_else(|| std::io::Error::other("cannot locate the executable"))?;
        std::fs::write(
            dir.join(PORTABLE_FLAG),
            b"NoteChest portable mode: data is stored in ./data\n",
        )?;
        Ok(Self::portable_at(&dir))
    }

    /// Switch to standard mode (the user chose it, e.g. in the wizard of a copy that
    /// started portable): remove the flag next to the executable, so the next start
    /// does not go back to the empty portable folder. Blocking (file system).
    pub fn make_standard() -> Self {
        if let Some(dir) = exe_dir() {
            remove_flag(&dir);
        }
        Self::standard()
    }

    /// Whether the executable's folder is writable (portable mode needs it).
    /// Blocking (writes a probe file): never call it while rendering.
    pub fn portable_available() -> bool {
        let Some(dir) = exe_dir() else { return false };
        let probe = dir.join(".notechest-write-test");
        let ok = std::fs::write(&probe, b"").is_ok();
        let _ = std::fs::remove_file(&probe);
        ok
    }

    /// Whether the folder can be moved from Settings: portable folders live next to
    /// the executable by design, and `--data-dir` is chosen by whoever starts the app.
    pub fn movable(&self) -> bool {
        matches!(self.origin, DataOrigin::Standard | DataOrigin::Moved)
    }

    /// Use `root` from the next start on. Pointing back at the standard folder
    /// removes the pointer file instead of writing it. Blocking (file system).
    pub fn set_location(root: &Path) -> std::io::Result<()> {
        let file = location_file()
            .ok_or_else(|| std::io::Error::other("no configuration folder for this user"))?;
        if standard_root().is_some_and(|standard| standard == root) {
            return match std::fs::remove_file(&file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        write_location(&file, root)
    }

    /// The pointer file, shown when a moved folder cannot be found.
    pub fn location_file() -> Option<PathBuf> {
        location_file()
    }

    /// The file-sync service whose folder holds the data, if any. The sync client
    /// may upload the database between writes (the main file without its WAL) or
    /// make conflict copies when two computers use it, so the user is warned.
    pub fn sync_service(&self) -> Option<&'static str> {
        sync_service_of(&self.root, |name| std::env::var_os(name).map(PathBuf::from))
    }
}

/// Where the pointer to a moved data folder lives: the per-user configuration folder
/// (`%APPDATA%\FittyAr\NoteChest\config` on Windows, `$XDG_CONFIG_HOME/notechest`, usually
/// `~/.config/notechest`, on Linux), which stays put whatever happens to the data folder.
fn location_file() -> Option<PathBuf> {
    directories::ProjectDirs::from("ar", "FittyAr", "NoteChest")
        .map(|d| d.config_dir().join(LOCATION_FILE))
}

/// The pointer of the previous name, in the configuration folder it used.
fn legacy_location_file() -> Option<PathBuf> {
    directories::ProjectDirs::from("ar", "FittyAr", LEGACY_APP_DIR)
        .map(|d| d.config_dir().join(LOCATION_FILE))
}

/// The data folder the user moved to, if any: the pointer file of this name, else the
/// one written by a version from before the rename (copied forward so it is found
/// directly from now on; the old file is left alone).
fn moved_location() -> Option<PathBuf> {
    let current = location_file()?;
    if let Some(root) = read_location(&current) {
        return Some(root);
    }
    let root = read_location(&legacy_location_file()?)?;
    if let Err(e) = write_location(&current, &root) {
        tracing::warn!(
            "could not copy the data folder pointer to {}: {e}",
            current.display()
        );
    }
    Some(root)
}

/// The folder a pointer file names; `None` for a missing, empty or relative entry.
fn read_location(file: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(file).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_absolute().then_some(path)
}

/// Write the pointer atomically: a torn write would send the next start to a wrong,
/// empty folder.
fn write_location(file: &Path, root: &Path) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = file
        .parent()
        .ok_or_else(|| std::io::Error::other("invalid configuration folder"))?;
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(root.as_os_str().to_string_lossy().as_bytes())?;
    tmp.write_all(b"\n")?;
    tmp.as_file().sync_all()?;
    tmp.persist(file).map_err(|e| e.error)?;
    Ok(())
}

/// [`DataPaths::sync_service`] with the environment lookup passed in (for tests).
fn sync_service_of(root: &Path, env: impl Fn(&str) -> Option<PathBuf>) -> Option<&'static str> {
    // OneDrive publishes its folders; a folder may also be named after the account.
    let onedrive = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .into_iter()
        .filter_map(&env)
        .any(|dir| !dir.as_os_str().is_empty() && root.starts_with(&dir));
    if onedrive {
        return Some("OneDrive");
    }
    root.components().find_map(|part| {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        match name.as_str() {
            "onedrive" => Some("OneDrive"),
            n if n.starts_with("onedrive - ") => Some("OneDrive"),
            "dropbox" => Some("Dropbox"),
            "google drive" | "my drive" | "mi unidad" => Some("Google Drive"),
            "iclouddrive" | "icloud drive" => Some("iCloud Drive"),
            "nextcloud" => Some("Nextcloud"),
            "pcloud drive" => Some("pCloud"),
            "megasync" | "mega" => Some("MEGA"),
            _ => None,
        }
    })
}

/// The standard data folder of this platform (see the module docs).
#[cfg(target_os = "windows")]
fn standard_root() -> Option<PathBuf> {
    let current = directories::ProjectDirs::from("ar", "FittyAr", "NoteChest")?;
    let legacy = directories::ProjectDirs::from("ar", "FittyAr", LEGACY_APP_DIR)?;
    // `data_dir()` is `%APPDATA%\FittyAr\<app>\data`: the whole `<app>` folder (data and
    // configuration) of the previous name is moved, so nothing is left behind.
    let app = adopt_previous(
        current.data_dir().parent()?,
        &[legacy.data_dir().parent()?.to_owned()],
    );
    Some(app.join("data"))
}

/// The standard data folder of this platform (see the module docs).
#[cfg(not(target_os = "windows"))]
fn standard_root() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    let (new, previous) = unix_candidates(dirs.data_dir(), cfg!(target_os = "macos"));
    Some(adopt_previous(&new, &previous))
}

/// `(folder, folders of the previous names, newest first)` under the platform data
/// folder `base` (`$XDG_DATA_HOME`, or `~/Library/Application Support` on macOS).
#[cfg_attr(target_os = "windows", allow(dead_code))]
fn unix_candidates(base: &Path, macos: bool) -> (PathBuf, Vec<PathBuf>) {
    // Before the shared FittyAr layout the folder was named after the app alone.
    let oldest = if macos {
        "ar.FittyAr.Cardfile"
    } else {
        "cardfile"
    };
    (
        base.join("FittyAr").join("NoteChest"),
        vec![base.join("FittyAr").join(LEGACY_APP_DIR), base.join(oldest)],
    )
}

/// The folder to use: `new`, after renaming the newest of `previous` (folders of
/// earlier names or layouts) into place when `new` does not exist yet. Nothing is
/// deleted or merged; if that folder cannot be moved (another file system,
/// permissions, files still open in a running older version) it keeps being used.
fn adopt_previous(new: &Path, previous: &[PathBuf]) -> PathBuf {
    if new.exists() {
        return new.to_path_buf();
    }
    let Some(previous) = previous.iter().find(|p| p.is_dir()) else {
        return new.to_path_buf();
    };
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
            previous.clone()
        }
    }
}

/// Remove the portable flag from the executable folder `dir`, if it is there.
fn remove_flag(dir: &Path) {
    match std::fs::remove_file(dir.join(PORTABLE_FLAG)) {
        Ok(()) => tracing::info!("standard mode chosen; removed {PORTABLE_FLAG}"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        // The next start is portable again and shows the wizard, which finds the
        // standard database already set up and uses it.
        Err(e) => tracing::warn!("cannot remove {PORTABLE_FLAG}: {e}"),
    }
}

/// Whether the executable folder `dir` holds a portable copy: the flag, or the
/// database of an earlier portable run in `data/`.
fn is_portable_dir(dir: &Path) -> bool {
    dir.join(PORTABLE_FLAG).exists() || notechest_db::has_collection(&dir.join("data"))
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
    fn location_pointer_round_trips_and_ignores_bad_entries() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config").join(LOCATION_FILE);
        assert_eq!(read_location(&file), None);
        let target = dir.path().join("Notes");
        write_location(&file, &target).unwrap();
        assert_eq!(read_location(&file), Some(target));
        std::fs::write(&file, "relative/path\n").unwrap();
        assert_eq!(read_location(&file), None);
        std::fs::write(&file, "  \n").unwrap();
        assert_eq!(read_location(&file), None);
    }

    #[test]
    fn synced_folders_are_recognised() {
        let env =
            |name: &str| (name == "OneDrive").then(|| PathBuf::from("C:/Users/me/Personal Cloud"));
        let of = |p: &str| sync_service_of(Path::new(p), env);
        assert_eq!(
            of("C:/Users/me/Personal Cloud/NoteChest/data"),
            Some("OneDrive")
        );
        assert_eq!(
            of("C:/Users/me/OneDrive - Acme/NoteChest/data"),
            Some("OneDrive")
        );
        assert_eq!(of("/home/me/Dropbox/NoteChest/data"), Some("Dropbox"));
        assert_eq!(of("G:/My Drive/NoteChest/data"), Some("Google Drive"));
        assert_eq!(
            of("C:/Users/me/AppData/Roaming/FittyAr/NoteChest/data"),
            None
        );
        // An empty variable must not match every path.
        assert_eq!(
            sync_service_of(Path::new("/data"), |_| Some(PathBuf::new())),
            None
        );
    }

    #[test]
    fn override_wins() {
        let p = DataPaths::resolve(Some(PathBuf::from("x")), true);
        assert_eq!(p.root, PathBuf::from("x"));
        assert_eq!(p.db(), PathBuf::from("x").join("notechest.db"));
    }

    #[test]
    fn a_data_folder_with_the_database_means_portable() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_portable_dir(tmp.path()));
        // An empty `data` folder is not enough: it may belong to something else.
        std::fs::create_dir(tmp.path().join("data")).unwrap();
        assert!(!is_portable_dir(tmp.path()));
        std::fs::write(tmp.path().join("data").join("notechest.db"), b"").unwrap();
        assert!(is_portable_dir(tmp.path()));
        let flag_only = tempfile::tempdir().unwrap();
        std::fs::write(flag_only.path().join(PORTABLE_FLAG), b"").unwrap();
        assert!(is_portable_dir(flag_only.path()));
    }

    #[test]
    fn choosing_standard_removes_the_portable_flag() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(PORTABLE_FLAG), b"").unwrap();
        assert!(is_portable_dir(tmp.path()));
        remove_flag(tmp.path());
        assert!(!is_portable_dir(tmp.path()));
        // No flag: nothing to do.
        remove_flag(tmp.path());
    }

    #[test]
    fn linux_and_macos_use_the_fittyar_folder() {
        let base = Path::new("/home/u/.local/share");
        let (new, previous) = unix_candidates(base, false);
        assert_eq!(new, base.join("FittyAr").join("NoteChest"));
        assert_eq!(
            previous,
            vec![base.join("FittyAr").join("Cardfile"), base.join("cardfile")]
        );
        let base = Path::new("/Users/u/Library/Application Support");
        let (new, previous) = unix_candidates(base, true);
        assert_eq!(new, base.join("FittyAr").join("NoteChest"));
        assert_eq!(previous[1], base.join("ar.FittyAr.Cardfile"));
    }

    #[test]
    fn the_folder_of_the_previous_name_is_moved_into_place() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        std::fs::create_dir_all(&previous[0]).unwrap();
        std::fs::write(previous[0].join("cardfile.db"), b"cards").unwrap();
        assert_eq!(adopt_previous(&new, &previous), new);
        assert_eq!(std::fs::read(new.join("cardfile.db")).unwrap(), b"cards");
        assert!(!previous[0].exists());
    }

    #[test]
    fn the_newest_previous_folder_wins_and_the_other_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        for (dir, content) in [(&previous[0], "newer"), (&previous[1], "older")] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("data.txt"), content).unwrap();
        }
        assert_eq!(adopt_previous(&new, &previous), new);
        assert_eq!(std::fs::read(new.join("data.txt")).unwrap(), b"newer");
        assert!(
            previous[1].join("data.txt").exists(),
            "never merged or deleted"
        );
    }

    #[test]
    fn an_existing_new_folder_is_never_merged() {
        let tmp = tempfile::tempdir().unwrap();
        let (new, previous) = unix_candidates(tmp.path(), false);
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&previous[0]).unwrap();
        std::fs::write(previous[0].join("cardfile.db"), b"old").unwrap();
        assert_eq!(adopt_previous(&new, &previous), new);
        assert!(!new.join("cardfile.db").exists());
        assert!(previous[0].join("cardfile.db").exists(), "old data kept");
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
        std::fs::create_dir_all(&previous[1]).unwrap();
        // `FittyAr` is a file, so the new folder cannot be created.
        std::fs::write(tmp.path().join("FittyAr"), b"").unwrap();
        assert_eq!(adopt_previous(&new, &previous), previous[1]);
        assert!(previous[1].is_dir());
    }

    #[test]
    fn an_old_database_name_still_marks_a_portable_folder_and_a_collection() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("data")).unwrap();
        std::fs::write(tmp.path().join("data").join("cardfile.db"), b"").unwrap();
        assert!(is_portable_dir(tmp.path()));
        let paths = DataPaths {
            root: tmp.path().join("data"),
            portable: true,
            origin: DataOrigin::Portable,
        };
        assert!(paths.has_database());
        assert!(!paths.db().exists(), "adopted under the new name on open");
    }
}
