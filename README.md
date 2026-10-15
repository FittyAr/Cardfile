# Cardfile — Official Website

This orphan branch (`gh-pages`) holds the static source of the **Cardfile** website, a modern digital
card file for your notes written in Rust with GPUI.

Live at [cardfile.fitty.ar](https://cardfile.fitty.ar).

## Contents

- `index.html`, `changelog.html`: landing page and release history (read at runtime from
  [`CHANGELOG.md`](https://github.com/FittyAr/Cardfile/blob/main/CHANGELOG.md) on `main`).
- `css/`, `js/`: styles and scripts, including the ES/EN strings in `js/translations.js`.
- `assets/`: brand icon (from `assets/brand` on `main`), social card and screenshots (`assets/screens`).
- `tools/showcase/`: the scripts that seed a demo database with fictional data and take the
  screenshots. See [tools/showcase/README.md](tools/showcase/README.md).
- `llms.txt`, `robots.txt`, `sitemap.xml`, `CNAME`, `.nojekyll`.

## Local development

```bash
git checkout gh-pages
python -m http.server 8000
```

Then open <http://localhost:8000>.

## Cardfile source code

The app lives on the [`main` branch](https://github.com/FittyAr/Cardfile/tree/main).

## License

The website is licensed under the [MIT License](LICENSE). Inter and JetBrains Mono are loaded from
Google Fonts (SIL Open Font License 1.1).
