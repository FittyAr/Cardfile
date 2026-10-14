# Contributing to Cardfile

Thanks for your interest! Read [AGENTS.md](AGENTS.md) for the conventions; it applies to humans too.

1. Open an issue first for anything bigger than a small fix.
2. Branch from `main` (`feat/…`, `fix/…`, `docs/…`).
3. Make sure `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` and `cargo deny check` pass.
4. Open a pull request whose title is a [Conventional Commit](https://www.conventionalcommits.org).

## Translations

During development the UI is English-only (`locales/en.yml`). Translations are added once the
string set is stable; to contribute one, copy `locales/en.yml` to `locales/<lang>.yml`.

## License

By contributing you agree that your contributions are dual-licensed under MIT and Apache-2.0,
the same terms as the project, without additional terms or conditions.
