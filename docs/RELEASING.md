# Releasing Cardfile

Releases are built by GitHub Actions from a version tag. [`dist`](https://github.com/axodotdev/cargo-dist)
generates `.github/workflows/release.yml`. Don't edit that file by hand: change
`dist-workspace.toml` and run `dist generate`.

## What a release contains

| File | Produced by | Notes |
|------|-------------|-------|
| `cardfile-<target>.zip` / `.tar.xz` | dist | Portable archives for Windows and Linux (x64, ARM64) |
| `install.ps1` / `install.sh` (repo root, not a release asset) | maintainers | One-line installers served from `main`; they always pick the newest release, so they need no change per release. They rely on the archive names `cardfile-<target>.zip` / `.tar.xz`, `SHA256SUMS` and the minisign trusted comment `Cardfile <tag> release file: <name>` |
| `Cardfile-<version>-setup-<arch>.exe` | `package.yml` | Inno Setup installer, also used by winget |
| `Cardfile-<version>-<arch>.msix` | `package.yml` | For Microsoft Store submission (the Store signs it) |
| `.deb`, `.rpm`, `.AppImage` | `package.yml` | Linux packages for x86_64 and aarch64 |
| `SHA256SUMS`, `*.minisig` | `sign.yml` | Checksums and minisign signatures of every file above, attached together with them when the release is created |

### In-app updates depend on these files

The self-updater (`crates/cardfile-updater`) picks the asset by name, so keep these names
stable: `cardfile-<arch>-pc-windows-msvc.zip`, `cardfile-<arch>-unknown-linux-gnu.tar.xz`,
`Cardfile-<version>-<x86_64|aarch64>.AppImage` and `Cardfile-<version>-setup-<x64|arm64>.exe`.
It installs a file only if its SHA-256 is in the signed `SHA256SUMS` **and** its own
`<file>.minisig` verifies with `PUBLIC_KEY` and carries the trusted comment
`Cardfile <tag> release file: <file>` (written by `sign.yml`). A release without signatures is
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

### Pinned third-party code

Actions are pinned to full commit SHAs (with the tag in a comment). For `release.yml` the pins
live in `github-action-commits` in `dist-workspace.toml`; regenerate with `dist generate`.
`package.yml` downloads a fixed `linuxdeploy` release and checks its SHA-256, and
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
  `PUBLIC_KEY` in `crates/cardfile-updater/src/lib.rs` (the app trusts only that key).
- Current key: `RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w` (ID `1822D9EE1D935A84`),
  kept outside the repository by the maintainer.
- Keep `minisign.key` offline and never commit it (`*.key` is ignored by git).

### 2. winget

1. Fork `microsoft/winget-pkgs` with the account that will submit.
2. Create a classic personal access token with the `public_repo` scope and save it as the secret `WINGET_TOKEN`.
3. The **first** version must be submitted manually:
   ```powershell
   winget install wingetcreate
   wingetcreate new https://github.com/FittyAr/Cardfile/releases/download/v2.0.0/Cardfile-2.0.0-setup-x64.exe
   ```
   Use the identifier `FittyAr.Cardfile`.
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
3. The first submission (store listing, screenshots, age rating) is done in Partner Center by hand. Upload the `.msix` from a release.
4. Later versions: run the **Microsoft Store** workflow manually with the release tag.

The MSIX build detects that it runs from `WindowsApps` and turns off Cardfile's own updater; the Store handles updates.

## Cutting a release

1. Update `version` in the root `Cargo.toml` (`workspace.package.version`) and the `<release>` entry in `packaging/linux/ar.fittyar.Cardfile.metainfo.xml`.
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
