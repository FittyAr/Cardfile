//! Markdown → [`Document`], used when the user pastes Markdown text into the editor.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::model::{Block, BlockKind, Document, MAX_INDENT, RichText, Style, Table};

/// Parse GitHub Flavored Markdown into a document.
pub fn from_markdown(md: &str) -> Document {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut b = Builder::default();
    for event in Parser::new_ext(md, opts) {
        b.event(event);
    }
    b.flush();
    let mut doc = Document {
        version: crate::DOC_VERSION,
        blocks: b.blocks,
    };
    doc.normalize();
    doc
}

/// Whether `text` looks like Markdown worth converting on paste.
pub fn looks_like_markdown(text: &str) -> bool {
    text.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("# ")
            || t.starts_with("## ")
            || t.starts_with("### ")
            || t.starts_with("- ")
            || t.starts_with("* ")
            || t.starts_with("> ")
            || t.starts_with("```")
            || t.starts_with("- [")
            || (t.starts_with('|') && t.ends_with('|'))
    }) || text.contains("**")
        || text.contains("](")
}

/// Whether an inline HTML tag is a line break (`<br>`, `<br/>`, `<br />`).
fn is_br(html: &str) -> bool {
    let t: String = html
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(t.as_str(), "<br>" | "<br/>")
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    current: Option<Block>,
    style: Vec<Style>,
    lists: Vec<bool>, // ordered?
    quote_depth: usize,
    code: Option<String>,
    table: Option<Table>,
    row: Vec<RichText>,
    cell: Option<RichText>,
}

impl Builder {
    fn style(&self) -> Style {
        self.style.last().cloned().unwrap_or_default()
    }

    fn push_style(&mut self, f: impl FnOnce(&mut Style)) {
        let mut s = self.style();
        f(&mut s);
        self.style.push(s);
    }

    fn flush(&mut self) {
        if let Some(block) = self.current.take() {
            self.blocks.push(block);
        }
    }

    fn context_kind(&self) -> BlockKind {
        match self.lists.last() {
            Some(true) => BlockKind::NumberedList,
            Some(false) => BlockKind::BulletList,
            None if self.quote_depth > 0 => BlockKind::Quote,
            None => BlockKind::Paragraph,
        }
    }

    fn indent(&self) -> u8 {
        u8::try_from(self.lists.len().saturating_sub(1))
            .unwrap_or(MAX_INDENT)
            .min(MAX_INDENT)
    }

    fn ensure_block(&mut self) {
        if self.current.is_none() {
            let mut block = Block::new(self.context_kind(), "");
            block.indent = self.indent();
            self.current = Some(block);
        }
    }

    fn text(&mut self, text: &str) {
        let style = self.style();
        if let Some(code) = &mut self.code {
            code.push_str(text);
            return;
        }
        if let Some(cell) = &mut self.cell {
            let at = cell.len();
            cell.insert(at, text, &style);
            return;
        }
        self.ensure_block();
        if let Some(block) = &mut self.current {
            let at = block.text.len();
            block.text.insert(at, text, &style);
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.text(&t),
            Event::Code(t) => {
                self.push_style(|s| s.code = true);
                self.text(&t);
                self.style.pop();
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.text("\n"),
            Event::Rule => {
                self.flush();
                self.blocks.push(Block::new(BlockKind::Divider, ""));
            }
            Event::TaskListMarker(checked) => {
                self.ensure_block();
                if let Some(block) = &mut self.current {
                    block.kind = BlockKind::CheckList { checked };
                }
            }
            // `<br>` is how line breaks travel in table cells (see the exporter).
            Event::InlineHtml(h) if is_br(&h) => self.text("\n"),
            Event::Html(h) | Event::InlineHtml(h) => self.text(&h),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                let reuse = self.current.as_ref().is_some_and(|b| b.text.is_empty());
                if !reuse && self.table.is_none() {
                    self.flush();
                    // A second paragraph inside a list item continues as plain text.
                    let kind = if self.lists.is_empty() {
                        self.context_kind()
                    } else {
                        BlockKind::Paragraph
                    };
                    let mut block = Block::new(kind, "");
                    block.indent = if self.lists.is_empty() {
                        0
                    } else {
                        self.indent() + 1
                    };
                    self.current = Some(block);
                }
            }
            Tag::Heading { level, .. } => {
                self.flush();
                // Standard levels (`#` is Heading 1). File exports shift headings one
                // level down under the card title; the AI tools export exact levels
                // (`MarkdownOptions::exact_heading_levels`) so they round-trip.
                let level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    _ => 3,
                };
                self.current = Some(Block::heading(level, ""));
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush();
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => {
                        l.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
                self.current = Some(Block::new(BlockKind::Code { language: lang }, ""));
                self.code = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.lists.push(start.is_some());
            }
            Tag::Item => {
                self.flush();
                self.ensure_block();
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Table {
                    rows: Vec::new(),
                    header: true,
                });
            }
            Tag::TableHead | Tag::TableRow => self.row.clear(),
            Tag::TableCell => self.cell = Some(RichText::default()),
            Tag::Emphasis => self.push_style(|s| s.italic = true),
            Tag::Strong => self.push_style(|s| s.bold = true),
            Tag::Strikethrough => self.push_style(|s| s.strike = true),
            // Unsafe schemes keep their text but lose the link.
            Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                let url = dest_url.to_string();
                let safe = crate::is_safe_link(&url);
                self.push_style(|s| {
                    if safe {
                        s.link = Some(url);
                    }
                });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item => {
                if self.table.is_none() {
                    self.flush();
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                let code = self.code.take().unwrap_or_default();
                if let Some(block) = &mut self.current {
                    block.text = RichText::plain(code.strip_suffix('\n').unwrap_or(&code));
                }
                self.flush();
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::TableCell => {
                if let Some(cell) = self.cell.take() {
                    self.row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = std::mem::take(&mut self.row);
                if let Some(table) = &mut self.table {
                    table.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(mut table) = self.table.take() {
                    table.normalize();
                    self.blocks.push(Block::new(BlockKind::Table(table), ""));
                }
            }
            TagEnd::Emphasis
            | TagEnd::Strong
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image => {
                self.style.pop();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Mark;

    #[test]
    fn drops_unsafe_link_schemes() {
        let doc = from_markdown("[ok](https://a.b) [bad](javascript:alert(1)) [f](file:///c:/x)");
        let links: Vec<_> = doc.blocks[0]
            .text
            .spans()
            .filter_map(|(_, s)| s.link.clone())
            .collect();
        assert_eq!(links, ["https://a.b"]);
        assert_eq!(crate::to_plain_text(&doc).trim(), "ok bad f");
    }

    #[test]
    fn parses_common_markdown() {
        let doc = from_markdown(
            "# Title\n\nSome **bold** and *it*.\n\n- a\n  - b\n1. one\n- [x] done\n\n> quote\n\n```rust\nfn x() {}\n```\n\n---\n\n| A | B |\n|---|---|\n| 1 | 2 |\n",
        );
        let kinds: Vec<_> = doc.blocks.iter().map(|b| b.kind.clone()).collect();
        assert_eq!(kinds[0], BlockKind::Heading(1));
        assert_eq!(kinds[1], BlockKind::Paragraph);
        assert!(doc.blocks[1].text.has_mark(5..9, Mark::Bold));
        assert!(doc.blocks[1].text.has_mark(14..16, Mark::Italic));
        assert_eq!(kinds[2], BlockKind::BulletList);
        assert_eq!(
            (kinds[3].clone(), doc.blocks[3].indent),
            (BlockKind::BulletList, 1)
        );
        assert_eq!(kinds[4], BlockKind::NumberedList);
        assert_eq!(kinds[5], BlockKind::CheckList { checked: true });
        assert_eq!(kinds[6], BlockKind::Quote);
        assert_eq!(
            kinds[7],
            BlockKind::Code {
                language: "rust".into()
            }
        );
        assert_eq!(doc.blocks[7].text.text, "fn x() {}");
        assert_eq!(kinds[8], BlockKind::Divider);
        let BlockKind::Table(t) = &kinds[9] else {
            panic!("table expected")
        };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1][1].text, "2");
    }

    #[test]
    fn markdown_roundtrip_through_export() {
        let md = "## Heading\n\nHello **world**\n\n- item\n";
        let doc = from_markdown(md);
        let exact = crate::MarkdownOptions {
            exact_heading_levels: true,
            ..Default::default()
        };
        let out = crate::to_markdown(&doc, None, &exact, &crate::NoBlobs);
        // With exact heading levels (as the AI tools use) a round trip keeps them.
        assert_eq!(out, md);
    }

    #[test]
    fn heading_levels_round_trip() {
        let md = "# One\n\n## Two\n\n### Three\n";
        let doc = from_markdown(md);
        let levels: Vec<_> = doc.blocks.iter().map(|b| b.kind.clone()).collect();
        assert_eq!(
            levels,
            [
                BlockKind::Heading(1),
                BlockKind::Heading(2),
                BlockKind::Heading(3)
            ]
        );
        let exact = crate::MarkdownOptions {
            exact_heading_levels: true,
            ..Default::default()
        };
        assert_eq!(crate::to_markdown(&doc, None, &exact, &crate::NoBlobs), md);
        // File exports put the card title in H1 and shift headings one level down.
        let file = crate::to_markdown(
            &doc,
            None,
            &crate::MarkdownOptions::default(),
            &crate::NoBlobs,
        );
        assert!(file.starts_with("## One"), "{file}");
        assert_eq!(from_markdown("# Top").blocks[0].kind, BlockKind::Heading(1));
    }

    #[test]
    fn br_in_table_cells_is_a_line_break() {
        let doc = from_markdown("| A |\n| --- |\n| one<br>two |\n");
        let BlockKind::Table(t) = &doc.blocks[0].kind else {
            panic!("table expected")
        };
        assert_eq!(t.rows[1][0].text, "one\ntwo");
    }

    #[test]
    fn detects_markdown() {
        assert!(looks_like_markdown("# hi"));
        assert!(looks_like_markdown("see [x](http://a)"));
        assert!(!looks_like_markdown("just text"));
    }
}
