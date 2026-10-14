//! Checks GitHub Releases for a newer Cardfile version.
//!
//! Cardfile only *notifies* about updates; installing is left to the package
//! manager, Microsoft Store or the downloaded installer (whose minisign
//! signatures are published next to each release asset).

use semver::Version;
use serde::Deserialize;
use thiserror::Error;

pub const REPOSITORY: &str = "FittyAr/Cardfile";

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("network error: {0}")]
    Network(String),
    #[error("unexpected response: {0}")]
    Parse(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
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
                url: r.html_url,
                notes: r.body.unwrap_or_default(),
            })
        })
        .filter(|r| r.version > *current)
        .max_by(|a, b| a.version.cmp(&b.version)))
}

/// Query GitHub for a release newer than `current`. Blocking: call off the UI thread.
pub fn check(current: &Version) -> Result<Option<Release>, UpdateError> {
    let url = format!("https://api.github.com/repos/{REPOSITORY}/releases?per_page=20");
    let mut resp = ureq::get(&url)
        .header(
            "User-Agent",
            concat!("Cardfile/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| UpdateError::Network(e.to_string()))?;
    newest_from_json(&body, current)
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

    #[test]
    fn up_to_date() {
        assert!(
            newest_from_json(JSON, &Version::parse("2.1.0").unwrap())
                .unwrap()
                .is_none()
        );
    }
}
