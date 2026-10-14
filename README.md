<p align="center"><img src="assets/logo_trasparente.png" alt="Cardfile" width="160"></p>

# Cardfile

A modern digital card file for your notes, inspired by the classic Windows 3.1 Cardfile.
Native, fast and private: written in Rust with [GPUI](https://gpui.rs), the UI framework behind
the [Zed](https://zed.dev) editor.

> **Status:** Cardfile 2 is a full rewrite and is in **alpha**. Español: [README.es.md](README.es.md)

## Features

- Word-like editor: headings, lists, checklists, tables, code blocks, colors, images and
  attachments. No Markdown syntax to learn, and every card exports to Markdown.
- Full-text search across titles and contents.
- Tags, favorites, pinned cards, manual ordering and version history.
- Card protection levels: **read-only** (no accidental edits), **private** (password to view)
  and **encrypted** (encrypted at rest).
- Multi-user, single-password or password-free mode, chosen on first run.
- Recycle bin, themes, configurable keyboard shortcuts and a command palette.
- Export to Markdown, HTML, PDF and JSON.
- Portable mode, so you can carry your cards on a USB drive.
- Share cards with other users (view or edit); every user has their own lock password and keys.
- Optional AI assistant with your own cloud or local AI (OpenAI, Claude, Gemini, Ollama, LM Studio
  and more): search, rewrite, format, tag and reorder cards by chat. Hidden until you set it up.
- Built-in MCP server and API keys, so AI apps like Claude or Cursor can use your cards; every AI
  change is attributed to the AI and can be undone.
- Audio and video attachments play inside the card; drag images and files between cards.
- Chinese, Japanese, Korean and right-to-left scripts (Hebrew, Arabic, Persian).
- 21 interface languages.
- No telemetry.

> **About translations:** every translation other than English was made with AI. We apologize
> for any mistakes and we are grateful for reports so we can fix them: please
> [open an issue](https://github.com/FittyAr/Cardfile/issues).

## Install

**Windows (PowerShell)**

```powershell
irm https://github.com/FittyAr/Cardfile/releases/latest/download/cardfile-installer.ps1 | iex
```

**Windows (winget)**

```powershell
winget install FittyAr.Cardfile
```

**Linux**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/FittyAr/Cardfile/releases/latest/download/cardfile-installer.sh | sh
```

`.deb`, `.rpm`, AppImage and the Inno Setup installer are attached to every
[release](https://github.com/FittyAr/Cardfile/releases). Microsoft Store and Linux repositories
are coming.

### Verifying downloads

Release files are signed with [minisign](https://jedisct1.github.io/minisign/):

```bash
minisign -Vm <file> -P <public key from docs/SIGNING.md>
```

The Windows executables are not Authenticode-signed (only the Microsoft Store package is), so
SmartScreen may warn on first run.

## Documentation

- [User guide](docs/USER_GUIDE.md)
- [Releasing](docs/RELEASING.md) and [verifying downloads](docs/SIGNING.md)

## Build from source

Requires the latest stable [Rust](https://rustup.rs).

```bash
git clone https://github.com/FittyAr/Cardfile
cd Cardfile
cargo run --release
```

Linux needs the system packages listed in [AGENTS.md](AGENTS.md#commands).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The roadmap and architecture are in
[docs/PLAN.md](docs/PLAN.md).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT), at your option.

Third-party software and credits: [docs/CREDITS.md](docs/CREDITS.md). Every release ships
`THIRD_PARTY_LICENSES.html`.
