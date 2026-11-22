//! Guards the shell start-up lines of the cargo-dist installer (2.0.0-alpha.1
//! to 2.0.0-alpha.3).
//!
//! That installer appended `. "$HOME/.cardfile/bin/env"` to `~/.profile`,
//! `~/.bashrc`, `~/.zshrc`, ... without checking that the file exists. When
//! `~/.cardfile` is removed, every `/bin/sh` that reads `~/.profile` aborts on
//! that line, and display managers (SDDM, Plasma Login, GDM) read it before
//! starting the desktop: the login ends in a black screen. On every start the
//! line is rewritten into a guarded one that keeps the same `PATH` while the
//! file exists and does nothing once it is gone. Lines are never removed here;
//! `install.sh` removes them together with the old installation.

use std::path::{Path, PathBuf};

const LEGACY_LINES: [&str; 2] = [
    ". \"$HOME/.cardfile/bin/env\"",
    "source \"$HOME/.cardfile/bin/env\"",
];
const GUARDED_LINE: &str =
    "if [ -f \"$HOME/.cardfile/bin/env\" ]; then . \"$HOME/.cardfile/bin/env\"; fi";
const LEGACY_FISH_LINE: &str = "source \"$HOME/.cardfile/bin/env.fish\"";
const GUARDED_FISH_LINE: &str =
    "test -f \"$HOME/.cardfile/bin/env.fish\"; and source \"$HOME/.cardfile/bin/env.fish\"";

/// Rewrite the unguarded legacy lines in the user's shell profiles. Errors are
/// logged, never fatal: Cardfile must start either way.
pub fn repair() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let zdot = std::env::var_os("ZDOTDIR").map_or_else(|| home.clone(), PathBuf::from);
    let mut files: Vec<PathBuf> = [".profile", ".bash_profile", ".bash_login", ".bashrc"]
        .iter()
        .map(|f| home.join(f))
        .collect();
    for dir in [&zdot, &home] {
        for f in [".zshrc", ".zshenv"] {
            let path = dir.join(f);
            if !files.contains(&path) {
                files.push(path);
            }
        }
    }
    for file in &files {
        repair_file(file, guard_posix);
    }
    repair_file(
        &home.join(".config/fish/conf.d/cardfile.env.fish"),
        guard_fish,
    );
}

fn repair_file(path: &Path, guard: fn(&str) -> Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Some(fixed) = guard(&text) else { return };
    match replace_contents(path, &fixed) {
        Ok(()) => tracing::info!("guarded the old Cardfile line in {}", path.display()),
        Err(e) => tracing::warn!("could not repair {}: {e}", path.display()),
    }
}

/// Write `contents` next to the real file (symlinks resolved), with its
/// permissions, then rename it over: a crash never leaves a cut-off profile.
fn replace_contents(path: &Path, contents: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path)?;
    let permissions = std::fs::metadata(&target)?.permissions();
    let mut tmp = target.clone().into_os_string();
    tmp.push(".cardfile-repair");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, contents)?;
    std::fs::set_permissions(&tmp, permissions)?;
    std::fs::rename(&tmp, &target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The profile with every legacy line guarded, or `None` when nothing changes.
fn guard_posix(text: &str) -> Option<String> {
    rewrite_lines(text, |line| LEGACY_LINES.contains(&line), GUARDED_LINE)
}

/// The fish `conf.d` file with its legacy line guarded, or `None`.
fn guard_fish(text: &str) -> Option<String> {
    rewrite_lines(text, |line| line == LEGACY_FISH_LINE, GUARDED_FISH_LINE)
}

fn rewrite_lines(text: &str, is_legacy: impl Fn(&str) -> bool, guarded: &str) -> Option<String> {
    let mut changed = false;
    let out: Vec<String> = text
        .split('\n')
        .map(|line| {
            let trimmed = line.trim();
            if is_legacy(trimmed) {
                changed = true;
                let indent = &line[..line.len() - line.trim_start().len()];
                format!("{indent}{guarded}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    changed.then(|| out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_the_cargo_dist_line() {
        let text = "# ~/.profile\n\n. \"$HOME/.cardfile/bin/env\"\n";
        assert_eq!(
            guard_posix(text).as_deref(),
            Some(format!("# ~/.profile\n\n{GUARDED_LINE}\n").as_str())
        );
    }

    #[test]
    fn keeps_indentation_and_other_lines() {
        let text = "alias a=b\r\n  source \"$HOME/.cardfile/bin/env\"\r\nexport Z=1";
        assert_eq!(
            guard_posix(text).as_deref(),
            Some(format!("alias a=b\r\n  {GUARDED_LINE}\nexport Z=1").as_str())
        );
    }

    #[test]
    fn leaves_guarded_and_unrelated_profiles_alone() {
        assert_eq!(guard_posix(&format!("{GUARDED_LINE}\n")), None);
        assert_eq!(guard_posix(". \"$HOME/.cargo/env\"\n"), None);
    }

    #[test]
    fn guards_the_fish_file() {
        let text = format!("{LEGACY_FISH_LINE}\n");
        assert_eq!(
            guard_fish(&text).as_deref(),
            Some(format!("{GUARDED_FISH_LINE}\n").as_str())
        );
        assert_eq!(guard_fish(&format!("{GUARDED_FISH_LINE}\n")), None);
    }

    #[test]
    fn repairs_a_file_on_disk() {
        let dir = std::env::temp_dir().join(format!("cardfile-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let profile = dir.join(".profile");
        std::fs::write(&profile, ". \"$HOME/.cardfile/bin/env\"\n").unwrap();
        repair_file(&profile, guard_posix);
        assert_eq!(
            std::fs::read_to_string(&profile).unwrap(),
            format!("{GUARDED_LINE}\n")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
