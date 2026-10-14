//! Where Cardfile keeps its data.
//!
//! - Standard mode: the platform data directory (`%APPDATA%\FittyAr\Cardfile`,
//!   `$XDG_DATA_HOME/cardfile`).
//! - Portable mode: a `data` folder next to the executable, enabled by a
//!   `portable.flag` file there or the `--portable` flag.

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
        let portable = force_portable
            || exe_dir
                .as_deref()
                .is_some_and(|d| d.join(PORTABLE_FLAG).exists());
        if portable && let Some(dir) = exe_dir {
            return Self::portable_at(&dir);
        }
        Self::standard()
    }

    pub fn standard() -> Self {
        let root = directories::ProjectDirs::from("ar", "FittyAr", "Cardfile")
            .map(|d| d.data_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from(".cardfile"));
        Self {
            root,
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

fn exe_dir() -> Option<PathBuf> {
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
}
