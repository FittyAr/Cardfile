# Acceptance evidence

Database integration tests are in `crates/cardfile-db/tests/recovery.rs`.

- Consistent WAL snapshot with accounts, password hashes, admin flags, current
  card content, revision history, encrypted payloads and attachment bytes/name:
  `snapshot_preserves_uncheckpointed_accounts_cards_and_attachment_bytes`.
- Pruning restricted to this collection's automatic snapshots; manual, safety,
  other collections and arbitrary files retained:
  `automatic_retention_keeps_manual_safety_and_other_collections`.
- Corrupt, foreign, future-schema and missing input rejection:
  `rejects_corrupt_foreign_future_and_missing_backups`.
- Restore freezes existing handles, retains a safety copy and unchanged source,
  applies selected accounts/passwords after restart:
  `restore_freezes_old_handles_and_restarts_with_the_selected_collection`.
- Recovery after old database rename and after replacement rename:
  `restart_recovers_after_either_restore_rename_boundary`.
- Invalid restore does not freeze or modify live collection:
  `rejected_restore_keeps_live_database_usable_and_creates_no_pending_job`.
- OS lock lifetime follows final cloned handle:
  `database_lock_is_held_until_the_last_cloned_handle_is_dropped`.

Assertion review: tests check both primary results and secondary file/state
effects. Tests operate only in temporary directories and use real SQLite with
migrations. No driver/system or network interactions. Byte preservation and
replacement/rejection assertions would fail under stale-copy, broad-prune,
missing-freeze, missing-lock or partial-recovery mutations.

Initial validation: `cargo test --workspace --offline` passed on 2026-11-04, including
39 database unit tests, 10 recovery integration tests and 25 UI unit tests.
The final run used approved local-socket access for existing mock HTTP tests;
the first sandboxed attempt could not access those sockets (Windows 10013).
No real app or installer was launched. `cargo fmt --all --check`, workspace
clippy with `-D warnings`, and bootstrap contract tests also passed.

Additional recovery tests:
- `corrupted_staged_restore_rolls_back_to_the_mandatory_safety_copy`
- `uncommitted_restore_staging_is_archived_without_replacing_live_data`
- `restores_supported_older_schema_without_migrating_the_selected_source`

Additional evidence:
- Local preferences: `dimensions_fit_small_displays_and_reject_invalid_sizes`,
  `unknown_version_falls_back_without_reusing_backup_path`,
  `daily_schedule_waits_a_full_day_and_never_runs_when_disabled`, and
  `repeated_atomic_save_preserves_local_destination_and_identity`.
- Portable: `portable_copy_preserves_collection_and_disables_services_without_touching_source`
  and `portable_copy_rejects_existing_and_nested_destinations`.
- Bootstrap: `pwsh -NoProfile -File scripts/test-install-bootstrap.ps1` passed;
  downloads, installer launches and legacy cleanup are mocked.
- Locale check passed: 479 used keys, none missing.
- `cargo deny check`: advisories, bans, licenses and sources all pass after the
  dependency refresh. No advisory waiver was added.
- Update-channel tests passed: `stable_build_can_opt_in_and_beta_build_can_opt_out`,
  `latest_accepts_promoted_beta_and_never_downgrades_after_switching`,
  `prerelease_channel_still_requires_signature_files_and_ignores_drafts`, and
  `update_channel_persists_and_invalid_values_preserve_other_preferences`.
- Direct/compatible transitive dependencies refreshed; GPUI 0.9.0 and GPUI tests
  0.3.11 stay aligned. libsqlite3-sys 0.38.2 is the latest Diesel-supported version.
- Actions pins verified against current release tags. cargo-dist 0.33.0 generates
  a consistent workflow; upstream 0.34.0 emits invalid custom-job references.
- `dist plan --tag v2.0.0-beta.3` confirms announcement_is_prerelease=true.

Safety boundary: all runtime tests use temporary directories. Do not launch
Cardfile or run the real installer against this computer's existing collection.

Native GPUI sizing, installers, physical USB, DPI and cross-device OneDrive
behavior remain manual acceptance items for the root implementation workflow.

Manual acceptance must use a disposable `--data-dir` or a new portable folder,
never the maintainer's current installation:
1. Resize/maximize/minimize, close and reopen; repeat with smaller display/DPI.
2. Compile setup for x64/ARM64; exercise installed/current-user, all-users,
   portable, silent `/MODE`/`/DIR`, and an existing conflicting destination.
3. Move a disposable portable folder between drives, open after removing its old
   drive, and validate copied encrypted cards/attachments with their passwords.
4. Disconnect the selected backup destination and verify notification/retry.
5. Restore in a disposable collection; authenticate with restored accounts and
   confirm original collection files and local preferences are retained.
6. OneDrive acceptance is sequential only: close, complete sync, open elsewhere.

The maintainer subsequently requested one integration of all work into main and a
new prerelease, superseding the original four-PR delivery grouping. Open the integration
PR only after all local checks pass; wait for Windows/Linux CI and installer compilation
before merging. Tag beta.3 on the verified main commit and wait for signed assets.
Cardscape remains unavailable in this session.


## beta.3 integration verification

The updater channel selection and dependency refresh are part of the same integration.
Local workspace clippy passed with GPUI 0.9.0; deny passed across all categories; fmt,
locale keys and bootstrap contract tests passed. The full updated workspace test run
passed on 2026-11-04 before committing (including all new recovery/channel tests). The CI Windows installer job will compile x64/ARM64
without running the generated installers; field acceptance remains for the maintainer.

Dependency audit: all declared dependencies are current except libsqlite3-sys, which
is constrained by Diesel (<0.39). All compatible locked versions were refreshed.
Minisign and linuxdeploy already match their latest published release; dictionary pins
already match upstream. cargo-dist 0.33.0 is the latest usable generator for custom jobs
because 0.34.0 produces invalid workflow references. No security-check waiver is used.


Windows installer smoke compilation passed locally for both x64 and ARM64 with
verified portable Inno Setup 7.1.1. `scripts/compile-windows-installer.ps1` now provides
the same pinned compiler to CI and release packaging. Only the compiler bootstrap was
executed in its documented portable mode; generated Cardfile installers and payloads
were never executed. The current Cardfile installation was not accessed.

## beta.4 translation and settings correction

Root cause: the long channel description shared a horizontal row with the dropdown.
The text column had no minimum-width override, so its intrinsic width displaced the
control. The selector now has a separate row, hints wrap, and shared settings rows
allow labels to shrink while preserving control width.

All 20 non-English locales now include the 51 missing keys. The full locale check
validates 525 messages per language and interpolation variables; CI runs it with
`--all`. New installer messages are translated in Spanish, Brazilian Portuguese,
French and German, with English fallback. x64 and ARM64 installer smoke compilation
passed with a disposable payload; neither generated installer was executed.

Native visual acceptance remains pending: the desktop automation tool's app-access
approval timed out before it could capture the fixture window. The fixture used
`--data-dir D:/GitHub/Cardfile/target/channel-visual-fixture` with seeded demo data
and a 760-pixel window. Only that explicitly identified process was closed afterward.
The maintainer's collection was never opened or modified.

Final beta.4 local validation: fmt, workspace clippy with `-D warnings`, full
workspace tests and deny passed. HTTP mock tests required execution outside the
sandbox because it blocks loopback sockets (10013); the same tests then passed.
Locale coverage/variables and duplicate-key checks passed. cargo-dist plan selects
`v2.0.0-beta.4` as a prerelease. Native visual acceptance is still outstanding.
