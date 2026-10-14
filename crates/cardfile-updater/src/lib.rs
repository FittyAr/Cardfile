//! Checks GitHub Releases for a newer Cardfile version.
//!
//! Cardfile only *notifies* about updates; installing is left to the package
//! manager, Microsoft Store or the downloaded installer. A release is announced
//! only when its `SHA256SUMS` carries a valid minisign signature from
//! [`PUBLIC_KEY`], so a forged or tampered release is never advertised.

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;
use thiserror::Error;

pub const REPOSITORY: &str = "FittyAr/Cardfile";

/// Minisign public key that signs every release (see `docs/SIGNING.md`).
pub const PUBLIC_KEY: &str = "RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w";

/// The signed file of each release; it lists the SHA-256 of every other asset.
const CHECKSUMS: &str = "SHA256SUMS";

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Parse(String),
    #[error("release signature is invalid: {0}")]
    Signature(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Git tag, e.g. `v2.0.1`.
    pub tag: String,
    pub url: String,
    pub notes: String,
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
}

/// Pick the newest release above `current` from a GitHub releases JSON array.
/// Pre-releases are considered only when `current` is itself a pre-release.
pub fn newest_from_json(json: &str, current: &Version) -> Result<Option<Release>, UpdateError> {
    let releases: Vec<GhRelease> =
        serde_json::from_str(json).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let allow_pre = !current.pre.is_empty();
    Ok(releases
        .into_iter()
        .filter(|r| !r.draft && (allow_pre || !r.prerelease))
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.trim_start_matches('v')).ok()?;
            Some(Release {
                version,
                tag: r.tag_name,
                url: r.html_url,
                notes: r.body.unwrap_or_default(),
            })
        })
        .filter(|r| r.version > *current)
        .max_by(|a, b| a.version.cmp(&b.version)))
}

/// Trusted comment `sign.yml` writes into each signature. Naming the tag stops an
/// older release's signed `SHA256SUMS` from being replayed under a newer tag.
fn trusted_comment(tag: &str) -> String {
    format!("Cardfile {tag} release file: {CHECKSUMS}")
}

/// Check that `signature` (a `.minisig` file) signs `checksums` of release `tag`
/// with `public_key`.
pub fn verify_checksums(
    checksums: &[u8],
    signature: &str,
    tag: &str,
    public_key: &str,
) -> Result<(), UpdateError> {
    let bad = |e: &dyn std::fmt::Display| UpdateError::Signature(e.to_string());
    let key = PublicKey::from_base64(public_key).map_err(|e| bad(&e))?;
    let sig = Signature::decode(signature).map_err(|e| bad(&e))?;
    key.verify(checksums, &sig, false).map_err(|e| bad(&e))?;
    if sig.trusted_comment() != trusted_comment(tag) {
        return Err(bad(&"signature is for another release or file"));
    }
    Ok(())
}

fn get(url: &str, accept: &str) -> Result<Vec<u8>, UpdateError> {
    let mut resp = ureq::get(url)
        .header(
            "User-Agent",
            concat!("Cardfile/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", accept)
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    resp.body_mut()
        .with_config()
        .limit(4 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| UpdateError::Network(e.to_string()))
}

/// Query GitHub for a release newer than `current` and verify its signature.
/// Blocking: call off the UI thread.
pub fn check(current: &Version) -> Result<Option<Release>, UpdateError> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=20");
    let body = get(&url, "application/vnd.github+json")?;
    let body = String::from_utf8(body).map_err(|e| UpdateError::Parse(e.to_string()))?;
    let Some(release) = newest_from_json(&body, current)? else {
        return Ok(None);
    };
    let base = format!(
        "https://github.com/{REPOSITORY}/releases/download/{}",
        release.tag
    );
    let checksums = get(&format!("{base}/{CHECKSUMS}"), "application/octet-stream")?;
    let signature = get(
        &format!("{base}/{CHECKSUMS}.minisig"),
        "application/octet-stream",
    )?;
    let signature =
        String::from_utf8(signature).map_err(|e| UpdateError::Signature(e.to_string()))?;
    verify_checksums(&checksums, &signature, &release.tag, PUBLIC_KEY)?;
    Ok(Some(release))
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"[
        {"tag_name":"v2.1.0-beta.1","html_url":"b","prerelease":true},
        {"tag_name":"v2.0.1","html_url":"s","body":"fixes"},
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
    fn prerelease_users_see_prereleases() {
        let r = newest_from_json(JSON, &Version::parse("2.0.0-alpha.1").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(r.version, Version::parse("2.1.0-beta.1").unwrap());
    }

    // Test vector from the minisign-verify crate (MIT): `b"test"` signed by this key.
    const TEST_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const TEST_SIG: &str = "untrusted comment: signature from minisign secret key
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
            trusted_comment("v2.0.1"),
            "Cardfile v2.0.1 release file: SHA256SUMS"
        );
        assert!(PublicKey::from_base64(PUBLIC_KEY).is_ok());
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
