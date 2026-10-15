//! Downloading and verifying a release asset.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use minisign_verify::{PublicKey, Signature, StreamVerifier};
use sha2::{Digest, Sha256};
use tempfile::TempPath;

use crate::kind::{Platform, TEMP_PREFIX, select_asset};
use crate::{
    Asset, InstallKind, PUBLIC_KEY, Release, UpdateError, check_trusted_comment,
    fetch_signed_checksums, get_text, signature_error, user_agent,
};

/// Hard cap for an asset whose size GitHub did not report.
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;

/// A downloaded and verified update, ready for [`crate::install`].
///
/// The file is temporary: dropping this value without installing deletes it.
#[derive(Debug)]
pub struct Downloaded {
    pub(crate) kind: InstallKind,
    pub(crate) asset: Asset,
    pub(crate) path: TempPath,
}

impl Downloaded {
    /// The release asset that was downloaded.
    pub fn asset(&self) -> &Asset {
        &self.asset
    }

    /// Where the verified file is stored until it is installed.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The installation this update is for.
    pub fn kind(&self) -> &InstallKind {
        &self.kind
    }
}

/// Parse a `sha256sum` listing (`<hex>  <name>` or `<hex> *<name>` per line) into
/// a map from file name to lowercase hex digest.
pub fn parse_checksums(text: &str) -> Result<HashMap<String, String>, UpdateError> {
    let mut sums = HashMap::new();
    for line in text.lines().map(str::trim_end).filter(|l| !l.is_empty()) {
        let bad = || UpdateError::Parse(format!("bad SHA256SUMS line: {line}"));
        let (hash, name) = line.split_once(' ').ok_or_else(bad)?;
        let name = name.trim_start_matches(' ');
        let name = name.strip_prefix('*').unwrap_or(name);
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) || name.is_empty() {
            return Err(bad());
        }
        sums.insert(name.to_string(), hash.to_ascii_lowercase());
    }
    Ok(sums)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        // Writing to a String cannot fail.
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Folder the downloaded file goes to: next to the file it will replace (same file
/// system, so the final rename is atomic), or a temp folder for the Inno Setup installer.
fn download_dir(kind: &InstallKind) -> Result<PathBuf, UpdateError> {
    let parent = |p: &Path| {
        p.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| UpdateError::Unsupported(format!("{} has no folder", p.display())))
    };
    match kind {
        InstallKind::Standalone { exe } | InstallKind::Portable { exe } => parent(exe),
        InstallKind::AppImage { appimage } => parent(appimage),
        InstallKind::InnoSetup { .. } => {
            let dir = crate::install::installer_dir();
            std::fs::create_dir_all(&dir)?;
            Ok(dir)
        }
        other => Err(UpdateError::Unsupported(format!(
            "{other:?} is updated by the platform"
        ))),
    }
}

/// Download the asset of `release` that updates `kind` and verify it.
///
/// The file goes to a temporary file next to its destination. `progress` is called
/// with `(downloaded_bytes, total_bytes)` after every chunk (`total` is `None` when
/// unknown). Setting `cancel` aborts with [`UpdateError::Cancelled`] and deletes the
/// partial file. The result is returned only if the file matches both its SHA-256 in
/// the signed `SHA256SUMS` and its own `.minisig` signature from [`PUBLIC_KEY`].
///
/// Blocking: call off the UI thread.
pub fn download(
    release: &Release,
    kind: &InstallKind,
    mut progress: impl FnMut(u64, Option<u64>),
    cancel: &AtomicBool,
) -> Result<Downloaded, UpdateError> {
    let platform = Platform::current()
        .ok_or_else(|| UpdateError::Unsupported("no releases for this platform".into()))?;
    let asset = select_asset(release, kind, platform)?.clone();
    let dir = download_dir(kind)?;

    // Everything that decides trust is fetched and checked before the big download.
    let checksums = fetch_signed_checksums(release)?;
    let checksums = String::from_utf8(checksums).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let expected = parse_checksums(&checksums)?
        .remove(&asset.name)
        .ok_or_else(|| UpdateError::Checksum(format!("{} is not in SHA256SUMS", asset.name)))?;
    let signature = get_text(&release.download_url(&format!("{}.minisig", asset.name)))?;
    let signature = Signature::decode(&signature).map_err(signature_error)?;
    check_trusted_comment(&signature, &release.tag, &asset.name)?;
    let key = PublicKey::from_base64(PUBLIC_KEY).map_err(signature_error)?;
    let verifier = key.verify_stream(&signature).map_err(signature_error)?;

    if cancel.load(Ordering::Relaxed) {
        return Err(UpdateError::Cancelled);
    }
    let mut file = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .suffix(&format!("-{}", asset.name))
        .tempfile_in(&dir)?;
    let resp = ureq::get(&asset.url)
        .header("User-Agent", user_agent())
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let total = resp
        .body()
        .content_length()
        .or((asset.size > 0).then_some(asset.size));
    let limit = if asset.size > 0 {
        asset.size
    } else {
        MAX_DOWNLOAD
    };
    let mut reader = resp.into_body().into_reader();
    receive(
        &mut reader,
        file.as_file_mut(),
        Expect {
            sha256: &expected,
            verifier,
            total,
            limit,
        },
        &mut progress,
        cancel,
    )?;
    file.as_file_mut().sync_all()?;
    Ok(Downloaded {
        kind: kind.clone(),
        asset,
        path: file.into_temp_path(),
    })
}

/// What the received bytes must match.
pub(crate) struct Expect<'a> {
    pub sha256: &'a str,
    pub verifier: StreamVerifier<'a>,
    pub total: Option<u64>,
    pub limit: u64,
}

/// Copy `reader` into `out` while hashing it, then check the SHA-256 and the
/// minisign signature. Network errors while reading are reported as such.
pub(crate) fn receive(
    reader: &mut dyn Read,
    out: &mut dyn Write,
    mut expect: Expect<'_>,
    progress: &mut dyn FnMut(u64, Option<u64>),
    cancel: &AtomicBool,
) -> Result<(), UpdateError> {
    let mut sha = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    progress(0, expect.total);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(UpdateError::Cancelled);
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(UpdateError::Network(e.to_string())),
        };
        let chunk = &buf[..n];
        done += n as u64;
        if done > expect.limit {
            return Err(UpdateError::Checksum(
                "the download is larger than the release asset".into(),
            ));
        }
        sha.update(chunk);
        expect.verifier.update(chunk);
        out.write_all(chunk)?;
        progress(done, expect.total);
    }
    out.flush()?;
    let actual = hex(&sha.finalize());
    if !actual.eq_ignore_ascii_case(expect.sha256) {
        return Err(UpdateError::Checksum(format!(
            "expected {}, got {actual}",
            expect.sha256
        )));
    }
    expect.verifier.finalize().map_err(signature_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{TEST_KEY, TEST_SIG};

    const TEST_SHA: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn checksums_parse() {
        let text = format!(
            "{TEST_SHA}  cardfile-x86_64-pc-windows-msvc.zip\n\
             {} *Cardfile-2.0.0-setup-x64.exe\n\n",
            TEST_SHA.to_uppercase()
        );
        let sums = parse_checksums(&text).unwrap();
        assert_eq!(sums.len(), 2);
        assert_eq!(sums["cardfile-x86_64-pc-windows-msvc.zip"], TEST_SHA);
        assert_eq!(sums["Cardfile-2.0.0-setup-x64.exe"], TEST_SHA);
        assert!(parse_checksums("abc  file").is_err());
        assert!(parse_checksums(TEST_SHA).is_err());
        assert!(parse_checksums(&format!("{TEST_SHA}  ")).is_err());
    }

    type Outcome = (Result<(), UpdateError>, Vec<u8>, Vec<(u64, Option<u64>)>);

    fn run(data: &[u8], sha: &str, limit: u64, cancel: bool) -> Outcome {
        let key = PublicKey::from_base64(TEST_KEY).unwrap();
        let sig = Signature::decode(TEST_SIG).unwrap();
        let verifier = key.verify_stream(&sig).unwrap();
        let mut out = Vec::new();
        let mut calls = Vec::new();
        let cancel = AtomicBool::new(cancel);
        let result = receive(
            &mut &data[..],
            &mut out,
            Expect {
                sha256: sha,
                verifier,
                total: Some(data.len() as u64),
                limit,
            },
            &mut |d, t| calls.push((d, t)),
            &cancel,
        );
        (result, out, calls)
    }

    #[test]
    fn verified_download() {
        let (result, out, calls) = run(b"test", TEST_SHA, 4, false);
        result.unwrap();
        assert_eq!(out, b"test");
        assert_eq!(calls, vec![(0, Some(4)), (4, Some(4))]);
    }

    #[test]
    fn rejects_bad_checksum() {
        let other = "0".repeat(64);
        let (result, _, _) = run(b"test", &other, 4, false);
        assert!(matches!(result, Err(UpdateError::Checksum(_))));
    }

    #[test]
    fn rejects_bad_signature() {
        // The checksum matches (e.g. a replaced SHA256SUMS) but the signature does not.
        let sha = hex(&Sha256::digest(b"tesT"));
        let (result, _, _) = run(b"tesT", &sha, 4, false);
        assert!(matches!(result, Err(UpdateError::Signature(_))));
    }

    #[test]
    fn rejects_oversized_and_cancelled() {
        let (result, _, _) = run(b"test", TEST_SHA, 3, false);
        assert!(matches!(result, Err(UpdateError::Checksum(_))));
        let (result, out, _) = run(b"test", TEST_SHA, 4, true);
        assert!(matches!(result, Err(UpdateError::Cancelled)));
        assert!(out.is_empty());
    }

    #[test]
    fn hex_digest() {
        assert_eq!(hex(&Sha256::digest(b"test")), TEST_SHA);
    }
}
