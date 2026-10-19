//! Unpacking the release archives (`.zip` on Windows, `.tar.xz` on Linux).

use std::fs::{self, File};
use std::io::{BufReader, Read, Seek};
use std::path::{Component, Path, PathBuf};

use crate::UpdateError;

/// Archive formats of the release assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArchiveFormat {
    Zip,
    TarXz,
}

impl ArchiveFormat {
    /// Format of the asset called `name`.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        if name.ends_with(".zip") {
            Some(ArchiveFormat::Zip)
        } else if name.ends_with(".tar.xz") {
            Some(ArchiveFormat::TarXz)
        } else {
            None
        }
    }
}

fn archive_error(e: impl std::fmt::Display) -> UpdateError {
    UpdateError::Archive(e.to_string())
}

/// `path` as a relative path made only of normal components, or `None` if it is
/// absolute or climbs out with `..` (a malicious archive must not write elsewhere).
fn safe_relative(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Unpack the regular files of `archive` into the (empty) folder `dest`. Links and
/// other special entries are skipped.
pub(crate) fn extract(
    archive: &Path,
    format: ArchiveFormat,
    dest: &Path,
) -> Result<(), UpdateError> {
    let file = BufReader::new(File::open(archive)?);
    match format {
        ArchiveFormat::Zip => extract_zip(file, dest),
        ArchiveFormat::TarXz => extract_tar_xz(file, dest),
    }
}

pub(crate) fn extract_zip(reader: impl Read + Seek, dest: &Path) -> Result<(), UpdateError> {
    let mut zip = zip::ZipArchive::new(reader).map_err(archive_error)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(archive_error)?;
        if !entry.is_file() || entry.is_symlink() {
            continue;
        }
        let rel = entry
            .enclosed_name()
            .as_deref()
            .and_then(safe_relative)
            .ok_or_else(|| archive_error(format!("unsafe path in entry {i}")))?;
        let path = dest.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&path)?;
        std::io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

pub(crate) fn extract_tar_xz(reader: impl Read, dest: &Path) -> Result<(), UpdateError> {
    let xz = lzma_rust2::XzReader::new(reader, true);
    let mut tar = tar::Archive::new(xz);
    // Keep the rwx bits but never setuid/setgid/sticky from an archive.
    tar.set_preserve_permissions(false);
    for entry in tar.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        if entry.header().entry_type() != tar::EntryType::Regular {
            continue;
        }
        let raw = entry.path().map_err(archive_error)?.into_owned();
        let rel = safe_relative(&raw)
            .ok_or_else(|| archive_error(format!("unsafe path {}", raw.display())))?;
        let path = dest.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(&path).map_err(archive_error)?;
    }
    Ok(())
}

/// The folder under `root` that holds a file called `name`, searching breadth-first
/// so the shallowest match wins (archives may wrap everything in a top-level folder).
pub(crate) fn find_parent_of(root: &Path, name: &std::ffi::OsStr) -> Option<PathBuf> {
    let mut level = vec![root.to_path_buf()];
    for _ in 0..4 {
        let mut next = Vec::new();
        for dir in level {
            let entries = fs::read_dir(&dir).ok()?;
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_file() && entry.file_name() == name {
                    return Some(dir);
                }
                if kind.is_dir() {
                    next.push(path);
                }
            }
        }
        level = next;
    }
    None
}

/// Every regular file under `root`, as paths relative to it.
pub(crate) fn list_files(root: &Path) -> Result<Vec<PathBuf>, UpdateError> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        for entry in fs::read_dir(root.join(&rel))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let child = rel.join(entry.file_name());
            if kind.is_dir() {
                stack.push(child);
            } else if kind.is_file() {
                out.push(child);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    /// A `.zip` with `files` (name, contents), deflate-compressed like dist's.
    pub(crate) fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        for (name, data) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// A `.tar.xz` with `files` (name, contents), plus a symlink that must be skipped.
    pub(crate) fn tar_xz_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_entry_type(tar::EntryType::Regular);
            tar.append_data(&mut header, name, *data).unwrap();
        }
        let mut link = tar::Header::new_gnu();
        link.set_entry_type(tar::EntryType::Symlink);
        link.set_size(0);
        tar.append_link(&mut link, "evil-link", "/etc/passwd")
            .unwrap();
        let tar = tar.into_inner().unwrap();
        let mut xz =
            lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1)).unwrap();
        xz.write_all(&tar).unwrap();
        xz.finish().unwrap()
    }

    const FILES: &[(&str, &[u8])] = &[
        ("cardfile-x86_64-unknown-linux-gnu/cardfile", b"new binary"),
        (
            "cardfile-x86_64-unknown-linux-gnu/THIRD_PARTY_LICENSES.html",
            b"<p>",
        ),
        (
            "cardfile-x86_64-unknown-linux-gnu/dictionaries/en_US.dic",
            b"words",
        ),
    ];

    fn check_tree(dest: &Path) {
        let root = find_parent_of(dest, std::ffi::OsStr::new("cardfile")).unwrap();
        assert_eq!(root, dest.join("cardfile-x86_64-unknown-linux-gnu"));
        let files = list_files(&root).unwrap();
        assert_eq!(
            files,
            vec![
                PathBuf::from("THIRD_PARTY_LICENSES.html"),
                PathBuf::from("cardfile"),
                Path::new("dictionaries").join("en_US.dic"),
            ]
        );
        assert_eq!(fs::read(root.join("cardfile")).unwrap(), b"new binary");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(root.join("cardfile"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
    }

    #[test]
    fn zip_extracts() {
        let dir = tempfile::tempdir().unwrap();
        extract_zip(Cursor::new(zip_of(FILES)), dir.path()).unwrap();
        check_tree(dir.path());
    }

    #[test]
    fn tar_xz_extracts_and_skips_links() {
        let dir = tempfile::tempdir().unwrap();
        extract_tar_xz(Cursor::new(tar_xz_of(FILES)), dir.path()).unwrap();
        check_tree(dir.path());
        assert!(!dir.path().join("evil-link").exists());
    }

    #[test]
    fn rejects_path_traversal() {
        assert_eq!(safe_relative(Path::new("../x")), None);
        assert_eq!(safe_relative(Path::new("/etc/x")), None);
        assert_eq!(safe_relative(Path::new("a/../../x")), None);
        assert_eq!(
            safe_relative(Path::new("./a/b")),
            Some(Path::new("a").join("b"))
        );
        // `zip` itself refuses to write a `..` name, so build the evil tar by hand.
        let mut header = tar::Header::new_gnu();
        header.set_size(1);
        header.set_entry_type(tar::EntryType::Regular);
        // `set_path` refuses `..` too: write the raw name field.
        let name = b"../escape";
        header.as_old_mut().name[..name.len()].copy_from_slice(name);
        header.set_cksum();
        let mut tar = tar::Builder::new(Vec::new());
        tar.append(&header, &b"x"[..]).unwrap();
        let tar = tar.into_inner().unwrap();
        let mut xz =
            lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::with_preset(1)).unwrap();
        xz.write_all(&tar).unwrap();
        let data = xz.finish().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir(&dest).unwrap();
        let err = extract_tar_xz(Cursor::new(data), &dest).unwrap_err();
        assert!(matches!(err, UpdateError::Archive(_)));
        assert!(!dir.path().join("escape").exists());
    }

    #[test]
    fn format_from_name() {
        assert_eq!(
            ArchiveFormat::from_name("cardfile-x86_64-pc-windows-msvc.zip"),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            ArchiveFormat::from_name("cardfile-x86_64-unknown-linux-gnu.tar.xz"),
            Some(ArchiveFormat::TarXz)
        );
        assert_eq!(
            ArchiveFormat::from_name("Cardfile-2.0.0-x86_64.AppImage"),
            None
        );
    }
}
