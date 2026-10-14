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
| [Lucide](https://lucide.dev) icons (via gpui-kit-assets) | Toolbar and UI icons | ISC |
| [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) | Markdown paste | MIT |
| [krilla](https://github.com/LaurenzV/krilla) | PDF export | MIT OR Apache-2.0 |
| [HarfRust](https://github.com/harfbuzz/harfrust) | Text measuring for PDF layout | MIT |
| [ureq](https://github.com/algesten/ureq) | Update check (GitHub releases API) | MIT OR Apache-2.0 |
| [spellbook](https://github.com/helix-editor/spellbook) | Spell checking (Hunspell-compatible) | MPL-2.0 (unmodified) |
| [rust-i18n](https://github.com/longbridge/rust-i18n) | Localization | MIT |
| [dist (cargo-dist)](https://github.com/axodotdev/cargo-dist) | Release pipeline and installers | MIT OR Apache-2.0 |
| [Inno Setup](https://jrsoftware.org/isinfo.php) | Windows installer | Inno Setup License (permissive) |
| [winresource](https://github.com/BenjaminRi/winresource) | Windows executable icon and version info | MIT |
| [cargo-deb](https://github.com/kornelski/cargo-deb), [cargo-generate-rpm](https://github.com/cat-in-136/cargo-generate-rpm), [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) | Linux packages (build tools, not shipped) | MIT |
| [cargo-about](https://github.com/EmbarkStudios/cargo-about) | Third-party license report | MIT OR Apache-2.0 |
| [minisign](https://jedisct1.github.io/minisign/) | Release signatures (CI tool) | ISC |

## Dictionaries

| Dictionary | Source | License |
|------------|--------|---------|
| English (en_US) Hunspell dictionary | [wooorm/dictionaries](https://github.com/wooorm/dictionaries) `dictionary-en` (SCOWL) | Shipped as `dictionaries/LICENSE-en_US` in every package |

## Inspiration

- Microsoft Cardfile (Windows 3.x), for the original concept.
- Cardfile 1.x (Python/Flet), whose 9 themes were ported to Cardfile 2.
- [Zed](https://zed.dev), for keymap and theme design (concepts only, no code).
