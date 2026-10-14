# Credits

Cardfile stands on the shoulders of these projects. The full, generated list of every crate and
its license ships with each release as `THIRD_PARTY_LICENSES.html` (`cargo about`).

## Frameworks and major libraries

| Project | Use in Cardfile | License |
|---------|-----------------|---------|
| [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) by Zed Industries | GPU-accelerated UI framework | Apache-2.0 |
| [gpui-kit / gpui-component](https://github.com/longbridge/gpui-component) by Longbridge | UI components, themes, icons | Apache-2.0 |
| [Diesel](https://diesel.rs) | ORM and migrations | MIT OR Apache-2.0 |
| [SQLite](https://sqlite.org) (via `libsqlite3-sys`) | Embedded database and FTS5 search | Public domain |
| [RustCrypto](https://github.com/RustCrypto) (`argon2`, `chacha20poly1305`) | Password hashing and encryption | MIT OR Apache-2.0 |
| [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) | Content hashing of images and attachments | CC0-1.0 OR Apache-2.0 |
| [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) | Markdown handling | MIT |
| [krilla](https://github.com/LaurenzV/krilla) | PDF export | MIT OR Apache-2.0 |
| [spellbook](https://github.com/helix-editor/spellbook) | Spell checking (Hunspell-compatible) | MPL-2.0 (unmodified) |
| [rust-i18n](https://github.com/longbridge/rust-i18n) | Localization | MIT |
| [minisign-verify](https://github.com/jedisct1/rust-minisign-verify) | Release signature verification | MIT |
| [dist (cargo-dist)](https://github.com/axodotdev/cargo-dist) | Release pipeline and installers | MIT OR Apache-2.0 |
| [Inno Setup](https://jrsoftware.org/isinfo.php) | Windows installer | Inno Setup License (permissive) |

## Dictionaries

| Dictionary | Source | License |
|------------|--------|---------|
| English (en_US) Hunspell dictionary | [wooorm/dictionaries](https://github.com/wooorm/dictionaries) (SCOWL) | See `assets/dictionaries/en/LICENSE` |

## Inspiration

- Microsoft Cardfile (Windows 3.x), for the original concept.
- [Zed](https://zed.dev), for keymap and theme design (concepts only, no code).
