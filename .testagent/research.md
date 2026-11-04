# Acceptance inventory

Rust workspace; unit tests live next to implementations and SQLite fixtures use
migrations and temporary directories. Scope: local preferences/window sizing,
consistent backups and retention, restore recovery and process exclusion,
portable publication, Windows installer/bootstrap contracts.

Acceptance: preserve WAL writes, reject corrupt/foreign/future databases, never
overwrite destinations, retain unrelated/manual backups, recover interrupted
restore, preserve existing data and local preferences. GPUI/native installer,
physical USB, DPI and cross-device OneDrive behavior require manual acceptance.

Update-channel addition: existing updater filters prereleases based on the
running semver; GitHub's current Latest is a promoted beta. Use an explicit
per-user Latest/Prerelease preference, the Latest endpoint for that designation,
and retain signatures/no-downgrade protection. Release beta.3 as prerelease only.
