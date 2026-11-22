# Releasing NoteChest

Releases are built by GitHub Actions from a version tag. [`dist`](https://github.com/axodotdev/cargo-dist)
generates `.github/workflows/release.yml`, which is then hardened by hand (see
[Regenerating release.yml](#regenerating-releaseyml)). Change `dist-workspace.toml` first and
regenerate; edit the file directly only for the hardening below.

## What a release contains

| File | Produced by | Notes |
|------|-------------|-------|
| `notechest-<target>.zip` / `.tar.xz` | dist | Portable archives for Windows and Linux (x64, ARM64) |
| `install.ps1` / `install.sh` (repo root, not a release asset) | maintainers | One-line installers served from `main`; they always pick the newest release, so they need no change per release. PowerShell uses `NoteChest-<version>-setup-<arch>.exe`; shell uses `notechest-<target>.tar.xz`. They rely on `SHA256SUMS` and the minisign trusted comment `NoteChest <tag> release file: <name>` |
| `NoteChest-<version>-setup-<arch>.exe` | `package.yml` | Inno Setup installer, also used by winget |
| `NoteChest-<version>-<arch>.msix` | `package.yml` | For Microsoft Store submission (the Store signs it) |
| `.deb`, `.rpm`, `.AppImage` | `package.yml` | Linux packages for x86_64 and aarch64 |
| `SHA256SUMS`, `*.minisig` | `sign.yml` | Checksums and minisign signatures of every file above, attached together with them when the release is created |

### In-app updates depend on these files

The self-updater (`crates/notechest-updater`) picks the asset by name, so keep these names
stable: `notechest-<arch>-pc-windows-msvc.zip`, `notechest-<arch>-unknown-linux-gnu.tar.xz`,
`NoteChest-<version>-<x86_64|aarch64>.AppImage` and `NoteChest-<version>-setup-<x64|arm64>.exe`.
It installs a file only if its SHA-256 is in the signed `SHA256SUMS` **and** its own
`<file>.minisig` verifies with `PUBLIC_KEY` and carries the trusted comment
`NoteChest <tag> release file: <file>` (written by `sign.yml`). A release without signatures is
never offered: the updater skips it and offers the newest *signed* release instead (or nothing).

### Order of the release jobs

`dist-workspace.toml` sets `github-release = "announce"`, so the GitHub Release is created only
in the final `announce` job, after every build, package and publish job:

1. `plan`, builds, `package.yml` (native installers).
2. `sign.yml` (`publish-jobs`): checksums and minisign signatures, handed over as the
   `artifacts-sign` workflow artifact. It **fails** when `MINISIGN_SECRET_KEY` or
   `MINISIGN_PUBLIC_KEY` is missing or a signature does not verify.
3. `announce`: creates the release with every file **and** its signatures in one step. If
   signing failed, no release is published at all.
4. `winget.yml` (`post-announce-jobs`): needs the published release; skips pre-releases.

### Workflow permissions and secrets

Every workflow sets a read-only default token (`permissions: contents: read`); `pull_request`
runs of `release.yml` (which only run `dist plan`) therefore never get a writable token. Only
the `announce` job of `release.yml` has `contents: write`, because it creates the GitHub
Release. Reusable workflows get only the secrets they use, declared under
`on.workflow_call.secrets`: `sign.yml` receives `MINISIGN_SECRET_KEY`, `winget.yml` receives
`WINGET_TOKEN` and `package.yml` receives none (`vars` still reach it).

### Regenerating release.yml

dist always writes a workflow-level `contents: write` and `secrets: inherit` for custom jobs;
it has no option to change either (`github-custom-job-permissions` only covers the called
jobs). So `release.yml` is edited by hand after generation, and `dist-workspace.toml` sets
`allow-dirty = ["ci"]` so that `dist plan` (run in the `plan` job) does not reject the edited
file and `dist generate` does not overwrite it. The flip side: dist no longer keeps
`release.yml` in sync on its own. After changing CI-related dist settings (version, targets,
`github-action-commits`, custom jobs):

1. Comment out `allow-dirty = ["ci"]` and run `dist generate` with the `cargo-dist-version`
   from `dist-workspace.toml` (with `allow-dirty` set, dist skips CI generation and
   `dist generate --mode ci` refuses to run). Restore `allow-dirty` afterwards.
2. Re-apply the hand edits, which are marked with a comment above `permissions:`:
   - workflow-level `permissions: "contents": "read"`;
   - `permissions: "contents": "write"` on the `announce` job;
   - no `secrets:` on `custom-package`;
   - `secrets: MINISIGN_SECRET_KEY: ${{ secrets.MINISIGN_SECRET_KEY }}` on `custom-sign`;
   - `secrets: WINGET_TOKEN: ${{ secrets.WINGET_TOKEN }}` on `custom-winget`.
3. Review the diff: anything else that changed came from dist.

A new secret for `sign.yml` or `winget.yml` must be declared in that workflow's
`on.workflow_call.secrets` and passed explicitly from `release.yml`.

### Pinned third-party code

Actions are pinned to full commit SHAs (with the tag in a comment). For `release.yml` the pins
live in `github-action-commits` in `dist-workspace.toml`; regenerate with `dist generate`.
`package.yml` installs exact versions of `cargo-deb` and `cargo-generate-rpm`, `ci.yml` pins
PyYAML, and `package.yml` downloads a fixed `linuxdeploy` release and checks its SHA-256, and
`scripts/fetch-dictionaries.sh` fetches the dictionaries from a fixed commit and checks their
SHA-256. Update the hashes together with the pins. The Inno Setup installer must keep the `/RELAUNCH` `[Run]` entry in
`installer.iss`: the app quits and the silent installer starts it again.

## One-time setup

### 1. Minisign key (release signatures)

```bash
minisign -G -W -p minisign.pub -s minisign.key
```

- `-W` creates a key without a password. The key is protected by GitHub's secret storage instead.
- Add the **contents** of `minisign.key` as the repository secret `MINISIGN_SECRET_KEY`.
- Add the public key (the second line of `minisign.pub`) as the repository **variable**
  `MINISIGN_PUBLIC_KEY` (CI checks every signature against it), in [SIGNING.md](SIGNING.md) and as
  `PUBLIC_KEY` in `crates/notechest-updater/src/lib.rs` (the app trusts only that key).
- Current key: `RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w` (ID `1822D9EE1D935A84`),
  kept outside the repository by the maintainer.
- Keep `minisign.key` offline and never commit it (`*.key` is ignored by git).

### 2. winget

1. Fork `microsoft/winget-pkgs` with the account that will submit.
2. Create a classic personal access token with the `public_repo` scope and save it as the secret `WINGET_TOKEN`.
3. The **first** version must be submitted manually:
   ```powershell
   winget install wingetcreate
   wingetcreate new https://github.com/FittyAr/NoteChest/releases/download/v2.0.0/NoteChest-2.0.0-setup-x64.exe
   ```
   Use the identifier `FittyAr.NoteChest`.
4. After that, `.github/workflows/winget.yml` (called by `release.yml` after the release is published) opens a pull request for every stable release. Pre-releases are skipped.

### 3. Microsoft Store

1. In Partner Center, reserve the app name and open **Product identity**. Save these as repository **variables**:
   - `MSIX_IDENTITY_NAME`: Package/Identity/Name
   - `MSIX_PUBLISHER`: Package/Identity/Publisher, e.g. `CN=…`
   - `MSIX_PUBLISHER_DISPLAY_NAME`
   - `STORE_PRODUCT_ID`: the Store ID
2. Associate an Azure AD application (Account settings → User management → Azure AD applications). Save these as **secrets**:
   - `PARTNER_CENTER_TENANT_ID`
   - `PARTNER_CENTER_CLIENT_ID`
   - `PARTNER_CENTER_CLIENT_SECRET`
   - `PARTNER_CENTER_SELLER_ID`
3. The first submission (store listing, screenshots, age rating) is done in Partner Center by hand. Upload the `.msix` from a release. The submission needs:
   - **Privacy policy URL**: `https://github.com/FittyAr/NoteChest/blob/main/docs/PRIVACY.md`
     (required because the app uses the network). Keep the policy in sync with what the app sends.
   - **Live generative AI** (Store policy 11.16): tick it in the submission and mention the
     assistant in the description. Users report answers with **Report** in the assistant panel,
     which opens a GitHub issue labelled `ai-content` (create that label once).
   - **Restricted capabilities**: the manifest declares `runFullTrust` and
     `unvirtualizedResources`. Justification for the second one: *"NoteChest stores the user's
     notes in a SQLite database under %APPDATA%\FittyAr\NoteChest. With AppData write
     virtualization, the database's WAL, lock file and backups would be redirected to a
     per-package folder that Windows deletes on uninstall, losing notes, and a copy installed
     outside the Store would open the same database with a different WAL and lock, which can
     corrupt it. Disabling write virtualization keeps the notes in one real location that
     survives uninstalling or reinstalling the app."*
4. Later versions: run the **Microsoft Store** workflow manually with the release tag.

Before the first submission, install the release `.msix` on a machine that already has beta data
(sideload with Developer Mode on, `Add-AppxPackage`) and check that the notes appear and that
new files (`notechest.db-wal`, `backups\`) are created in the real `%APPDATA%\FittyAr\NoteChest`,
not under `%LOCALAPPDATA%\Packages\…`. An unsigned package must first be signed with a test
certificate for sideloading.

The MSIX build detects that it runs from `WindowsApps` and turns off NoteChest's own updater; the Store handles updates.

## Cutting a release

1. Update `version` in the root `Cargo.toml` (`workspace.package.version`) and the `<release>` entry in `packaging/linux/ar.fittyar.NoteChest.metainfo.xml`.
   Write pre-releases there with a tilde (`2.0.0~beta.8`, not `2.0.0-beta.8`): AppStream sorts the
   hyphen form after the stable version, and the AppImage build fails its validation with
   `releases-not-in-order`.
2. Update `CHANGELOG.md`:
   ```bash
   git cliff --tag v2.0.0 -o CHANGELOG.md
   ```
3. Merge to `main` through a PR. The title should be `chore(release): v2.0.0`.
4. Tag and push:
   ```bash
   git tag v2.0.0
   git push origin v2.0.0
   ```
5. Wait for **Release** to finish, then check the GitHub release page.
6. Stable releases run **winget** automatically (a `post-announce-jobs` step of `release.yml`; it can also be run by hand with a tag). Run **Microsoft Store** manually.

Tags with a pre-release suffix (`v2.0.0-alpha.1`) are published as GitHub pre-releases.

## Linux repositories (planned)

Flathub and AUR are planned. The AppStream metadata in `packaging/linux` is ready for them.

Windows setup supports `/MODE=installed|portable` and `/DIR`. Portable updates continue using ZIP archives. Validate both x64 and ARM64 setup modes before publishing.


## beta.3 validation and dependency refresh

The maintainer requested beta.3 as a **prerelease**; do not mark it Latest automatically.
After testing, the maintainer can promote it in GitHub. The updater's Latest channel
uses GitHub's designation, independently of beta suffixes in tags.

All direct requirements and compatible locked dependencies were refreshed on 2026-11-04.
Keep libsqlite3-sys 0.38.2 until Diesel supports 0.39. cargo-dist is updated to 0.33.0:
[the 0.34.0 template](https://github.com/axodotdev/cargo-dist/blob/v0.34.0/cargo-dist/templates/ci/github/release.yml.j2)
emits invalid `uses: $/.github/workflows/...` for custom jobs. Do not hand-edit generated
release.yml or disable its consistency checks to work around that generator regression.
CI compiles both Windows installer architectures with a disposable payload and runs the
PowerShell bootstrap with mocked downloads, launches and legacy cleanup.


Windows compilation uses `scripts/compile-windows-installer.ps1`, which downloads the
official Inno Setup 7.1.1 bootstrap pinned by SHA-256 and extracts it in portable mode
under the build's temporary tooling folder. It creates no compiler uninstaller, PATH
entry or application registration. CI and release jobs use this same compiler entrypoint.
Local x64 and ARM64 smoke compilations succeeded with a disposable, never-run payload.
