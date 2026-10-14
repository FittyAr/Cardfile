# Changelog

All notable changes to this project are documented here. Generated with
[git-cliff](https://git-cliff.org) from Conventional Commits (`cliff.toml`).

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
