# Changelog

All notable changes to this project are documented here. Generated with
[git-cliff](https://git-cliff.org) from Conventional Commits (`cliff.toml`).

## [2.0.0-beta.2] - 2026-10-27

### Features

- Cards in the list have a right-click menu: pin, favorite, share, history, export and
  delete (restore or delete permanently in the recycle bin).

### Bug Fixes

- Linux: the window has a single frame again instead of a dark band around it.
- Linux: text keeps subpixel antialiasing (the window background stays opaque).
## [2.0.0-beta.1] - 2026-10-19

First beta.

### Features

- The Spanish (es_ES) spelling dictionary ships with Cardfile, next to English.
- Dictionaries for the other interface languages (German, Persian, French, Italian, Korean,
  Dutch, Polish, Portuguese (Brazil), Russian, Ukrainian and Vietnamese) can be installed
  from Settings → General, right below the language picker.
- Changing the interface language switches spell checking to that language when its
  dictionary is installed; another dictionary can still be chosen in Settings → Editor.

### Bug Fixes

- The tag list, the card list and the card editor show a scrollbar, and scrollbars stay
  visible instead of appearing only while scrolling.

## [2.0.0-alpha.6] - 2026-10-19

### Bug Fixes

- Confirmation and input dialogs (empty recycle bin, delete, lock password, tags, links) show
  OK and Cancel buttons again.
- Card saves are only marked as saved once written; a failed save keeps your edits, shows a
  notification and is retried. Quitting and updating wait for the last save.
- Auto-lock counts activity anywhere in the window, and locking closes open dialogs.
- Decrypted attachments live in a temporary folder removed on lock and quit.
- A database that cannot be opened is reported instead of failing silently.
- Markdown export keeps line breaks, escapes text that looked like Markdown and round-trips
  heading levels; right-to-left detection and the PDF quote bar are fixed.
- The AI keeps heading levels when it edits cards, refuses to change read-only or recycled
  cards and says when it stops at its step limit.
- Pasting, dropping files, loading images and spell checking no longer freeze the editor.
- A corrupt audio file stops its player instead of crashing the app.
- Updates time out on stalled connections, can be cancelled, only install signed releases
  and roll back completely if installing fails.
- The Windows installer rolls back a failed install; the Linux `.desktop` entry works with
  paths that contain spaces.
- Deleting a user revokes their API keys, and leaving encryption or emptying the recycle bin
  no longer leaves plaintext behind on disk.

## [2.0.0-alpha.5] - 2026-10-18

### Features

- Portable mode turns on by itself when a `data` folder with `cardfile.db` sits next to the
  executable, so a Cardfile folder copied or synced (e.g. in OneDrive) keeps working without
  `portable.flag`.
- Settings → Data explains that the cards and every setting are in `cardfile.db` and the
  shortcuts in `keymap.json`, both in the data folder.
- About links to the project website, https://cardfile.fitty.ar/.

### Bug Fixes

- The "Open data folder", "Open" (keymap), "Back up now" and "Empty recycle bin" buttons in
  Settings did nothing.
- "Open data folder" opens the folder itself.

## [2.0.0-alpha.4] - 2026-10-16

### Bug Fixes

- Linux: removing the old `~/.cardfile` installation no longer breaks the graphical login (black
  screen). The cargo-dist installer of alpha.1 to alpha.3 sourced `~/.cardfile/bin/env` from
  `~/.profile`, `~/.bashrc`, `~/.zshrc` and fish without checking that it exists. `install.sh`
  now removes those lines with the old installation (and guards them while it stays), and
  Cardfile guards them on every start.

### Features

- One-line installers that always install the newest release, under a shared FittyAr folder;
  the cards live in the FittyAr folder on Linux and macOS too.

## [2.0.0-alpha.3] - 2026-10-15

### Features

- In-app updates: Cardfile downloads the new version, verifies its SHA-256 and minisign
  signature, installs it and restarts (installer, standalone, portable and AppImage installs;
  Store and package-manager installs keep notifying).
- Own title bar in the style of Zed, with the app menu and window controls.
- New flat logo and regenerated icons for every platform.
- Bundled Inter and JetBrains Mono fonts, so text looks the same everywhere (UI and PDF).
- Resize images with handles: the corner keeps proportions, the edges stretch freely.
- Settings redesigned with a grouped sidebar; AI split into Services, Assistant and AI apps (MCP).
- Clearer "Add AI service": pick a cloud or local provider, then API key, model and name.
- AI service API keys are stored in the system's credential manager.
- Right-to-left text in the card title and the assistant input.
- Improved About window.

### Changes

- Patent-encumbered codecs removed: inline audio plays MP3, WAV, OGG/Vorbis and FLAC; video and
  AAC open in the system player, and Cardfile says so when no application can open a file.

## [2.0.0-alpha.2] - 2026-10-14

### Features

- AI assistant: connect a cloud or local AI (OpenAI, Anthropic, Gemini, Mistral, Groq, DeepSeek,
  xAI, OpenRouter, Together, Perplexity, Azure OpenAI, Ollama, LM Studio or a custom service) and
  chat to search, rewrite, format, tag, pin and reorder cards. Hidden until configured.
- Built-in MCP server (Streamable HTTP on 127.0.0.1) with per-user API keys (read-only and
  private-card options) so AI apps such as Claude, Cursor or VS Code can use Cardfile.
- Built-in "Cardfile AI" account: every change records who made it (and through which key),
  shown in the status bar, the version history and the AI activity list.
- Share cards with other users as view or edit; "Shared with me" view.
- Per-user lock passwords and keys: each encrypted card has its own key, wrapped for every user
  with access, so shared encrypted cards open with each person's own password.
- Inline playback of audio (MP3, WAV, OGG, FLAC, AAC) and MP4/H.264 video attachments.
- Drag images and attachments within a card or onto another card (Ctrl/Alt copies).
- Chinese/Japanese/Korean fallback fonts in PDF export; right-to-left text (Hebrew, Arabic,
  Persian) in the editor, labels, HTML and PDF.
- 20 new interface languages (AI-translated).

### Bug fixes

- Inserting a link (Ctrl+K) no longer crashes the app.

## [2.0.0-alpha.1] - Unreleased

Cardfile 2 is a complete rewrite in Rust with a GPUI interface, replacing the
Python/Flet 1.x line.

### Features

- WYSIWYG card editor: headings, lists, checklists, quotes, code blocks, tables,
  images, attachments, links, colors, highlight, alignment and Markdown shortcuts.
- Full-text search, tags, favorites, pinned cards, manual ordering and version history.
- Read-only, private and encrypted cards (Argon2id + XChaCha20-Poly1305).
- No-password, single-password and multi-user sign-in, chosen on first run.
- Export to Markdown, HTML, PDF and JSON; recycle bin; portable mode.
- 14 themes, configurable shortcuts, command palette, spell checking.
- Releases for Windows and Linux: one-line installers, Inno Setup, MSIX, deb, rpm, AppImage.
