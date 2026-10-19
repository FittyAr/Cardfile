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
| [ureq](https://github.com/algesten/ureq) | Update check and download (GitHub releases) | MIT OR Apache-2.0 |
| [sha2](https://github.com/RustCrypto/hashes) (RustCrypto) | SHA-256 of downloaded updates | MIT OR Apache-2.0 |
| [zip](https://github.com/zip-rs/zip2) | Unpacking Windows update archives | MIT |
| [tar](https://github.com/composefs/tar-rs) | Unpacking Linux update archives | MIT OR Apache-2.0 |
| [lzma-rust2](https://github.com/hasenbanck/lzma-rust2) | `.xz` decompression of Linux update archives (pure Rust) | Apache-2.0 |
| [tempfile](https://github.com/Stebalien/tempfile) | Temporary files for downloads and atomic replacement | MIT OR Apache-2.0 |
| [tiny-http](https://github.com/tiny-http/tiny-http) | HTTP server for the embedded MCP endpoint | MIT OR Apache-2.0 |
| [spellbook](https://github.com/helix-editor/spellbook) | Spell checking (Hunspell-compatible) | MPL-2.0 (unmodified) |
| [Symphonia](https://github.com/pdeljanov/Symphonia) | Audio demuxing and decoding (MP3, WAV, OGG/Vorbis, FLAC) | MPL-2.0 (unmodified) |
| [rodio](https://github.com/RustAudio/rodio) and [cpal](https://github.com/RustAudio/cpal) | Audio output | MIT OR Apache-2.0 / Apache-2.0 |
| [x25519-dalek](https://github.com/dalek-cryptography/curve25519-dalek) | Key pairs for sharing encrypted cards | BSD-3-Clause |
| [keyring](https://github.com/open-source-cooperative/keyring-rs) | Stores AI service API keys in the system credential manager | MIT OR Apache-2.0 |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | Bundled monospace font | SIL OFL 1.1 |
| [unicode-bidi](https://github.com/servo/unicode-bidi) | Right-to-left text ordering | MIT OR Apache-2.0 |
| [ar-reshaper](https://github.com/YouKnow-sys/ar-reshaper) | Arabic letter shaping for plain labels | MIT |
| [rust-i18n](https://github.com/longbridge/rust-i18n) | Localization | MIT |
| [dist (cargo-dist)](https://github.com/axodotdev/cargo-dist) | Release pipeline and installers | MIT OR Apache-2.0 |
| [Inno Setup](https://jrsoftware.org/isinfo.php) | Windows installer | Inno Setup License (permissive) |
| [winresource](https://github.com/BenjaminRi/winresource) | Windows executable icon and version info | MIT |
| [cargo-deb](https://github.com/kornelski/cargo-deb), [cargo-generate-rpm](https://github.com/cat-in-136/cargo-generate-rpm), [linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) | Linux packages (build tools, not shipped) | MIT |
| [cargo-about](https://github.com/EmbarkStudios/cargo-about) | Third-party license report | MIT OR Apache-2.0 |
| [minisign](https://jedisct1.github.io/minisign/) | Release signatures (CI tool) | ISC |
| [minisign-verify](https://github.com/jedisct1/rust-minisign-verify) | Update signature check; its test vector is reused in `cardfile-updater` tests | MIT |

## Brand

| Project | Use in Cardfile | License |
|---------|-----------------|---------|
| [Inter](https://github.com/rsms/inter) by Rasmus Andersson (Inter Display SemiBold) | Bundled text font (`assets/fonts/`) and the letterforms of the "Cardfile" wordmark (`assets/brand/wordmark.svg`) | SIL Open Font License 1.1 |
| [resvg](https://github.com/linebender/resvg) (via `resvg-py`), [HarfBuzz](https://github.com/harfbuzz/harfbuzz) (via `uharfbuzz`), [fontTools](https://github.com/fonttools/fonttools), [Pillow](https://python-pillow.org) | Rendering the logo rasters and icons in `scripts/render_brand.py` (build tools, not shipped) | Apache-2.0 OR MIT / MIT, Apache-2.0 / MIT / MIT-CMU |

## Dictionaries

| Dictionary | Source | License |
|------------|--------|---------|
| English (en_US) Hunspell dictionary | [wooorm/dictionaries](https://github.com/wooorm/dictionaries) `dictionary-en` (SCOWL) | Shipped as `dictionaries/LICENSE-en_US` in every package |
| Spanish (es_ES) Hunspell dictionary | [wooorm/dictionaries](https://github.com/wooorm/dictionaries) `dictionary-es` (RLA-ES) | MPL-1.1 (chosen from GPL-3.0 / LGPL-3.0 / MPL-1.1); shipped as `dictionaries/LICENSE-es_ES` |
| Other languages | [wooorm/dictionaries](https://github.com/wooorm/dictionaries) | Not shipped: downloaded by the user from Settings, with their own license file |

## Inspiration

- Cardfile 1.x (Python/Flet), whose 9 themes were ported to Cardfile 2.
- [Zed](https://zed.dev), for keymap and theme design (concepts only, no code).
