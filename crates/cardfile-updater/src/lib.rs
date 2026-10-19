//! Checks GitHub Releases for a newer Cardfile version and updates the app in place.
//!
//! # Trust
//!
//! A release is announced only when its `SHA256SUMS` carries a valid minisign signature
//! from [`PUBLIC_KEY`], so a forged or tampered release is never advertised. Before
//! anything is installed, the downloaded file must match **both** its SHA-256 in that
//! signed `SHA256SUMS` and its own `.minisig`; any mismatch aborts the update.
//!
//! # Flow
//!
//! Every function here blocks; the UI calls them on a background thread.
//!
//! 1. [`cleanup_after_update`] at startup removes what a previous update left behind.
//! 2. [`check`] finds a newer, signed [`Release`].
//! 3. [`InstallKind::detect`] tells how this copy was installed. Store, Linux package
//!    manager, Flatpak, Snap and read-only copies are not self-updatable
//!    ([`InstallKind::is_self_updatable`]): keep notifying and open [`Release::url`].
//! 4. [`download`] fetches and verifies the right asset ([`select_asset`]), reporting
//!    progress and honouring a cancel flag.
//! 5. [`install`] replaces the files (archive or AppImage) or starts the Inno Setup
//!    installer, and returns an [`InstallOutcome`]: either call [`restart`] and quit, or
//!    quit right away so the installer can finish and relaunch Cardfile.

mod archive;
mod download;
mod install;
mod kind;

use std::sync::OnceLock;
use std::time::Duration;

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;
use thiserror::Error;

pub use download::{Downloaded, download, parse_checksums};
pub use install::{InstallOutcome, cleanup_after_update, install, restart};
pub use kind::{Arch, InstallKind, Os, Platform, select_asset};

/// GitHub repository (`owner/name`) that publishes Cardfile releases.
pub const REPOSITORY: &str = "FittyAr/Cardfile";

/// Minisign public key that signs every release (see `docs/SIGNING.md`).
pub const PUBLIC_KEY: &str = "RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w";

/// The signed file of each release; it lists the SHA-256 of every other asset.
const CHECKSUMS: &str = "SHA256SUMS";

/// Why checking for, downloading or installing an update failed.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// The HTTP request failed.
    #[error("network error: {0}")]
    Network(String),
    /// GitHub answered with something we could not understand.
    #[error("unexpected response: {0}")]
    Parse(String),
    /// A minisign signature is missing, malformed or does not match.
    #[error("release signature is invalid: {0}")]
    Signature(String),
    /// The downloaded file does not match the signed `SHA256SUMS`.
    #[error("checksum mismatch: {0}")]
    Checksum(String),
    /// The release archive is malformed or lacks the executable.
    #[error("invalid release archive: {0}")]
    Archive(String),
    /// This installation cannot update itself, or the release has no asset for it.
    #[error("cannot update this installation: {0}")]
    Unsupported(String),
    /// The download was cancelled through the cancel flag.
    #[error("update cancelled")]
    Cancelled,
    /// Reading or writing local files failed.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// A file attached to a GitHub release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Asset {
    /// File name, e.g. `cardfile-x86_64-pc-windows-msvc.zip`.
    pub name: String,
    /// Direct download URL.
    #[serde(rename = "browser_download_url")]
    pub url: String,
    /// Size in bytes as reported by GitHub (0 if unknown).
    #[serde(default)]
    pub size: u64,
}

/// A newer Cardfile release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Release version parsed from the tag.
    pub version: Version,
    /// Git tag, e.g. `v2.0.1`.
    pub tag: String,
    /// Release page on GitHub (for installs that cannot update themselves).
    pub url: String,
    /// Release notes (Markdown).
    pub notes: String,
    /// Files attached to the release.
    pub assets: Vec<Asset>,
}

impl Release {
    /// Whether the signed checksum list and its signature are attached.
    pub fn has_signature_files(&self) -> bool {
        self.asset(CHECKSUMS).is_some() && self.asset(&format!("{CHECKSUMS}.minisig")).is_some()
    }

    /// The attached file called `name`, if any.
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// Download URL of the file `name`: the asset's own URL, or GitHub's standard
    /// download path for this tag when the asset list does not mention it.
    pub fn download_url(&self, name: &str) -> String {
        match self.asset(name) {
            Some(asset) => asset.url.clone(),
            None => format!(
                "https://github.com/{REPOSITORY}/releases/download/{}/{name}",
                self.tag
            ),
        }
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

/// Every release above `current` in a GitHub releases JSON array, newest first.
/// Pre-releases are considered only when `current` is itself a pre-release.
fn candidates_from_json(json: &str, current: &Version) -> Result<Vec<Release>, UpdateError> {
    let releases: Vec<GhRelease> =
        serde_json::from_str(json).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let allow_pre = !current.pre.is_empty();
    let mut candidates: Vec<Release> = releases
        .into_iter()
        .filter(|r| !r.draft && (allow_pre || !r.prerelease))
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.trim_start_matches('v')).ok()?;
            Some(Release {
                version,
                tag: r.tag_name,
                url: r.html_url,
                notes: r.body.unwrap_or_default(),
                assets: r.assets,
            })
        })
        .filter(|r| r.version > *current)
        .collect();
    candidates.sort_by(|a, b| b.version.cmp(&a.version));
    Ok(candidates)
}

/// Pick the newest release above `current` from a GitHub releases JSON array.
/// Pre-releases are considered only when `current` is itself a pre-release.
pub fn newest_from_json(json: &str, current: &Version) -> Result<Option<Release>, UpdateError> {
    Ok(candidates_from_json(json, current)?.into_iter().next())
}

/// Pick the newest release above `current` that carries its signature files
/// (`SHA256SUMS` and `SHA256SUMS.minisig`). A release whose signatures are not attached
/// yet is skipped, so an older signed release can still be offered. Nothing is
/// verified here; [`check`] verifies the pick.
pub fn newest_signed_from_json(
    json: &str,
    current: &Version,
) -> Result<Option<Release>, UpdateError> {
    Ok(candidates_from_json(json, current)?
        .into_iter()
        .find(Release::has_signature_files))
}

/// Trusted comment `sign.yml` writes into the signature of `file`. Naming the tag
/// stops an older release's signed file from being replayed under a newer tag.
fn trusted_comment(tag: &str, file: &str) -> String {
    format!("Cardfile {tag} release file: {file}")
}

/// Fail unless `signature` was made for `file` of release `tag`.
fn check_trusted_comment(signature: &Signature, tag: &str, file: &str) -> Result<(), UpdateError> {
    if signature.trusted_comment() == trusted_comment(tag, file) {
        Ok(())
    } else {
        Err(UpdateError::Signature(
            "signature is for another release or file".into(),
        ))
    }
}

fn signature_error(e: impl std::fmt::Display) -> UpdateError {
    UpdateError::Signature(e.to_string())
}

/// Check that `signature` (a `.minisig` file) signs `checksums` of release `tag`
/// with `public_key`.
pub fn verify_checksums(
    checksums: &[u8],
    signature: &str,
    tag: &str,
    public_key: &str,
) -> Result<(), UpdateError> {
    let key = PublicKey::from_base64(public_key).map_err(signature_error)?;
    let sig = Signature::decode(signature).map_err(signature_error)?;
    key.verify(checksums, &sig, false)
        .map_err(signature_error)?;
    check_trusted_comment(&sig, tag, CHECKSUMS)
}

fn user_agent() -> &'static str {
    concat!("Cardfile/", env!("CARGO_PKG_VERSION"))
}

/// Time to resolve the host name and to connect (TCP + TLS).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Time from sending a request to receiving the response headers.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);
/// Whole-request limit for the small requests (API JSON, checksums, signatures).
const SMALL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Hard limit for receiving a release asset. Stalls are caught much sooner by the
/// download's own idle timeout; this only bounds the reader thread left behind.
pub(crate) const DOWNLOAD_BODY_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

/// The HTTP agent every request goes through. ureq has no timeouts by default, so a
/// silent server would block a check or a download forever.
pub(crate) fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_resolve(Some(CONNECT_TIMEOUT))
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_send_request(Some(RESPONSE_TIMEOUT))
            .timeout_recv_response(Some(RESPONSE_TIMEOUT))
            .build()
            .into()
    })
}

/// GET a small resource (API JSON, checksums, signatures) into memory.
fn get(url: &str, accept: &str) -> Result<Vec<u8>, UpdateError> {
    let mut resp = agent()
        .get(url)
        .config()
        .timeout_global(Some(SMALL_REQUEST_TIMEOUT))
        .build()
        .header("User-Agent", user_agent())
        .header("Accept", accept)
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    resp.body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| UpdateError::Network(e.to_string()))
}

/// GET a small text file (e.g. a `.minisig`).
fn get_text(url: &str) -> Result<String, UpdateError> {
    let body = get(url, "application/octet-stream")?;
    String::from_utf8(body).map_err(|e| UpdateError::Parse(e.to_string()))
}

/// Download `SHA256SUMS` of `release` and verify its signature. Returns its contents.
fn fetch_signed_checksums(release: &Release) -> Result<Vec<u8>, UpdateError> {
    let checksums = get(&release.download_url(CHECKSUMS), "application/octet-stream")?;
    let signature =
        get_text(&release.download_url(&format!("{CHECKSUMS}.minisig"))).map_err(|e| match e {
            UpdateError::Parse(e) => UpdateError::Signature(e),
            other => other,
        })?;
    verify_checksums(&checksums, &signature, &release.tag, PUBLIC_KEY)?;
    Ok(checksums)
}

/// Query GitHub for the newest signed release newer than `current` and verify its
/// signature. A release still waiting for its signature files is skipped (an older
/// signed one is offered instead, or nothing), so it is never reported as an error.
/// Blocking: call off the UI thread.
pub fn check(current: &Version) -> Result<Option<Release>, UpdateError> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=20");
    let body = get(&url, "application/vnd.github+json")?;
    let body = String::from_utf8(body).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let Some(release) = newest_signed_from_json(&body, current)? else {
        return Ok(None);
    };
    fetch_signed_checksums(&release)?;
    Ok(Some(release))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const JSON: &str = r#"[
        {"tag_name":"v2.1.0-beta.1","html_url":"b","prerelease":true},
        {"tag_name":"v2.0.1","html_url":"s","body":"fixes","assets":[
            {"name":"SHA256SUMS","browser_download_url":"https://x/SHA256SUMS","size":12},
            {"name":"cardfile-x86_64-pc-windows-msvc.zip","browser_download_url":"https://x/w.zip"}
        ]},
        {"tag_name":"v2.0.0","html_url":"o"},
        {"tag_name":"v3.0.0","html_url":"d","draft":true},
        {"tag_name":"nightly","html_url":"n"}
    ]"#;

    #[test]
    fn stable_users_skip_prereleases() {
        let r = newest_from_json(JSON, &Version::parse("2.0.0").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(r.version, Version::parse("2.0.1").unwrap());
        assert_eq!(r.notes, "fixes");
    }

    #[test]
    fn release_carries_assets() {
        let r = newest_from_json(JSON, &Version::parse("2.0.0").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(r.assets.len(), 2);
        assert_eq!(r.asset("SHA256SUMS").unwrap().size, 12);
        assert_eq!(r.download_url("SHA256SUMS"), "https://x/SHA256SUMS");
        assert_eq!(
            r.download_url("SHA256SUMS.minisig"),
            "https://github.com/FittyAr/Cardfile/releases/download/v2.0.1/SHA256SUMS.minisig"
        );
    }

    #[test]
    fn prerelease_users_see_prereleases() {
        let r = newest_from_json(JSON, &Version::parse("2.0.0-alpha.1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(r.version, Version::parse("2.1.0-beta.1").unwrap());
        assert!(r.assets.is_empty());
    }

    // Test vector from the minisign-verify crate (MIT): `b"test"` signed by this key.
    pub(crate) const TEST_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    pub(crate) const TEST_SIG: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335	file:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";

    #[test]
    fn signature_checks() {
        // Valid signature, but its trusted comment is not this release's.
        let err = verify_checksums(b"test", TEST_SIG, "v2.0.0", TEST_KEY).unwrap_err();
        assert!(err.to_string().contains("another release"));
        // Tampered content.
        assert!(verify_checksums(b"tesT", TEST_SIG, "v2.0.0", TEST_KEY).is_err());
        // Wrong key.
        assert!(verify_checksums(b"test", TEST_SIG, "v2.0.0", PUBLIC_KEY).is_err());
        assert_eq!(
            trusted_comment("v2.0.1", CHECKSUMS),
            "Cardfile v2.0.1 release file: SHA256SUMS"
        );
        assert!(PublicKey::from_base64(PUBLIC_KEY).is_ok());
    }

    #[test]
    fn trusted_comment_names_tag_and_file() {
        let sig = Signature::decode(TEST_SIG).unwrap();
        assert!(check_trusted_comment(&sig, "v2.0.0", "test").is_err());
        assert_eq!(
            trusted_comment("v2.0.1", "cardfile-x86_64-unknown-linux-gnu.tar.xz"),
            "Cardfile v2.0.1 release file: cardfile-x86_64-unknown-linux-gnu.tar.xz"
        );
    }

    #[test]
    fn unsigned_releases_are_skipped() {
        let json = r#"[
            {"tag_name":"v2.0.2","html_url":"new","assets":[
                {"name":"cardfile-x86_64-pc-windows-msvc.zip","browser_download_url":"https://x/w.zip"}
            ]},
            {"tag_name":"v2.0.1","html_url":"signed","assets":[
                {"name":"SHA256SUMS","browser_download_url":"https://x/SHA256SUMS"},
                {"name":"SHA256SUMS.minisig","browser_download_url":"https://x/SHA256SUMS.minisig"}
            ]},
            {"tag_name":"v2.0.3","html_url":"half","assets":[
                {"name":"SHA256SUMS","browser_download_url":"https://x/SHA256SUMS"}
            ]}
        ]"#;
        let v = |s| Version::parse(s).unwrap();
        // The newest overall is v2.0.3, but only v2.0.1 has both signature files.
        let newest = newest_from_json(json, &v("2.0.0")).unwrap().unwrap();
        assert_eq!(newest.tag, "v2.0.3");
        assert!(!newest.has_signature_files());
        let signed = newest_signed_from_json(json, &v("2.0.0")).unwrap().unwrap();
        assert_eq!(signed.tag, "v2.0.1");
        assert!(signed.has_signature_files());
        // Already on the newest signed one: the unsigned v2.0.2/v2.0.3 are no update.
        assert!(
            newest_signed_from_json(json, &v("2.0.1"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn agent_has_timeouts() {
        let timeouts = agent().config().timeouts();
        assert_eq!(timeouts.connect, Some(CONNECT_TIMEOUT));
        assert_eq!(timeouts.resolve, Some(CONNECT_TIMEOUT));
        assert_eq!(timeouts.recv_response, Some(RESPONSE_TIMEOUT));
    }

    #[test]
    fn up_to_date() {
        assert!(
            newest_from_json(JSON, &Version::parse("2.1.0").unwrap())
                .unwrap()
                .is_none()
        );
    }
}
