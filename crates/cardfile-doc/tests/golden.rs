//! Golden-file tests for the exporters.
//!
//! `tests/fixtures/sample.json` is a document that uses every block kind and inline
//! style. Each exporter's output for it is compared with a file in `tests/golden/`.
//!
//! When an exporter changes on purpose, regenerate the goldens and review the diff:
//!
//! ```text
//! CARDFILE_UPDATE_GOLDENS=1 cargo test -p cardfile-doc --test golden
//! ```

use std::path::{Path, PathBuf};

use base64::Engine as _;
use cardfile_doc::{BlobResolver, Document, MarkdownOptions, to_html, to_markdown};

const TITLE: &str = "Golden <card>";

/// A valid 1×1 PNG, so the image blob embeds in HTML and renders in PDF.
const PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

struct Blobs;

impl BlobResolver for Blobs {
    fn resolve(&self, blob_id: &str) -> Option<(String, Vec<u8>)> {
        match blob_id {
            "img1" => Some((
                "image/png".into(),
                base64::engine::general_purpose::STANDARD
                    .decode(PIXEL_PNG)
                    .ok()?,
            )),
            "att1" => Some(("application/pdf".into(), b"%PDF-1.7 stub".to_vec())),
            _ => None,
        }
    }
}

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn sample() -> Document {
    let json = std::fs::read_to_string(dir().join("fixtures/sample.json")).unwrap();
    Document::from_json(&json).unwrap()
}

/// Compare `actual` with `tests/golden/<name>`, or rewrite the file when
/// `CARDFILE_UPDATE_GOLDENS` is set.
fn check(name: &str, actual: &str) {
    let path = dir().join("golden").join(name);
    if std::env::var_os("CARDFILE_UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden {}: {e}; run with CARDFILE_UPDATE_GOLDENS=1",
            path.display()
        )
    });
    // Checkouts may convert line endings; the exporters always write `\n`.
    let expected = expected.replace("\r\n", "\n");
    pretty_assertions::assert_eq!(expected, actual, "golden {name} differs");
}

#[test]
fn fixture_covers_every_block_kind() {
    let doc = sample();
    let kinds: std::collections::BTreeSet<&str> = doc
        .blocks
        .iter()
        .map(|b| match b.kind {
            cardfile_doc::BlockKind::Paragraph => "paragraph",
            cardfile_doc::BlockKind::Heading(_) => "heading",
            cardfile_doc::BlockKind::BulletList => "bullet",
            cardfile_doc::BlockKind::NumberedList => "numbered",
            cardfile_doc::BlockKind::CheckList { .. } => "check",
            cardfile_doc::BlockKind::Quote => "quote",
            cardfile_doc::BlockKind::Code { .. } => "code",
            cardfile_doc::BlockKind::Divider => "divider",
            cardfile_doc::BlockKind::Image(_) => "image",
            cardfile_doc::BlockKind::Attachment(_) => "attachment",
            cardfile_doc::BlockKind::Table(_) => "table",
        })
        .collect();
    assert_eq!(kinds.len(), 11, "{kinds:?}");
}

#[test]
fn markdown_golden() {
    let md = to_markdown(&sample(), Some(TITLE), &MarkdownOptions::default(), &Blobs);
    check("sample.md", &md);
}

#[test]
fn markdown_with_extras_golden() {
    let opts = MarkdownOptions {
        html_for_extras: true,
        image_links_relative: true,
        exact_heading_levels: true,
    };
    let md = to_markdown(&sample(), Some(TITLE), &opts, &Blobs);
    check("sample.extras.md", &md);
}

#[test]
fn html_golden() {
    check("sample.html", &to_html(&sample(), TITLE, &Blobs));
}

#[test]
fn json_golden() {
    // The JSON export is the pretty-printed document.
    let mut json = serde_json::to_string_pretty(&sample()).unwrap();
    json.push('\n');
    check("sample.json", &json);
    // And it reads back to the same document.
    assert_eq!(Document::from_json(&json).unwrap(), sample());
}

#[test]
fn plain_text_golden() {
    let mut text = cardfile_doc::to_plain_text(&sample());
    text.push('\n');
    check("sample.txt", &text);
}

#[test]
fn pdf_renders() {
    // PDF output embeds fonts and metadata, so it is checked for sanity only.
    let pdf = cardfile_doc::pdf::to_pdf(&sample(), TITLE, &Blobs).unwrap();
    assert!(pdf.starts_with(b"%PDF-"), "not a PDF");
    assert!(
        pdf.len() > 1000,
        "suspiciously small PDF: {} bytes",
        pdf.len()
    );
    let tail = &pdf[pdf.len().saturating_sub(64)..];
    assert!(tail.windows(5).any(|w| w == b"%%EOF"), "truncated PDF");
}
