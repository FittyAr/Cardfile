//! How the running copy of Cardfile was installed, and which release asset updates it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{Asset, Release, UpdateError};

/// Prefix of every temporary file or folder the updater creates next to the app.
pub(crate) const TEMP_PREFIX: &str = ".cardfile-update-";

/// How the running copy of Cardfile was installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallKind {
    /// Microsoft Store / MSIX (`WindowsApps`). The Store updates it.
    Store,
    /// A Linux package manager (`.deb`/`.rpm` under `/usr`). The package manager updates it.
    PackageManager,
    /// Flatpak. Flatpak updates it.
    Flatpak,
    /// Snap. snapd updates it.
    Snap,
    /// An AppImage; the update replaces the `.AppImage` file itself.
    AppImage {
        /// Path of the `.AppImage` file (from `$APPIMAGE`).
        appimage: PathBuf,
    },
    /// Installed by the Inno Setup installer (`unins000.exe` next to the exe); the update
    /// runs the new installer silently.
    InnoSetup {
        /// The installed executable.
        exe: PathBuf,
        /// The install folder is writable without elevation (a per-user install).
        per_user: bool,
    },
    /// A portable copy (`portable.flag` next to the exe); the update replaces the files.
    Portable {
        /// The running executable.
        exe: PathBuf,
    },
    /// A bare executable in a writable folder, e.g. from the `install.ps1` / `install.sh`
    /// one-line installers (`%LOCALAPPDATA%\Programs\FittyAr\Cardfile`,
    /// `~/.local/lib/FittyAr/Cardfile`) or
    /// an unpacked archive; the update replaces the files.
    Standalone {
        /// The running executable.
        exe: PathBuf,
    },
    /// A copy in a folder we cannot write to (e.g. copied into a system folder by hand).
    ReadOnly {
        /// The running executable (or AppImage).
        exe: PathBuf,
    },
    /// The executable path could not be determined.
    Unknown,
}

impl InstallKind {
    /// Detect how the running executable was installed, from its path, the environment
    /// and the files next to it. Touches the file system (probes write access).
    pub fn detect() -> Self {
        match std::env::current_exe() {
            Ok(exe) => classify(&exe, &|key| std::env::var_os(key), &RealFs),
            Err(_) => InstallKind::Unknown,
        }
    }

    /// Whether Cardfile can download and install updates itself. When `false`, only
    /// announce the release and open [`Release::url`].
    pub fn is_self_updatable(&self) -> bool {
        matches!(
            self,
            InstallKind::AppImage { .. }
                | InstallKind::InnoSetup { .. }
                | InstallKind::Portable { .. }
                | InstallKind::Standalone { .. }
        )
    }
}

/// File-system questions [`classify`] needs; faked in tests.
pub(crate) trait Probe {
    fn exists(&self, path: &Path) -> bool;
    fn writable(&self, dir: &Path) -> bool;
}

pub(crate) struct RealFs;

impl Probe for RealFs {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn writable(&self, dir: &Path) -> bool {
        dir_writable(dir)
    }
}

/// Whether we may create files in `dir`. Creating a file is the only reliable test
/// (Windows ACLs and read-only mounts are not visible in the permission bits).
pub(crate) fn dir_writable(dir: &Path) -> bool {
    tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .tempfile_in(dir)
        .is_ok()
}

/// Classify the executable at `exe`, given environment lookups `env` and file-system `fs`.
pub(crate) fn classify(
    exe: &Path,
    env: &dyn Fn(&str) -> Option<OsString>,
    fs: &dyn Probe,
) -> InstallKind {
    let var = |key: &str| env(key).filter(|v| !v.is_empty());
    if var("FLATPAK_ID").is_some() {
        return InstallKind::Flatpak;
    }
    if var("SNAP").is_some() {
        return InstallKind::Snap;
    }
    if let Some(appimage) = var("APPIMAGE").map(PathBuf::from) {
        let writable = appimage.parent().is_some_and(|dir| fs.writable(dir));
        return if writable {
            InstallKind::AppImage { appimage }
        } else {
            InstallKind::ReadOnly { exe: appimage }
        };
    }
    let text = exe.to_string_lossy();
    let lower = text.to_lowercase();
    if lower.contains(r"\windowsapps\") || lower.contains("/windowsapps/") {
        return InstallKind::Store;
    }
    // `/usr/local` is where people put programs by hand; the rest of `/usr` is the
    // package manager's.
    if text.starts_with("/usr/") && !text.starts_with("/usr/local/") {
        return InstallKind::PackageManager;
    }
    let Some(dir) = exe.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return InstallKind::Unknown;
    };
    let exe = exe.to_path_buf();
    if fs.exists(&dir.join("unins000.exe")) {
        let per_user = fs.writable(dir);
        return InstallKind::InnoSetup { exe, per_user };
    }
    if !fs.writable(dir) {
        return InstallKind::ReadOnly { exe };
    }
    if fs.exists(&dir.join("portable.flag")) {
        InstallKind::Portable { exe }
    } else {
        InstallKind::Standalone { exe }
    }
}

/// Operating systems Cardfile ships for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
}

/// CPU architectures Cardfile ships for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    /// x86-64 / x64.
    X86_64,
    /// ARM64 / AArch64.
    Aarch64,
}

impl Arch {
    /// Architecture as written in Rust target triples and AppImage names.
    fn triple(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Aarch64 => "aarch64",
        }
    }

    /// Architecture as written in the Inno Setup installer names.
    fn windows(self) -> &'static str {
        match self {
            Arch::X86_64 => "x64",
            Arch::Aarch64 => "arm64",
        }
    }
}

/// An OS and CPU architecture pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    /// Operating system.
    pub os: Os,
    /// CPU architecture.
    pub arch: Arch,
}

impl Platform {
    /// The platform this binary was built for, or `None` if releases do not cover it.
    pub fn current() -> Option<Self> {
        let os = if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            return None;
        };
        let arch = if cfg!(target_arch = "x86_64") {
            Arch::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Arch::Aarch64
        } else {
            return None;
        };
        Some(Platform { os, arch })
    }
}

/// The release asset that updates an installation of `kind` on `platform`:
/// the `.zip` (Windows) or `.tar.xz` (Linux) archive for standalone and portable
/// copies, the matching `.AppImage`, or the Inno Setup `-setup-<arch>.exe`.
pub fn select_asset<'r>(
    release: &'r Release,
    kind: &InstallKind,
    platform: Platform,
) -> Result<&'r Asset, UpdateError> {
    let arch = platform.arch.triple();
    let find = |what: &str, pred: &dyn Fn(&str) -> bool| {
        release
            .assets
            .iter()
            .find(|a| pred(&a.name))
            .ok_or_else(|| UpdateError::Unsupported(format!("the release has no {what}")))
    };
    let wrong_os = || {
        Err(UpdateError::Unsupported(format!(
            "{kind:?} is not available on {:?}",
            platform.os
        )))
    };
    match (kind, platform.os) {
        (InstallKind::Standalone { .. } | InstallKind::Portable { .. }, Os::Windows) => {
            let name = format!("cardfile-{arch}-pc-windows-msvc.zip");
            find(&name, &|n| n == name)
        }
        (InstallKind::Standalone { .. } | InstallKind::Portable { .. }, Os::Linux) => {
            let name = format!("cardfile-{arch}-unknown-linux-gnu.tar.xz");
            find(&name, &|n| n == name)
        }
        (InstallKind::AppImage { .. }, Os::Linux) => {
            let suffix = format!("-{arch}.AppImage");
            find(&format!("Cardfile-*{suffix}"), &|n| {
                n.starts_with("Cardfile-") && n.ends_with(&suffix)
            })
        }
        (InstallKind::InnoSetup { .. }, Os::Windows) => {
            let suffix = format!("-setup-{}.exe", platform.arch.windows());
            find(&format!("Cardfile-*{suffix}"), &|n| {
                n.starts_with("Cardfile-") && n.ends_with(&suffix)
            })
        }
        (InstallKind::AppImage { .. } | InstallKind::InnoSetup { .. }, _) => wrong_os(),
        _ => Err(UpdateError::Unsupported(format!(
            "{kind:?} is updated by the platform"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[derive(Default)]
    struct FakeFs {
        files: HashSet<PathBuf>,
        writable: HashSet<PathBuf>,
    }

    impl Probe for FakeFs {
        fn exists(&self, path: &Path) -> bool {
            self.files.contains(path)
        }
        fn writable(&self, dir: &Path) -> bool {
            self.writable.contains(dir)
        }
    }

    fn run(exe: &str, env: &[(&str, &str)], fs: &FakeFs) -> InstallKind {
        let env: HashMap<String, OsString> = env
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        classify(Path::new(exe), &|k| env.get(k).cloned(), fs)
    }

    fn fs(files: &[&str], writable: &[&str]) -> FakeFs {
        FakeFs {
            files: files.iter().map(PathBuf::from).collect(),
            writable: writable.iter().map(PathBuf::from).collect(),
        }
    }

    #[test]
    fn platform_managed_installs() {
        let none = FakeFs::default();
        assert_eq!(
            run(
                r"C:\Program Files\WindowsApps\FittyAr.Cardfile_2.0.0.0_x64__abc\cardfile.exe",
                &[],
                &none
            ),
            InstallKind::Store
        );
        assert_eq!(
            run("/usr/bin/cardfile", &[], &none),
            InstallKind::PackageManager
        );
        assert_eq!(
            run(
                "/app/bin/cardfile",
                &[("FLATPAK_ID", "ar.fittyar.Cardfile")],
                &none
            ),
            InstallKind::Flatpak
        );
        assert_eq!(
            run(
                "/snap/cardfile/1/bin/cardfile",
                &[("SNAP", "/snap/cardfile/1")],
                &none
            ),
            InstallKind::Snap
        );
        // An empty variable does not count.
        assert_eq!(
            run("/usr/bin/cardfile", &[("SNAP", "")], &none),
            InstallKind::PackageManager
        );
        for kind in [
            InstallKind::Store,
            InstallKind::PackageManager,
            InstallKind::Flatpak,
            InstallKind::Snap,
            InstallKind::Unknown,
            InstallKind::ReadOnly { exe: "x".into() },
        ] {
            assert!(!kind.is_self_updatable(), "{kind:?}");
        }
    }

    #[test]
    fn appimage() {
        let env = [
            ("APPIMAGE", "/home/u/Apps/Cardfile-2.0.0-x86_64.AppImage"),
            ("APPDIR", "/tmp/.mount_Cardfi"),
        ];
        let exe = "/tmp/.mount_Cardfi/usr/bin/cardfile";
        assert_eq!(
            run(exe, &env, &fs(&[], &["/home/u/Apps"])),
            InstallKind::AppImage {
                appimage: "/home/u/Apps/Cardfile-2.0.0-x86_64.AppImage".into()
            }
        );
        assert_eq!(
            run(exe, &env, &FakeFs::default()),
            InstallKind::ReadOnly {
                exe: "/home/u/Apps/Cardfile-2.0.0-x86_64.AppImage".into()
            }
        );
    }

    #[test]
    fn inno_setup() {
        let exe = "C:/Program Files/Cardfile/cardfile.exe";
        let files = ["C:/Program Files/Cardfile/unins000.exe"];
        let kind = run(exe, &[], &fs(&files, &[]));
        assert_eq!(
            kind,
            InstallKind::InnoSetup {
                exe: exe.into(),
                per_user: false
            }
        );
        assert!(kind.is_self_updatable());
        let kind = run(exe, &[], &fs(&files, &["C:/Program Files/Cardfile"]));
        assert!(matches!(
            kind,
            InstallKind::InnoSetup { per_user: true, .. }
        ));
    }

    #[test]
    fn standalone_and_portable() {
        let exe = "C:/Users/u/.cardfile/bin/cardfile.exe";
        let dir = "C:/Users/u/.cardfile/bin";
        assert_eq!(
            run(exe, &[], &fs(&[], &[dir])),
            InstallKind::Standalone { exe: exe.into() }
        );
        assert_eq!(
            run(
                exe,
                &[],
                &fs(&["C:/Users/u/.cardfile/bin/portable.flag"], &[dir])
            ),
            InstallKind::Portable { exe: exe.into() }
        );
        assert_eq!(
            run(exe, &[], &FakeFs::default()),
            InstallKind::ReadOnly { exe: exe.into() }
        );
        // `/usr/local` is not a package manager location.
        assert_eq!(
            run(
                "/usr/local/bin/cardfile",
                &[],
                &fs(&[], &["/usr/local/bin"])
            ),
            InstallKind::Standalone {
                exe: "/usr/local/bin/cardfile".into()
            }
        );
        assert_eq!(
            run("cardfile", &[], &FakeFs::default()),
            InstallKind::Unknown
        );
    }

    #[test]
    fn real_write_probe() {
        let dir = tempfile::tempdir().unwrap();
        assert!(dir_writable(dir.path()));
        assert!(!dir_writable(&dir.path().join("missing")));
        // The probe leaves nothing behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    fn release() -> Release {
        let names = [
            "cardfile-x86_64-pc-windows-msvc.zip",
            "cardfile-aarch64-pc-windows-msvc.zip",
            "cardfile-x86_64-unknown-linux-gnu.tar.xz",
            "cardfile-aarch64-unknown-linux-gnu.tar.xz",
            "cardfile-x86_64-unknown-linux-gnu.tar.xz.minisig",
            "Cardfile-2.0.0-alpha.2-setup-x64.exe",
            "Cardfile-2.0.0-alpha.2-setup-arm64.exe",
            "Cardfile-2.0.0-alpha.2-setup-x64.exe.minisig",
            "Cardfile-2.0.0-alpha.2-x86_64.AppImage",
            "Cardfile-2.0.0-alpha.2-aarch64.AppImage",
            "Cardfile-2.0.0-alpha.2-x64.msix",
            "cardfile_2.0.0~alpha.2-1_amd64.deb",
            "SHA256SUMS",
        ];
        Release {
            version: semver::Version::new(2, 0, 0),
            tag: "v2.0.0-alpha.2".into(),
            url: String::new(),
            notes: String::new(),
            assets: names
                .iter()
                .map(|n| Asset {
                    name: n.to_string(),
                    url: format!("https://x/{n}"),
                    size: 1,
                })
                .collect(),
        }
    }

    fn pick(kind: &InstallKind, os: Os, arch: Arch) -> Result<String, UpdateError> {
        let r = release();
        select_asset(&r, kind, Platform { os, arch }).map(|a| a.name.clone())
    }

    #[test]
    fn asset_selection() {
        let standalone = InstallKind::Standalone { exe: "x".into() };
        let portable = InstallKind::Portable { exe: "x".into() };
        let appimage = InstallKind::AppImage {
            appimage: "x".into(),
        };
        let inno = InstallKind::InnoSetup {
            exe: "x".into(),
            per_user: true,
        };
        assert_eq!(
            pick(&standalone, Os::Windows, Arch::X86_64).unwrap(),
            "cardfile-x86_64-pc-windows-msvc.zip"
        );
        assert_eq!(
            pick(&portable, Os::Windows, Arch::Aarch64).unwrap(),
            "cardfile-aarch64-pc-windows-msvc.zip"
        );
        assert_eq!(
            pick(&standalone, Os::Linux, Arch::Aarch64).unwrap(),
            "cardfile-aarch64-unknown-linux-gnu.tar.xz"
        );
        assert_eq!(
            pick(&appimage, Os::Linux, Arch::X86_64).unwrap(),
            "Cardfile-2.0.0-alpha.2-x86_64.AppImage"
        );
        assert_eq!(
            pick(&appimage, Os::Linux, Arch::Aarch64).unwrap(),
            "Cardfile-2.0.0-alpha.2-aarch64.AppImage"
        );
        assert_eq!(
            pick(&inno, Os::Windows, Arch::X86_64).unwrap(),
            "Cardfile-2.0.0-alpha.2-setup-x64.exe"
        );
        assert_eq!(
            pick(&inno, Os::Windows, Arch::Aarch64).unwrap(),
            "Cardfile-2.0.0-alpha.2-setup-arm64.exe"
        );
        assert!(pick(&inno, Os::Linux, Arch::X86_64).is_err());
        assert!(pick(&appimage, Os::Windows, Arch::X86_64).is_err());
        assert!(pick(&InstallKind::Store, Os::Windows, Arch::X86_64).is_err());
        assert!(pick(&InstallKind::PackageManager, Os::Linux, Arch::X86_64).is_err());

        let mut empty = release();
        empty.assets.clear();
        let err = select_asset(
            &empty,
            &standalone,
            Platform {
                os: Os::Linux,
                arch: Arch::X86_64,
            },
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::Unsupported(_)));
    }
}
