//! Cardfile's document model and exporters.
//!
//! Cards are stored as a [`Document`] serialized to JSON. Markdown, HTML, PDF and
//! JSON are export formats only; users never edit Markdown directly.

mod export;
mod model;

pub mod rules;

pub use export::{
    BlobResolver, MarkdownOptions, NoBlobs, escape_html, to_html, to_markdown, to_plain_text,
};
pub use model::{
    Align, Attachment, Block, BlockKind, Color, DOC_VERSION, Document, Image, MAX_INDENT, Mark,
    RichText, Run, Style, Table, TextSize,
};

/// Up to `max_chars` characters of plain text on one line, for list previews.
pub fn preview(plain_text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for word in plain_text.split_whitespace() {
        if out.chars().count() + word.chars().count() + 1 > max_chars {
            if out.is_empty() {
                out.extend(word.chars().take(max_chars));
            }
            out.push('…');
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}
