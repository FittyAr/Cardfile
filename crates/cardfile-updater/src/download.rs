//! Downloading and verifying a release asset.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use minisign_verify::{PublicKey, Signature, StreamVerifier};
use sha2::{Digest, Sha256};
use tempfile::TempPath;

use crate::kind::{Platform, TEMP_PREFIX, select_asset};
use crate::{
    Asset, DOWNLOAD_BODY_TIMEOUT, InstallKind, PUBLIC_KEY, Release, UpdateError, agent,
    check_trusted_comment, fetch_signed_checksums, get_text, signature_error, user_agent,
};

/// Hard cap for an asset whose size GitHub did not report.
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;
/// A download that receives nothing for this long is abandoned as stalled.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
/// How often a waiting download looks at the cancel flag.
const CANCEL_POLL: Duration = Duration::from_millis(200);

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
    let resp = agent()
        .get(&asset.url)
        .config()
        .timeout_recv_body(Some(DOWNLOAD_BODY_TIMEOUT))
        .build()
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
    let mut reader = StallGuard::spawn(resp.into_body().into_reader(), cancel, STALL_TIMEOUT)?;
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

/// Reads a blocking network stream on a helper thread, so the download can give up
/// on a stalled connection (no data for `stall` -> [`std::io::ErrorKind::TimedOut`])
/// and react to the cancel flag within [`CANCEL_POLL`] (reported as
/// [`std::io::ErrorKind::Interrupted`]; [`receive`] then sees the flag and stops).
///
/// When the download is abandoned, the helper thread is left to finish its current
/// read; the agent's body timeout bounds how long that can take.
pub(crate) struct StallGuard<'a> {
    rx: Receiver<std::io::Result<Vec<u8>>>,
    pending: Vec<u8>,
    pos: usize,
    done: bool,
    cancel: &'a AtomicBool,
    stall: Duration,
}

impl<'a> StallGuard<'a> {
    pub(crate) fn spawn(
        mut inner: impl Read + Send + 'static,
        cancel: &'a AtomicBool,
        stall: Duration,
    ) -> std::io::Result<Self> {
        // A small bound keeps memory flat when the disk is slower than the network.
        let (tx, rx) = mpsc::sync_channel(4);
        std::thread::Builder::new()
            .name("cardfile-update-download".into())
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let msg = match inner.read(&mut buf) {
                        Ok(n) => Ok(buf[..n].to_vec()),
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => Err(e),
                    };
                    let last = !matches!(&msg, Ok(chunk) if !chunk.is_empty());
                    // A closed channel means the download was abandoned.
                    if tx.send(msg).is_err() || last {
                        break;
                    }
                }
            })?;
        Ok(Self {
            rx,
            pending: Vec::new(),
            pos: 0,
            done: false,
            cancel,
            stall,
        })
    }
}

impl Read for StallGuard<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Error, ErrorKind};
        if self.pos < self.pending.len() {
            let n = out.len().min(self.pending.len() - self.pos);
            out[..n].copy_from_slice(&self.pending[self.pos..self.pos + n]);
            self.pos += n;
            return Ok(n);
        }
        if self.done {
            return Ok(0);
        }
        let started = Instant::now();
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(Error::new(ErrorKind::Interrupted, "download cancelled"));
            }
            let waited = started.elapsed();
            if waited >= self.stall {
                return Err(Error::new(
                    ErrorKind::TimedOut,
                    format!("no data received for {} s", self.stall.as_secs()),
                ));
            }
            match self.rx.recv_timeout(CANCEL_POLL.min(self.stall - waited)) {
                Ok(Ok(chunk)) if chunk.is_empty() => {
                    self.done = true;
                    return Ok(0);
                }
                Ok(Ok(chunk)) => {
                    self.pending = chunk;
                    self.pos = 0;
                    return self.read(out);
                }
                Ok(Err(e)) => {
                    self.done = true;
                    return Err(e);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.done = true;
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        "download reader stopped",
                    ));
                }
            }
        }
    }
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
    use std::io::Cursor;

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

    /// A reader that never returns, like a server that stopped sending.
    struct Stalled;

    impl Read for Stalled {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            std::thread::sleep(Duration::from_secs(3600));
            Ok(0)
        }
    }

    #[test]
    fn stall_guard_passes_data_through() {
        let cancel = AtomicBool::new(false);
        let data: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
        let mut guard =
            StallGuard::spawn(Cursor::new(data.clone()), &cancel, Duration::from_secs(5)).unwrap();
        let mut out = Vec::new();
        guard.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn stalled_download_times_out() {
        let cancel = AtomicBool::new(false);
        let mut guard = StallGuard::spawn(Stalled, &cancel, Duration::from_millis(300)).unwrap();
        let started = Instant::now();
        let err = guard.read(&mut [0u8; 16]).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancel_stops_a_stalled_download() {
        let cancel = AtomicBool::new(false);
        let guard = StallGuard::spawn(Stalled, &cancel, Duration::from_secs(3600)).unwrap();
        let key = PublicKey::from_base64(TEST_KEY).unwrap();
        let sig = Signature::decode(TEST_SIG).unwrap();
        let started = Instant::now();
        let result = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::Relaxed);
            });
            let mut guard = guard;
            receive(
                &mut guard,
                &mut Vec::new(),
                Expect {
                    sha256: TEST_SHA,
                    verifier: key.verify_stream(&sig).unwrap(),
                    total: None,
                    limit: 4,
                },
                &mut |_, _| {},
                &cancel,
            )
        });
        assert!(matches!(result, Err(UpdateError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn hex_digest() {
        assert_eq!(hex(&Sha256::digest(b"test")), TEST_SHA);
    }
}
