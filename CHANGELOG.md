# Changelog

All notable changes to this project are documented here. Generated with
[git-cliff](https://git-cliff.org) from Conventional Commits (`cliff.toml`).

## [2.0.0-beta.6] - 2026-11-22

Data-safety fixes and Microsoft Store preparation from a pre-release audit. Testing release.

### Bug Fixes

- Closing the window right after typing no longer loses the last changes: closing
  (X, Alt+F4, the Linux title-bar button) and quitting now save the open card first,
  and ask before quitting if the save failed.
- A version of a card written by the assistant, an MCP client or another user is
  always kept in the history before your edits overwrite it.
- An image pasted again after it was left unused is no longer removed by the
  automatic clean-up before the card is saved.
- Microsoft Store package: notes, backups and the database's journal are kept in the
  real `%APPDATA%\FittyAr\Cardfile`, so uninstalling or reinstalling the Store app
  never removes them. Requires Windows 10 version 1903 or later.

### Features

- Daily automatic backups are on by default for new installations.
- Cardfile warns at sign-in when its data is in a folder synced by OneDrive,
  Dropbox, Google Drive or a similar service.
- The assistant panel has a **Report** button for inappropriate answers.

### Security

- Update `url` to 2.5.9 (RUSTSEC-2026-0592).

### Documentation

- Privacy policy (`docs/PRIVACY.md`) and Microsoft Store submission notes.

## [2.0.0-beta.5] - 2026-11-21

Quality and security hardening from a full project audit. Testing release.

### Security

- Update the bundled SQLite to 3.54 (Diesel 2.4, libsqlite3-sys 0.40) for
  RUSTSEC-2026-0459, which affects opening a database with a crafted `-wal` file,
  such as a backup being restored.
- Removing someone from an encrypted card now moves it to a new key, so the removed
  user can no longer read later changes or the history.
- The assistant asks before running tools that change or delete cards (allow once,
  allow for the conversation, or deny).
- AI services are reached over plain `http://` only on this computer (localhost);
  remote services need `https://`, so API keys never travel in clear text.
- The Windows updater locks the verified installer and checks it again right before
  starting it.
- Password and key hashes with out-of-range Argon2 parameters are rejected instead
  of exhausting memory.

### Fixes

- An attachment, image or pasted content saved in the background is no longer
  inserted into a different card when you switch cards meanwhile.
- Moving a block to another card never removes a matching block from a third card.
- A card whose content cannot be read opens read-only with a notice instead of
  as an empty card that would overwrite it.
- AI keys are kept in the settings until the system credential store has accepted
  them, so none is lost when the store fails.
- Cards in the recycle bin cannot be edited until restored.
- Markdown export keeps code blocks and inline code intact when they contain
  backticks.
- The MCP server keeps answering after an internal error, and reordering cards from
  the assistant is applied all at once or not at all.
- Cardfile starts with command-line arguments that are not valid UTF-8, and crash
  logs include a backtrace.

### Internal

- CI runs with read-only tokens, `--locked` builds and pinned tools; release jobs
  receive only the secrets they use.
- Public items are documented and the lint enforces it; golden-file tests cover the
  exporters; large view functions were split and more logic is unit tested.

## [2.0.0-beta.4] - 2026-11-04

### Fixes

- Keep the update-channel dropdown visible in narrow windows and wrap explanatory
  text. Label the channels Recommended (Latest) and Preview (Prerelease), with a
  clearer description of preview releases and downgrade protection.
- Complete 51 missing messages in all 20 translated locales, including backups,
  restoration, portable copies, dictionaries and update channels. Translate the
  new installation-mode messages in every supported installer language.
- Check translation coverage and interpolation variables in CI.

## [2.0.0-beta.3] - 2026-11-04

Testing release. The maintainer will promote it to Latest after validation.

### Features

- Cardfile remembers the normal window size and whether it was maximized.
- Windows setup can install Cardfile on this computer or create a portable folder
  on a USB drive, OneDrive or another disk. PowerShell uses the same verified setup.
- Administrators can choose a backup folder, create manual backups and enable daily
  backups while Cardfile is running, with configurable retention (10 automatic copies
  by default). Manual and safety backups are retained separately.
- Restore an entire database backup from Settings → Data. Cardfile checks it, saves
  a safety copy, prepares the replacement and restarts using the restored accounts.
- Create a portable copy of the complete collection, including users, cards and
  attachments. Existing destinations are never overwritten. MCP and automatic backups
  are disabled in the copy; system credential-store AI keys must be entered again.
- Choose Latest or Prerelease updates in Settings → General. Latest follows GitHub's
  designation, even for a promoted beta tag. Switching channels never downgrades.

### Reliability

- Backups include committed WAL data and are verified before publication. Restoration
  has restart recovery and a rollback copy; a local process lock prevents two Cardfile
  processes from opening the same collection.
- Update signing and checksum verification remain mandatory in both channels.
- Updated direct and compatible transitive dependencies, including GPUI 0.9 and the
  fixes for the pulldown-cmark advisories. Refreshed CI actions and installer checks.

### Portable use

Close Cardfile and finish OneDrive synchronization before using the folder on another
computer. Simultaneous cross-computer use is not supported. Keep the whole portable
folder together and close Cardfile before removing a USB drive.

The new channel selection applies from this version onward; older beta updaters retain
their previous prerelease-detection behavior until updated.

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
