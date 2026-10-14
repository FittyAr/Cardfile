//! Converting card documents to and from the Markdown the AI reads and writes.
//!
//! Images and attachments cannot travel through Markdown, so they appear as
//! placeholder lines (`⟦file 2: photo.png⟧`). When the AI sends Markdown back, a
//! paragraph holding just a placeholder becomes that block again; placeholders it
//! leaves out are removed from the card.

use cardfile_doc::{Block, BlockKind, Document, MarkdownOptions, NoBlobs, RichText};

const OPEN: char = '⟦';
const CLOSE: char = '⟧';

fn placeholder(index: usize, block: &Block) -> Option<String> {
    let name = match &block.kind {
        BlockKind::Image(img) if !img.alt.is_empty() => format!("image {}", img.alt),
        BlockKind::Image(_) => "image".to_owned(),
        BlockKind::Attachment(att) => att.file_name.clone(),
        _ => return None,
    };
    Some(format!("{OPEN}file {index}: {name}{CLOSE}"))
}

/// The document as Markdown for the AI, without the card title.
pub fn to_markdown(doc: &Document) -> String {
    let mut copy = doc.clone();
    for (i, block) in copy.blocks.iter_mut().enumerate() {
        if let Some(text) = placeholder(i, block) {
            *block = Block::paragraph(&text);
        }
    }
    cardfile_doc::to_markdown(&copy, None, &MarkdownOptions::default(), &NoBlobs)
        .trim_end()
        .to_owned()
}

/// Parse a placeholder paragraph back to the block index it names.
fn placeholder_index(text: &str) -> Option<usize> {
    let inner = text.trim().strip_prefix(OPEN)?.strip_suffix(CLOSE)?;
    let rest = inner.strip_prefix("file ")?;
    let (n, _) = rest.split_once(':')?;
    n.trim().parse().ok()
}

/// Build a document from the AI's Markdown, restoring the images and attachments of
/// `previous` that it kept as placeholders.
pub fn from_markdown(markdown: &str, previous: Option<&Document>) -> Document {
    let mut doc = cardfile_doc::from_markdown(markdown);
    if let Some(prev) = previous {
        for block in &mut doc.blocks {
            if !matches!(block.kind, BlockKind::Paragraph) {
                continue;
            }
            if let Some(original) = placeholder_index(&block.text.text)
                .and_then(|i| prev.blocks.get(i))
                .filter(|b| matches!(b.kind, BlockKind::Image(_) | BlockKind::Attachment(_)))
            {
                *block = original.clone();
            }
        }
    }
    // A stray placeholder the AI invented stays as plain text; that is harmless.
    doc.normalize();
    doc
}

/// Replace `find` with `replace` in every text block (and table cell), keeping
/// formatting. Returns how many occurrences changed. `all = false` stops after one.
pub fn replace_text(doc: &mut Document, find: &str, replace: &str, all: bool) -> usize {
    if find.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut edit = |text: &mut RichText| -> bool {
        let mut from = 0;
        while let Some(pos) = text.text[from..].find(find) {
            let start = from + pos;
            let style = text.style_at(start);
            text.delete(start..start + find.len());
            text.insert(start, replace, &style);
            count += 1;
            from = start + replace.len();
            if !all {
                return true;
            }
        }
        false
    };
    'outer: for block in &mut doc.blocks {
        match &mut block.kind {
            BlockKind::Table(table) => {
                for row in &mut table.rows {
                    for cell in row {
                        if edit(cell) {
                            break 'outer;
                        }
                    }
                }
            }
            kind if kind.is_textual() && edit(&mut block.text) => break,
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use cardfile_doc::{Attachment, Style};

    use super::*;

    fn doc_with_file() -> Document {
        Document {
            version: cardfile_doc::DOC_VERSION,
            blocks: vec![
                Block::heading(1, "Trip"),
                Block::new(
                    BlockKind::Attachment(Attachment {
                        blob: "abc".into(),
                        file_name: "tickets.pdf".into(),
                        mime: "application/pdf".into(),
                        size: 3,
                    }),
                    "",
                ),
                Block::paragraph("Bring passports."),
            ],
        }
    }

    #[test]
    fn attachments_survive_a_markdown_round_trip() {
        let doc = doc_with_file();
        let md = to_markdown(&doc);
        assert!(md.contains("⟦file 1: tickets.pdf⟧"), "{md}");
        let edited = md.replace("Bring passports.", "Bring passports and cash.");
        let back = from_markdown(&edited, Some(&doc));
        assert_eq!(back.blocks[1], doc.blocks[1]);
        assert_eq!(back.blocks[2].text.text, "Bring passports and cash.");
        // Dropping the placeholder removes the attachment.
        let without = from_markdown("## Trip\n\nNothing else.", Some(&doc));
        assert!(without.blob_ids().is_empty());
    }

    #[test]
    fn replace_keeps_formatting() {
        let mut doc = Document {
            version: cardfile_doc::DOC_VERSION,
            blocks: vec![Block::new(
                BlockKind::Paragraph,
                RichText::styled(
                    "red apple, red car",
                    Style {
                        bold: true,
                        ..Default::default()
                    },
                ),
            )],
        };
        assert_eq!(replace_text(&mut doc, "red", "blue", false), 1);
        assert_eq!(doc.blocks[0].text.text, "blue apple, red car");
        assert_eq!(replace_text(&mut doc, "red", "green", true), 1);
        assert_eq!(doc.blocks[0].text.text, "blue apple, green car");
        assert!(doc.blocks[0].text.style_at(0).bold);
        assert_eq!(replace_text(&mut doc, "", "x", true), 0);
    }
}
