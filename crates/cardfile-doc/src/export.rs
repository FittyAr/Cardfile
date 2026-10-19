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
        // Line breaks inside a block become hard breaks (a trailing backslash); a
        // continuation line is indented to stay inside its list item or quote.
        let lines = md_lines(&block.text, opts);
        let text = |cont: &str| {
            let joined = lines.join(&format!("\\\n{cont}"));
            wrap_align(joined, block.align, opts)
        };
        match &block.kind {
            BlockKind::Paragraph => {
                let _ = writeln!(out, "{}\n", text(""));
            }
            BlockKind::Heading(level) => {
                // The card title is H1, so document headings start at H2 (the
                // Markdown import maps them back, see `markdown_in`). A heading is
                // a single line in Markdown, so its line breaks become spaces.
                let hashes = "#".repeat((*level as usize + 1).min(6));
                let text = wrap_align(lines.join(" "), block.align, opts);
                let _ = writeln!(out, "{hashes} {text}\n");
            }
            BlockKind::BulletList => {
                let _ = writeln!(out, "{indent}- {}", text(&format!("{indent}  ")));
            }
            BlockKind::NumberedList => {
                let level = block.indent as usize;
                numbers.resize(level + 1, 0);
                numbers.truncate(level + 1);
                numbers[level] += 1;
                let marker = format!("{}. ", numbers[level]);
                let cont = format!("{indent}{}", " ".repeat(marker.len()));
                let _ = writeln!(out, "{indent}{marker}{}", text(&cont));
            }
            BlockKind::CheckList { checked } => {
                let mark = if *checked { "x" } else { " " };
                let _ = writeln!(out, "{indent}- [{mark}] {}", text(&format!("{indent}  ")));
            }
            BlockKind::Quote => {
                let _ = writeln!(out, "> {}\n", text("> "));
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
                    format!("blobs/{}", blob_file_name(&img.blob))
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
                    blob_file_name(&att.blob)
                );
            }
            BlockKind::Table(table) => {
                let cols = table.cols();
                for (r, row) in table.rows.iter().enumerate() {
                    let cells: Vec<String> = row.iter().map(|c| md_cell(c, opts)).collect();
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

/// Escape inline Markdown syntax in plain text.
fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, ch) in s.char_indices() {
        let escape = match ch {
            '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '#' | '~' | '|' => true,
            // `&amp;`, `&#38;`, `&#x26;` would be decoded as character references.
            '&' => looks_like_entity(&s[i + 1..]),
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Whether `rest` (the text after a `&`) starts like an HTML character reference.
fn looks_like_entity(rest: &str) -> bool {
    let Some(end) = rest.find(';') else {
        return false;
    };
    let body = &rest[..end];
    let valid = if let Some(num) = body.strip_prefix('#') {
        if let Some(hex) = num.strip_prefix(['x', 'X']) {
            !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())
        } else {
            !num.is_empty() && num.chars().all(|c| c.is_ascii_digit())
        }
    } else {
        !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric())
    };
    valid && body.len() <= 32
}

/// Escape what would start a block when it begins a line of a paragraph: list
/// markers, thematic breaks, setext underlines and indented code.
fn escape_line_start(line: &str) -> String {
    let mut chars = line.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let second = chars.next();
    let blank_after = |c: Option<char>| matches!(c, None | Some(' ' | '\t'));
    match first {
        // Leading spaces would be dropped or turn the line into indented code; a
        // character reference keeps them.
        ' ' => return format!("&#32;{}", &line[1..]),
        '\t' => return format!("&#9;{}", &line[1..]),
        '-' | '+' if blank_after(second) => return format!("\\{line}"),
        '-' | '=' => {
            let t = line.trim_end();
            let rule = first == '-'
                && t.chars().all(|c| matches!(c, '-' | ' ' | '\t'))
                && t.matches('-').count() >= 3;
            let setext = first == '=' && t.chars().all(|c| c == '=');
            if rule || setext {
                return format!("\\{line}");
            }
        }
        '0'..='9' => {
            let digits = line.bytes().take_while(u8::is_ascii_digit).count();
            let rest = &line[digits..];
            let mut r = rest.chars();
            if digits <= 9 && matches!(r.next(), Some('.' | ')')) && blank_after(r.next()) {
                return format!("{}\\{rest}", &line[..digits]);
            }
        }
        _ => {}
    }
    line.to_owned()
}

/// A block's inline Markdown split at its line breaks, each line escaped so it
/// cannot start a new block. Trailing empty lines are dropped (Markdown can't keep
/// them).
fn md_lines(rt: &RichText, opts: &MarkdownOptions) -> Vec<String> {
    let inline = md_inline(rt, opts, false);
    let mut lines: Vec<&str> = inline.split('\n').collect();
    while lines.len() > 1 && lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.into_iter().map(escape_line_start).collect()
}

/// A table cell: GFM cells are one line, so line breaks become `<br>`.
fn md_cell(rt: &RichText, opts: &MarkdownOptions) -> String {
    md_inline(rt, opts, true)
        .split('\n')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("<br>")
}

fn md_inline(rt: &RichText, opts: &MarkdownOptions, in_table: bool) -> String {
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
            // A code span can't hold a line break: one span per line.
            trimmed
                .split('\n')
                .map(|part| {
                    let part = part.trim();
                    // In a table an unescaped `|` would end the cell, even in code.
                    let part = if in_table {
                        part.replace('|', "\\|")
                    } else {
                        part.to_owned()
                    };
                    if part.is_empty() {
                        part
                    } else {
                        format!("{ticks}{part}{ticks}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
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

/// File name of a blob in an export folder: its storage id, never a sealed blob's key.
pub fn blob_file_name(blob_ref: &str) -> &str {
    cardfile_core::BlobRef::parse(blob_ref).id
}

fn data_uri(blobs: &dyn BlobResolver, id: &str, mime: &str) -> String {
    use base64::Engine as _;
    match blobs.resolve(id) {
        // The document's MIME type wins: sealed blobs are stored as octet-stream.
        Some((m, bytes)) => format!(
            "data:{};base64,{}",
            if mime.is_empty() { &m } else { mime },
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ),
        None => format!("blobs/{}", blob_file_name(id)),
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
    // Let the browser pick each paragraph's direction (Hebrew, Arabic, …).
    let dir = if block.kind.is_textual() && crate::bidi::is_rtl_paragraph(&block.text.text) {
        " dir=\"rtl\""
    } else {
        ""
    };
    if css.is_empty() {
        dir.to_owned()
    } else {
        format!("{dir} style=\"{css}\"")
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
                    .unwrap_or_default()
                    + &img
                        .height
                        .map(|h| format!(" height=\"{h}\""))
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

    fn md(doc: &Document) -> String {
        to_markdown(doc, None, &MarkdownOptions::default(), &NoBlobs)
    }

    fn doc_of(blocks: Vec<Block>) -> Document {
        Document {
            version: crate::DOC_VERSION,
            blocks,
        }
    }

    #[test]
    fn markdown_keeps_line_breaks() {
        let mut table = Table::new(2, 1);
        table.rows[0][0] = "H".into();
        table.rows[1][0] = "one\ntwo".into();
        let doc = doc_of(vec![
            Block::paragraph("first\nsecond"),
            Block::heading(1, "split\nheading"),
            Block::new(BlockKind::BulletList, "item\nmore"),
            Block::new(BlockKind::NumberedList, "num\nmore"),
            Block::new(BlockKind::Quote, "q1\nq2"),
            Block::new(BlockKind::Table(table), ""),
        ]);
        let expected = "first\\\nsecond\n\n## split heading\n\n- item\\\n  more\n1. num\\\n   more\n\n> q1\\\n> q2\n\n| H |\n| --- |\n| one<br>two |\n";
        pretty_assertions::assert_eq!(md(&doc), expected);
        // And back: every line break survives the round trip.
        let back = crate::from_markdown(expected);
        assert_eq!(back.blocks[0].text.text, "first\nsecond");
        assert_eq!(back.blocks[2].text.text, "item\nmore");
        assert_eq!(back.blocks[3].text.text, "num\nmore");
        assert_eq!(back.blocks[4].text.text, "q1\nq2");
        let BlockKind::Table(t) = &back.blocks[5].kind else {
            panic!("table expected")
        };
        assert_eq!(t.rows[1][0].text, "one\ntwo");
    }

    #[test]
    fn markdown_code_span_with_line_break() {
        let mut p = Block::paragraph("a\nb");
        p.text.toggle_mark(0..3, Mark::Code);
        pretty_assertions::assert_eq!(md(&doc_of(vec![p])), "`a`\\\n`b`\n");
    }

    #[test]
    fn markdown_escapes_block_starts() {
        let texts = [
            "- not a list",
            "+ plus",
            "* star",
            "1. one",
            "2) two",
            "---",
            "***",
            "___",
            "# hash",
            "> quote",
            "    four spaces",
            "&amp; and &#38; and &#x26;",
            "-",
            "line\n===",
            "line\n- dash",
            "a & b; c",
        ];
        let doc = doc_of(texts.iter().map(|t| Block::paragraph(t)).collect());
        let out = md(&doc);
        let expected = "\\- not a list\n\n\\+ plus\n\n\\* star\n\n1\\. one\n\n2\\) two\n\n\\---\n\n\\*\\*\\*\n\n\\_\\_\\_\n\n\\# hash\n\n\\> quote\n\n&#32;   four spaces\n\n\\&amp; and \\&\\#38; and \\&\\#x26;\n\n\\-\n\nline\\\n\\===\n\nline\\\n\\- dash\n\na & b; c\n";
        pretty_assertions::assert_eq!(out, expected);
        // Every paragraph comes back as the same plain paragraph.
        let back = crate::from_markdown(&out);
        let got: Vec<_> = back
            .blocks
            .iter()
            .map(|b| (b.kind.clone(), b.text.text.clone()))
            .collect();
        let want: Vec<_> = texts
            .iter()
            .map(|t| (BlockKind::Paragraph, (*t).to_owned()))
            .collect();
        pretty_assertions::assert_eq!(got, want);
    }

    #[test]
    fn markdown_escapes_list_item_starts() {
        let doc = doc_of(vec![Block::new(BlockKind::BulletList, "- nested?")]);
        let out = md(&doc);
        assert_eq!(out, "- \\- nested?\n");
        let back = crate::from_markdown(&out);
        assert_eq!(back.blocks.len(), 1);
        assert_eq!(back.blocks[0].text.text, "- nested?");
    }

    #[test]
    fn markdown_table_pipes_round_trip() {
        let mut table = Table::new(2, 1);
        table.rows[0][0] = "H".into();
        table.rows[1][0] = "a|b".into();
        let mut code = Table::new(2, 1);
        code.rows[0][0] = "H".into();
        code.rows[1][0] = "x|y".into();
        code.rows[1][0].toggle_mark(0..3, Mark::Code);
        let doc = doc_of(vec![
            Block::new(BlockKind::Table(table), ""),
            Block::new(BlockKind::Table(code), ""),
        ]);
        let back = crate::from_markdown(&md(&doc));
        for (i, want) in ["a|b", "x|y"].into_iter().enumerate() {
            let BlockKind::Table(t) = &back.blocks[i].kind else {
                panic!("table expected")
            };
            assert_eq!(t.rows[1][0].text, want);
        }
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
