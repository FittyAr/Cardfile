//! Manual end-to-end check of the updater against the real GitHub releases:
//! `cargo run -p cardfile-updater --example self_update -- 2.0.0-alpha.1`
//! pretends to be the given version, downloads the newest release for this
//! platform, verifies it and installs it over this example binary.

use std::sync::atomic::AtomicBool;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pretend = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "2.0.0-alpha.1".into());
    let current = semver::Version::parse(&pretend)?;
    let Some(release) = cardfile_updater::check(&current)? else {
        println!("no newer release than {current}");
        return Ok(());
    };
    let kind = cardfile_updater::InstallKind::detect();
    println!(
        "found {} ({} assets); install kind: {kind:?}",
        release.version,
        release.assets.len()
    );
    let cancel = AtomicBool::new(false);
    let mut last = 0;
    let downloaded = cardfile_updater::download(
        &release,
        &kind,
        |done, total| {
            if done - last > 2_000_000 {
                last = done;
                println!("  {done} / {total:?}");
            }
        },
        &cancel,
    )?;
    println!("verified {}", downloaded.asset().name);
    let outcome = cardfile_updater::install(downloaded)?;
    println!("installed: {outcome:?}");
    Ok(())
}
