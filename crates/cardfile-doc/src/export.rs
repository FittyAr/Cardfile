//! Text-based exporters: plain text, Markdown (GFM) and standalone HTML.

use std::fmt::Write as _;

use crate::model::{Align, Block, BlockKind, Document, RichText, Style, TextSize};

/// Resolves blob ids to data for exporters that embed images.
pub trait BlobResolver {
    /// Returns `(mime, bytes)` for an image or attachment blob.
    fn resolve(&self, blob_id: &str) -> Option<(String, Vec<u8>)>;
}

/// A resolver that knows no blobs.
pub struct NoBlobs;

impl BlobResolver for NoBlobs {
    fn resolve(&self, _: &str) -> Option<(String, Vec<u8>)> {
        None
    }
}

/// Plain text for search indexing and list previews.
pub fn to_plain_text(doc: &Document) -> String {
    let mut out = String::new();
    for block in &doc.blocks {
        match &block.kind {
            BlockKind::Table(t) => {
                for row in &t.rows {
                    let cells: Vec<&str> = row.iter().map(|c| c.text.as_str()).collect();
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
            BlockKind::Image(img) => {
                if !img.alt.is_empty() {
                    out.push_str(&img.alt);
                    out.push('\n');
                }
            }
            BlockKind::Attachment(att) => {
                out.push_str(&att.file_name);
                out.push('\n');
            }
            BlockKind::Divider => {}
            _ => {
                out.push_str(&block.text.text);
                out.push('\n');
            }
        }
    }
    out.truncate(out.trim_end().len());
    out
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

/// Options for [`to_markdown`].
#[derive(Clone, Debug, Default)]
pub struct MarkdownOptions {
    /// Keep colors, highlight, underline, sizes and alignment as inline HTML.
    /// When `false` those are dropped and only Markdown-native formatting remains.
    pub html_for_extras: bool,
    /// Link images as `blobs/<id>` (for an export folder) instead of embedding data URIs.
    pub image_links_relative: bool,
}

/// Export a document as GitHub Flavored Markdown.
pub fn to_markdown(
    doc: &Document,
    title: Option<&str>,
    opts: &MarkdownOptions,
    blobs: &dyn BlobResolver,
) -> String {
    let mut out = String::new();
    if let Some(title) = title.filter(|t| !t.is_empty()) {
        let _ = writeln!(out, "# {}\n", escape_md(title));
    }
    let mut numbers: Vec<usize> = Vec::new();
    let mut prev_list = false;
    for (i, block) in doc.blocks.iter().enumerate() {
        let indent = "   ".repeat(block.indent as usize);
        let is_list = block.kind.is_list();
        if prev_list && !is_list {
            out.push('\n');
        }
        if !matches!(block.kind, BlockKind::NumberedList) {
            numbers.truncate(block.indent as usize);
        }
        let text = md_inline(&block.text, opts);
        let text = wrap_align(text, block.align, opts);
        match &block.kind {
            BlockKind::Paragraph => {
                let _ = writeln!(out, "{text}\n");
            }
            BlockKind::Heading(level) => {
                // The card title is H1, so document headings start at H2.
                let hashes = "#".repeat((*level as usize + 1).min(6));
                let _ = writeln!(out, "{hashes} {text}\n");
            }
            BlockKind::BulletList => {
                let _ = writeln!(out, "{indent}- {text}");
            }
            BlockKind::NumberedList => {
                let level = block.indent as usize;
                numbers.resize(level + 1, 0);
                numbers.truncate(level + 1);
                numbers[level] += 1;
                let _ = writeln!(out, "{indent}{}. {text}", numbers[level]);
            }
            BlockKind::CheckList { checked } => {
                let mark = if *checked { "x" } else { " " };
                let _ = writeln!(out, "{indent}- [{mark}] {text}");
            }
            BlockKind::Quote => {
                for line in text.lines() {
                    let _ = writeln!(out, "> {line}");
                }
                out.push('\n');
            }
            BlockKind::Code { language } => {
                let fence = if block.text.text.contains("```") {
                    "````"
                } else {
                    "```"
                };
                let _ = writeln!(out, "{fence}{language}\n{}\n{fence}\n", block.text.text);
            }
            BlockKind::Divider => out.push_str("---\n\n"),
            BlockKind::Image(img) => {
                let src = if opts.image_links_relative {
                    format!("blobs/{}", img.blob)
                } else {
                    data_uri(blobs, &img.blob, &img.mime)
                };
                let _ = writeln!(out, "![{}]({src})\n", escape_md(&img.alt));
            }
            BlockKind::Attachment(att) => {
                let _ = writeln!(
                    out,
                    "[📎 {}](blobs/{})\n",
                    escape_md(&att.file_name),
                    att.blob
                );
            }
            BlockKind::Table(table) => {
                let cols = table.cols();
                for (r, row) in table.rows.iter().enumerate() {
                    let cells: Vec<String> = row
                        .iter()
                        .map(|c| md_inline(c, opts).replace('|', "\\|"))
                        .collect();
                    let _ = writeln!(out, "| {} |", cells.join(" | "));
                    if r == 0 {
                        let _ = writeln!(out, "|{}", " --- |".repeat(cols));
                    }
                }
                out.push('\n');
            }
        }
        prev_list = is_list;
        let _ = i;
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn wrap_align(text: String, align: Align, opts: &MarkdownOptions) -> String {
    if !opts.html_for_extras || align == Align::Left || text.is_empty() {
        return text;
    }
    format!("<div align=\"{}\">{text}</div>", align_name(align))
}

fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(
            ch,
            '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '#' | '~' | '|'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn md_inline(rt: &RichText, opts: &MarkdownOptions) -> String {
    let mut out = String::new();
    for (range, style) in rt.spans() {
        let raw = &rt.text[range];
        if raw.is_empty() {
            continue;
        }
        // Markdown emphasis must hug non-space characters: move surrounding spaces out.
        let trimmed = raw.trim();
        let lead = &raw[..raw.len() - raw.trim_start().len()];
        let trail = &raw[raw.trim_end().len()..];
        if trimmed.is_empty() {
            out.push_str(raw);
            continue;
        }
        let mut s = if style.code {
            let ticks = if trimmed.contains('`') { "``" } else { "`" };
            format!("{ticks}{trimmed}{ticks}")
        } else {
            escape_md(trimmed)
        };
        if style.bold && style.italic {
            s = format!("***{s}***");
        } else if style.bold {
            s = format!("**{s}**");
        } else if style.italic {
            s = format!("*{s}*");
        }
        if style.strike {
            s = format!("~~{s}~~");
        }
        if opts.html_for_extras {
            if style.underline {
                s = format!("<u>{s}</u>");
            }
            let css = extra_css(style);
            if !css.is_empty() {
                s = format!("<span style=\"{css}\">{s}</span>");
            }
        }
        if let Some(link) = style.link.as_ref().filter(|l| is_safe_link(l)) {
            s = format!("[{s}]({})", link.replace(' ', "%20").replace(')', "%29"));
        }
        out.push_str(lead);
        out.push_str(&s);
        out.push_str(trail);
    }
    out
}

fn extra_css(style: &Style) -> String {
    let mut css = String::new();
    if let Some(c) = &style.color {
        let _ = write!(css, "color:{};", sanitize_color(c));
    }
    if let Some(c) = &style.highlight {
        let _ = write!(css, "background-color:{};", sanitize_color(c));
    }
    css
}

/// Accept only `#rgb`/`#rrggbb` so exported HTML can't be injected through colors.
fn sanitize_color(c: &str) -> &str {
    let ok = c.starts_with('#')
        && matches!(c.len(), 4 | 7)
        && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if ok { c } else { "inherit" }
}

fn data_uri(blobs: &dyn BlobResolver, id: &str, mime: &str) -> String {
    use base64::Engine as _;
    match blobs.resolve(id) {
        Some((m, bytes)) => format!(
            "data:{};base64,{}",
            if m.is_empty() { mime } else { &m },
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
        None => format!("blobs/{id}"),
    }
}

fn align_name(a: Align) -> &'static str {
    match a {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "justify",
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

/// Escape text for HTML element content and attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Whether a link target is safe to export or hand to the OS opener. Only web, mail
/// and in-page links pass; `javascript:`, `file:` and custom protocols could run code.
pub fn is_safe_link(link: &str) -> bool {
    let lower = link.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:", "#"]
        .iter()
        .any(|p| lower.starts_with(p))
}

fn safe_href(link: &str) -> String {
    if is_safe_link(link) {
        escape_html(link)
    } else {
        "#".into()
    }
}

fn html_inline(rt: &RichText) -> String {
    let mut out = String::new();
    for (range, style) in rt.spans() {
        let mut s = escape_html(&rt.text[range]).replace('\n', "<br>");
        if style.code {
            s = format!("<code>{s}</code>");
        }
        if style.bold {
            s = format!("<strong>{s}</strong>");
        }
        if style.italic {
            s = format!("<em>{s}</em>");
        }
        if style.underline {
            s = format!("<u>{s}</u>");
        }
        if style.strike {
            s = format!("<s>{s}</s>");
        }
        let css = extra_css(style);
        if !css.is_empty() {
            s = format!("<span style=\"{css}\">{s}</span>");
        }
        if let Some(link) = &style.link {
            s = format!("<a href=\"{}\">{s}</a>", safe_href(link));
        }
        out.push_str(&s);
    }
    out
}

fn block_attrs(block: &Block) -> String {
    let mut css = String::new();
    if block.align != Align::Left {
        let _ = write!(css, "text-align:{};", align_name(block.align));
    }
    if block.size != TextSize::Normal {
        let _ = write!(css, "font-size:{}em;", block.size.scale());
    }
    if block.indent > 0 && !block.kind.is_list() {
        let _ = write!(css, "margin-left:{}em;", block.indent as f32 * 1.5);
    }
    if css.is_empty() {
        String::new()
    } else {
        format!(" style=\"{css}\"")
    }
}

/// Export a document as a standalone HTML page with images embedded.
pub fn to_html(doc: &Document, title: &str, blobs: &dyn BlobResolver) -> String {
    let mut body = String::new();
    if !title.is_empty() {
        let _ = writeln!(body, "<h1>{}</h1>", escape_html(title));
    }
    // Open list stack: (tag, indent).
    let mut lists: Vec<&'static str> = Vec::new();
    let close_lists = |body: &mut String, lists: &mut Vec<&'static str>, keep: usize| {
        while lists.len() > keep {
            if let Some(tag) = lists.pop() {
                let _ = writeln!(body, "</li></{}>", tag_name(tag));
            }
        }
    };
    for block in &doc.blocks {
        let list_tag = match block.kind {
            BlockKind::BulletList => Some("ul"),
            BlockKind::NumberedList => Some("ol"),
            BlockKind::CheckList { .. } => Some("ul class=\"checklist\""),
            _ => None,
        };
        if let Some(tag) = list_tag {
            let depth = block.indent as usize + 1;
            if lists.len() > depth {
                close_lists(&mut body, &mut lists, depth);
            }
            if lists.len() == depth && lists.last() != Some(&tag) {
                close_lists(&mut body, &mut lists, depth - 1);
            }
            if lists.len() == depth {
                body.push_str("</li>");
            }
            while lists.len() < depth {
                let _ = write!(body, "<{tag}>");
                lists.push(tag);
            }
            let text = html_inline(&block.text);
            match block.kind {
                BlockKind::CheckList { checked } => {
                    let c = if checked { " checked" } else { "" };
                    let _ = write!(
                        body,
                        "<li{}><input type=\"checkbox\" disabled{c}> {text}",
                        block_attrs(block)
                    );
                }
                _ => {
                    let _ = write!(body, "<li{}>{text}", block_attrs(block));
                }
            }
            continue;
        }
        close_lists(&mut body, &mut lists, 0);
        let attrs = block_attrs(block);
        let text = html_inline(&block.text);
        match &block.kind {
            BlockKind::Paragraph => {
                let _ = writeln!(
                    body,
                    "<p{attrs}>{}</p>",
                    if text.is_empty() { "<br>" } else { &text }
                );
            }
            BlockKind::Heading(level) => {
                let l = (*level as usize + 1).min(6);
                let _ = writeln!(body, "<h{l}{attrs}>{text}</h{l}>");
            }
            BlockKind::Quote => {
                let _ = writeln!(body, "<blockquote{attrs}>{text}</blockquote>");
            }
            BlockKind::Code { language } => {
                let _ = writeln!(
                    body,
                    "<pre><code class=\"language-{}\">{}</code></pre>",
                    escape_html(language),
                    escape_html(&block.text.text)
                );
            }
            BlockKind::Divider => body.push_str("<hr>\n"),
            BlockKind::Image(img) => {
                let src = data_uri(blobs, &img.blob, &img.mime);
                let width = img
                    .width
                    .map(|w| format!(" width=\"{w}\""))
                    .unwrap_or_default();
                let _ = writeln!(
                    body,
                    "<p{attrs}><img src=\"{}\" alt=\"{}\"{width}></p>",
                    escape_html(&src),
                    escape_html(&img.alt)
                );
            }
            BlockKind::Attachment(att) => {
                let _ = writeln!(
                    body,
                    "<p class=\"attachment\">📎 {} ({} bytes)</p>",
                    escape_html(&att.file_name),
                    att.size
                );
            }
            BlockKind::Table(table) => {
                body.push_str("<table>\n");
                for (r, row) in table.rows.iter().enumerate() {
                    let cell = if r == 0 && table.header { "th" } else { "td" };
                    body.push_str("<tr>");
                    for c in row {
                        let _ = write!(body, "<{cell}>{}</{cell}>", html_inline(c));
                    }
                    body.push_str("</tr>\n");
                }
                body.push_str("</table>\n");
            }
            BlockKind::BulletList | BlockKind::NumberedList | BlockKind::CheckList { .. } => {}
        }
    }
    close_lists(&mut body, &mut lists, 0);

    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"Cardfile\">\n<title>{}</title>\n<style>{CSS}</style>\n\
         </head>\n<body>\n{body}</body>\n</html>\n",
        escape_html(title)
    )
}

fn tag_name(tag: &'static str) -> &'static str {
    tag.split(' ').next().unwrap_or(tag)
}

const CSS: &str = "body{font-family:system-ui,-apple-system,'Segoe UI',sans-serif;max-width:48rem;\
margin:2rem auto;padding:0 1rem;line-height:1.6;color:#1f2328}\
pre{background:#f6f8fa;padding:1rem;border-radius:6px;overflow:auto}\
code{font-family:ui-monospace,Consolas,monospace;background:#f6f8fa;padding:.1em .3em;border-radius:4px}\
pre code{padding:0;background:none}blockquote{border-left:4px solid #d0d7de;margin:0;padding:0 1rem;color:#59636e}\
table{border-collapse:collapse}th,td{border:1px solid #d0d7de;padding:.4rem .8rem}th{background:#f6f8fa}\
ul.checklist{list-style:none;padding-left:1.2rem}img{max-width:100%}hr{border:0;border-top:1px solid #d0d7de}";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Mark, Table};

    fn sample() -> Document {
        let mut doc = Document {
            version: 1,
            blocks: vec![],
        };
        doc.blocks.push(Block::heading(1, "Intro"));
        let mut p = Block::paragraph("Hello bold world");
        p.text.toggle_mark(6..10, Mark::Bold);
        doc.blocks.push(p);
        doc.blocks.push(Block::new(BlockKind::BulletList, "one"));
        let mut nested = Block::new(BlockKind::BulletList, "nested");
        nested.indent = 1;
        doc.blocks.push(nested);
        doc.blocks
            .push(Block::new(BlockKind::NumberedList, "first"));
        doc.blocks
            .push(Block::new(BlockKind::NumberedList, "second"));
        doc.blocks
            .push(Block::new(BlockKind::CheckList { checked: true }, "done"));
        doc.blocks.push(Block::new(
            BlockKind::Code {
                language: "rust".into(),
            },
            "let x = 1;",
        ));
        let mut table = Table::new(2, 2);
        table.rows[0][0] = "A".into();
        table.rows[0][1] = "B".into();
        table.rows[1][0] = "1".into();
        table.rows[1][1] = "2".into();
        doc.blocks.push(Block::new(BlockKind::Table(table), ""));
        doc
    }

    #[test]
    fn markdown_export() {
        let md = to_markdown(
            &sample(),
            Some("Card"),
            &MarkdownOptions::default(),
            &NoBlobs,
        );
        let expected = "# Card\n\n## Intro\n\nHello **bold** world\n\n- one\n   - nested\n1. first\n2. second\n- [x] done\n\n```rust\nlet x = 1;\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n";
        pretty_assertions::assert_eq!(md, expected);
    }

    #[test]
    fn plain_text_export() {
        let text = to_plain_text(&sample());
        assert!(text.starts_with("Intro\nHello bold world\none"));
        assert!(text.contains("A\tB"));
    }

    #[test]
    fn html_escapes_and_blocks_scripts() {
        let mut doc = Document::from_plain_text("<script>alert(1)</script>");
        doc.blocks[0]
            .text
            .update_style(0..8, |s| s.link = Some("javascript:alert(1)".into()));
        let html = to_html(&doc, "T", &NoBlobs);
        assert!(!html.contains("<script>"));
        assert!(!html.contains("javascript:"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn html_nested_lists_balance() {
        let html = to_html(&sample(), "T", &NoBlobs);
        assert_eq!(html.matches("<ul").count(), html.matches("</ul>").count());
        assert_eq!(html.matches("<ol>").count(), html.matches("</ol>").count());
    }
}
