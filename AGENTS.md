# AGENTS.md

Guidance for AI coding agents (and humans) working on Cardfile.

**Source of truth:** [`docs/PLAN.md`](docs/PLAN.md). Read it before changing scope, architecture
or dependencies. If a change contradicts the plan, update the plan in the same PR.

## Project

Cardfile 2 is a desktop card-file / notes app written in Rust with a GPUI UI
([`gpui-kit`](https://crates.io/crates/gpui-kit): GPUI + gpui-component). Targets: Windows and
Linux. Data lives in SQLite through Diesel. Cards are edited in a WYSIWYG editor; users never
see Markdown.

## Workspace layout

| Crate | Purpose | May depend on |
|-------|---------|---------------|
| `cardfile-core` | Domain types, services, repository traits, errors. No UI, no DB. | — |
| `cardfile-doc` | Document model (block tree), plain-text extraction, exporters (Markdown, HTML, PDF, JSON). | core |
| `cardfile-crypto` | Argon2id hashing, card encryption (XChaCha20-Poly1305), key handling. | core |
| `cardfile-db` | Diesel schema, embedded migrations, repository implementations, FTS5. | core, doc |
| `cardfile-i18n` | Locale loading and the `t!()` macro. | — |
| `cardfile-editor` | GPUI WYSIWYG editor widget (incl. inline media player, bidi). | doc, i18n, media |
| `cardfile-updater` | New-version check and minisign verification. | core |
| `cardfile-media` | Inline audio playback engine (symphonia + rodio; patent-free MP3/WAV/OGG/FLAC only). No GPUI. | — |
| `cardfile-ai` | Blocking LLM client (provider presets, tool calling, agent loop). No GPUI. | — |
| `cardfile-agent` | AI tools over the database (used by the MCP server and the chat). | core, db, doc |
| `cardfile-mcp` | Embedded MCP server (Streamable HTTP on 127.0.0.1); tools come from the app via a trait. | — |
| `cardfile-ui` | GPUI application views. | everything above |
| `cardfile` | Binary: CLI args, logging, data dir, wiring. | ui |

Never create dependency cycles. `cardfile-core` must not depend on GPUI or Diesel.

## Commands

```bash
cargo build                                   # debug build of the app
cargo run                                     # run the app
cargo run -- --portable                       # run with data next to the executable
cargo test --workspace                        # all tests
cargo fmt --all                               # format
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                              # licenses, advisories, sources
```

All four checks (fmt, clippy, test, deny) must pass before a PR is opened. CI enforces them.

Linux build deps (Debian/Ubuntu): `libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
libvulkan-dev libfontconfig-dev libfreetype-dev libx11-xcb-dev libssl-dev libasound2-dev pkg-config`
(the full list CI uses is in `scripts/install-linux-deps.sh`).

## Rules

### Dependencies and licenses
- Always use the **latest** published version of a crate when adding it (`cargo search`).
  Declare versions once in the root `[workspace.dependencies]`; crates use `dep.workspace = true`.
- Only permissive licenses (see `deny.toml`). **Never** add GPL/AGPL/LGPL code or crates. Never
  copy code from Zed crates other than `gpui` (most of Zed is GPL/AGPL).
- When code, assets or ideas are taken from another project, keep its license header and add an
  entry to [`docs/CREDITS.md`](docs/CREDITS.md).
- Prefer proven libraries over custom implementations. Justify new dependencies in the PR.

### Code
- Rust edition 2024, latest stable toolchain.
- Libraries use typed errors (`thiserror`); `anyhow` only in the `cardfile` binary and at UI edges.
- No `unwrap()`/`expect()` outside tests unless the invariant is documented in a comment.
- No `unsafe` (workspace lint `unsafe_code = "deny"`).
- Never block the UI thread: DB, crypto, export and network work run on the background executor.
- Every user-visible string goes through `t!("key")`. Add keys to `locales/en.yml` only
  (English is the sole development language; translations are added at the end).
- Sync-ready data: UUIDv7 primary keys, `created_at`/`updated_at`/`deleted_at`, `rev`. Never hard
  delete outside the "empty recycle bin" path.
- Secrets (`Zeroizing`) never get logged or serialized.
- Public items in library crates get doc comments. Keep comments for the *why*.

### Tests
- Unit tests next to the code (`#[cfg(test)]`), integration tests in `crates/*/tests/`.
- DB tests use an in-memory SQLite with migrations applied.
- Document model and exporters use round-trip and golden-file tests.

### Git
- Branch from `main`: `feat/…`, `fix/…`, `chore/…`, `docs/…`, `ci/…`, `refactor/…`.
- [Conventional Commits](https://www.conventionalcommits.org): `feat(editor): add tables`.
- Open a PR against `main`; the PR title must be a conventional commit (squash merge).
- Code, identifiers, commit messages: English. Docs: English, with Spanish where noted.

### Releases
- Version lives in the root `Cargo.toml` (`workspace.package.version`).
- Tag `vX.Y.Z[-pre]` on `main` triggers `.github/workflows/release.yml`.
- Changelog is generated by `git-cliff` (`cliff.toml`).
