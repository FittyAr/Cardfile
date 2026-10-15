//! The card document: a flat list of blocks, each holding rich text.
//!
//! Lists are expressed with an indent level instead of nesting (as Google Docs and
//! Word do), which keeps editing operations simple: every caret position is a
//! `(block index, byte offset)` pair, except inside tables where a cell is addressed too.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Current serialization version of [`Document`].
pub const DOC_VERSION: u32 = 1;

/// Maximum list / paragraph indent level.
pub const MAX_INDENT: u8 = 8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    pub blocks: Vec<Block>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: DOC_VERSION,
            blocks: vec![Block::paragraph("")],
        }
    }
}

impl Document {
    /// A document with one paragraph per line of `text`.
    pub fn from_plain_text(text: &str) -> Self {
        let blocks: Vec<Block> = text.lines().map(Block::paragraph).collect();
        if blocks.is_empty() {
            Self::default()
        } else {
            Self {
                version: DOC_VERSION,
                blocks,
            }
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Parse a stored document. Empty input yields an empty document.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        if json.trim().is_empty() {
            return Ok(Self::default());
        }
        let mut doc: Self = serde_json::from_str(json)?;
        doc.normalize();
        Ok(doc)
    }

    /// Restore invariants: at least one block, merged runs, valid run lengths.
    pub fn normalize(&mut self) {
        if self.blocks.is_empty() {
            self.blocks.push(Block::paragraph(""));
        }
        for block in &mut self.blocks {
            block.text.normalize();
            if let BlockKind::Table(table) = &mut block.kind {
                table.normalize();
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks
            .iter()
            .all(|b| b.text.is_empty() && b.kind.is_textual())
    }

    /// IDs of all blobs referenced by images and attachments.
    /// Rewrite image and attachment blob references; `f` returns the new reference or
    /// `None` to keep the old one.
    pub fn map_blob_refs<E>(
        &mut self,
        mut f: impl FnMut(&str) -> Result<Option<String>, E>,
    ) -> Result<(), E> {
        for block in &mut self.blocks {
            let blob = match &mut block.kind {
                BlockKind::Image(img) => &mut img.blob,
                BlockKind::Attachment(att) => &mut att.blob,
                _ => continue,
            };
            if let Some(new) = f(blob)? {
                *blob = new;
            }
        }
        Ok(())
    }

    pub fn blob_ids(&self) -> Vec<String> {
        self.blocks
            .iter()
            .filter_map(|b| match &b.kind {
                BlockKind::Image(img) => Some(img.blob.clone()),
                BlockKind::Attachment(att) => Some(att.blob.clone()),
                _ => None,
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub kind: BlockKind,
    #[serde(default, skip_serializing_if = "is_default")]
    pub align: Align,
    #[serde(default, skip_serializing_if = "is_default")]
    pub indent: u8,
    /// Text size of the whole block (paragraphs, lists, quotes).
    #[serde(default, skip_serializing_if = "is_default")]
    pub size: TextSize,
    /// Text of textual blocks; empty for dividers, images, attachments and tables.
    #[serde(default, skip_serializing_if = "RichText::is_empty")]
    pub text: RichText,
}

impl Block {
    pub fn new(kind: BlockKind, text: impl Into<RichText>) -> Self {
        Self {
            kind,
            align: Align::Left,
            indent: 0,
            size: TextSize::Normal,
            text: text.into(),
        }
    }

    pub fn paragraph(text: &str) -> Self {
        Self::new(BlockKind::Paragraph, text)
    }

    pub fn heading(level: u8, text: &str) -> Self {
        Self::new(BlockKind::Heading(level.clamp(1, 3)), text)
    }
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum BlockKind {
    Paragraph,
    /// Level 1 to 3.
    Heading(u8),
    BulletList,
    /// Numbering restarts after any non-numbered block or indent change.
    NumberedList,
    CheckList {
        checked: bool,
    },
    Quote,
    Code {
        language: String,
    },
    Divider,
    Image(Image),
    Attachment(Attachment),
    Table(Table),
}

impl BlockKind {
    /// Whether the block holds editable rich text in [`Block::text`].
    pub fn is_textual(&self) -> bool {
        !matches!(
            self,
            Self::Divider | Self::Image(_) | Self::Attachment(_) | Self::Table(_)
        )
    }

    pub fn is_list(&self) -> bool {
        matches!(
            self,
            Self::BulletList | Self::NumberedList | Self::CheckList { .. }
        )
    }

    /// Code blocks hold plain text only.
    pub fn allows_marks(&self) -> bool {
        !matches!(self, Self::Code { .. })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    /// BLAKE3 hex id of the image bytes in the `blobs` table.
    pub blob: String,
    pub mime: String,
    /// Display width in logical pixels; `None` fits the editor width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    /// Display height in logical pixels when the image was stretched; `None` keeps
    /// the aspect ratio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub blob: String,
    pub file_name: String,
    pub mime: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Row-major cells. All rows have the same length.
    pub rows: Vec<Vec<RichText>>,
    /// The first row is a header.
    #[serde(default)]
    pub header: bool,
}

impl Table {
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        Self {
            rows: vec![vec![RichText::default(); cols]; rows],
            header: true,
        }
    }

    pub fn cols(&self) -> usize {
        self.rows.first().map_or(0, Vec::len)
    }

    pub fn normalize(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(vec![RichText::default()]);
        }
        let cols = self.rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for row in &mut self.rows {
            row.resize(cols, RichText::default());
            for cell in row {
                cell.normalize();
            }
        }
    }
}

/// A color as `#rrggbb`.
pub type Color = String;

/// Relative text size. A fixed scale keeps documents consistent ("Word without excess").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSize {
    Small,
    #[default]
    Normal,
    Large,
    Huge,
}

impl TextSize {
    pub fn scale(self) -> f32 {
        match self {
            Self::Small => 0.85,
            Self::Normal => 1.0,
            Self::Large => 1.25,
            Self::Huge => 1.6,
        }
    }
}

/// Inline formatting applied to a run of text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub strike: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub code: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}

/// A toggleable boolean mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
}

impl Mark {
    pub fn get(self, s: &Style) -> bool {
        match self {
            Self::Bold => s.bold,
            Self::Italic => s.italic,
            Self::Underline => s.underline,
            Self::Strike => s.strike,
            Self::Code => s.code,
        }
    }

    pub fn set(self, s: &mut Style, on: bool) {
        match self {
            Self::Bold => s.bold = on,
            Self::Italic => s.italic = on,
            Self::Underline => s.underline = on,
            Self::Strike => s.strike = on,
            Self::Code => s.code = on,
        }
    }
}

/// A run of `len` bytes sharing one [`Style`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub len: usize,
    #[serde(default, skip_serializing_if = "Style::is_plain")]
    pub style: Style,
}

/// Text plus style runs that cover it exactly.
///
/// Invariants (restored by [`RichText::normalize`]): run lengths sum to `text.len()`,
/// every run boundary is a char boundary, no empty runs, adjacent runs differ in style.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RichText {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<Run>,
}

impl From<&str> for RichText {
    fn from(s: &str) -> Self {
        Self::plain(s)
    }
}

impl From<String> for RichText {
    fn from(s: String) -> Self {
        Self::plain(&s)
    }
}

impl RichText {
    pub fn plain(text: &str) -> Self {
        let mut rt = Self {
            text: text.to_owned(),
            runs: Vec::new(),
        };
        rt.normalize();
        rt
    }

    pub fn styled(text: &str, style: Style) -> Self {
        let mut rt = Self {
            text: text.to_owned(),
            runs: vec![Run {
                len: text.len(),
                style,
            }],
        };
        rt.normalize();
        rt
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn normalize(&mut self) {
        let total: usize = self.runs.iter().map(|r| r.len).sum();
        if total != self.text.len() {
            // Corrupt or hand-written input: keep styles that fit, pad with plain text.
            let mut kept = Vec::new();
            let mut acc = 0;
            for run in self.runs.drain(..) {
                if acc + run.len > self.text.len() {
                    let len = self.text.len() - acc;
                    kept.push(Run {
                        len,
                        style: run.style,
                    });
                    acc += len;
                    break;
                }
                acc += run.len;
                kept.push(run);
            }
            if acc < self.text.len() {
                kept.push(Run {
                    len: self.text.len() - acc,
                    style: Style::default(),
                });
            }
            self.runs = kept;
        }
        // Snap run boundaries to char boundaries.
        let mut pos = 0;
        for run in &mut self.runs {
            let mut end = (pos + run.len).min(self.text.len());
            while end < self.text.len() && !self.text.is_char_boundary(end) {
                end += 1;
            }
            run.len = end.saturating_sub(pos);
            pos = end;
        }
        let mut merged: Vec<Run> = Vec::with_capacity(self.runs.len());
        for run in self.runs.drain(..) {
            if run.len == 0 {
                continue;
            }
            match merged.last_mut() {
                Some(last) if last.style == run.style => last.len += run.len,
                _ => merged.push(run),
            }
        }
        if merged.len() == 1 && merged[0].style.is_plain() {
            merged.clear();
        }
        self.runs = merged;
    }

    /// Iterate `(byte range, style)` pairs covering the whole text.
    pub fn spans(&self) -> impl Iterator<Item = (Range<usize>, &Style)> {
        static PLAIN: std::sync::LazyLock<Style> = std::sync::LazyLock::new(Style::default);
        let plain_full = self.runs.is_empty() && !self.text.is_empty();
        let mut pos = 0;
        let runs = self.runs.iter().map(move |r| {
            let range = pos..pos + r.len;
            pos += r.len;
            (range, &r.style)
        });
        let plain = plain_full.then(|| (0..self.text.len(), &*PLAIN));
        plain.into_iter().chain(runs)
    }

    /// Style of the character before `offset` (or after it at the start), the style
    /// that newly typed text should inherit.
    pub fn style_at(&self, offset: usize) -> Style {
        let probe = if offset == 0 { 0 } else { offset - 1 };
        self.spans()
            .find(|(r, _)| r.contains(&probe))
            .map(|(_, s)| s.clone())
            .unwrap_or_default()
    }

    fn expand_runs(&self) -> Vec<Run> {
        if self.runs.is_empty() && !self.text.is_empty() {
            vec![Run {
                len: self.text.len(),
                style: Style::default(),
            }]
        } else {
            self.runs.clone()
        }
    }

    /// Insert `text` at byte `offset` with `style`.
    pub fn insert(&mut self, offset: usize, text: &str, style: &Style) {
        let offset = offset.min(self.text.len());
        if text.is_empty() {
            return;
        }
        let mut runs = Vec::new();
        let mut pos = 0;
        let mut inserted = false;
        for run in self.expand_runs() {
            let end = pos + run.len;
            if !inserted && offset >= pos && offset <= end {
                let left = offset - pos;
                if left > 0 {
                    runs.push(Run {
                        len: left,
                        style: run.style.clone(),
                    });
                }
                runs.push(Run {
                    len: text.len(),
                    style: style.clone(),
                });
                if run.len - left > 0 {
                    runs.push(Run {
                        len: run.len - left,
                        style: run.style,
                    });
                }
                inserted = true;
            } else {
                runs.push(run);
            }
            pos = end;
        }
        if !inserted {
            runs.push(Run {
                len: text.len(),
                style: style.clone(),
            });
        }
        self.text.insert_str(offset, text);
        self.runs = runs;
        self.normalize();
    }

    /// Remove the byte `range`.
    pub fn delete(&mut self, range: Range<usize>) {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        if start >= end {
            return;
        }
        let mut runs = Vec::new();
        let mut pos = 0;
        for run in self.expand_runs() {
            let (rs, re) = (pos, pos + run.len);
            let overlap = re.min(end).saturating_sub(rs.max(start));
            let len = run.len - overlap;
            if len > 0 {
                runs.push(Run {
                    len,
                    style: run.style,
                });
            }
            pos = re;
        }
        self.text.replace_range(start..end, "");
        self.runs = runs;
        self.normalize();
    }

    /// Split at `offset`, returning the tail.
    pub fn split_off(&mut self, offset: usize) -> RichText {
        let offset = offset.min(self.text.len());
        let tail = self.slice(offset..self.text.len());
        self.delete(offset..self.text.len());
        tail
    }

    /// Copy of the byte `range`.
    pub fn slice(&self, range: Range<usize>) -> RichText {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len()).max(start);
        let mut runs = Vec::new();
        for (r, style) in self.spans() {
            let s = r.start.max(start);
            let e = r.end.min(end);
            if e > s {
                runs.push(Run {
                    len: e - s,
                    style: style.clone(),
                });
            }
        }
        let mut rt = RichText {
            text: self.text[start..end].to_owned(),
            runs,
        };
        rt.normalize();
        rt
    }

    /// Append another rich text.
    pub fn append(&mut self, other: &RichText) {
        let mut runs = self.expand_runs();
        runs.extend(other.expand_runs());
        self.text.push_str(&other.text);
        self.runs = runs;
        self.normalize();
    }

    /// Apply `f` to the style of every character in `range`.
    pub fn update_style(&mut self, range: Range<usize>, mut f: impl FnMut(&mut Style)) {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        if start >= end {
            return;
        }
        let mut runs = Vec::new();
        for (r, style) in self.spans() {
            let cuts = [
                r.start,
                r.start.max(start).min(r.end),
                r.end.min(end).max(r.start),
                r.end,
            ];
            for w in cuts.windows(2) {
                let (s, e) = (w[0], w[1]);
                if e <= s {
                    continue;
                }
                let mut st = style.clone();
                if s >= start && e <= end {
                    f(&mut st);
                }
                runs.push(Run {
                    len: e - s,
                    style: st,
                });
            }
        }
        self.runs = runs;
        self.normalize();
    }

    /// Whether `mark` is set on every character of `range` (or at the caret if empty).
    pub fn has_mark(&self, range: Range<usize>, mark: Mark) -> bool {
        if range.is_empty() {
            return mark.get(&self.style_at(range.start));
        }
        self.spans()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .all(|(_, s)| mark.get(s))
    }

    /// Toggle `mark` over `range`: if every character has it, remove it; else add it.
    pub fn toggle_mark(&mut self, range: Range<usize>, mark: Mark) {
        let on = !self.has_mark(range.clone(), mark);
        self.update_style(range, |s| mark.set(s, on));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bold() -> Style {
        Style {
            bold: true,
            ..Default::default()
        }
    }

    #[test]
    fn insert_inherits_and_merges() {
        let mut rt = RichText::plain("hello world");
        rt.insert(5, ",", &Style::default());
        assert_eq!(rt.text, "hello, world");
        assert!(rt.runs.is_empty());
        rt.insert(0, "Oh ", &bold());
        assert_eq!(rt.runs.len(), 2);
        assert_eq!(
            rt.runs[0],
            Run {
                len: 3,
                style: bold()
            }
        );
    }

    #[test]
    fn toggle_mark_over_range() {
        let mut rt = RichText::plain("abcdef");
        rt.toggle_mark(1..3, Mark::Bold);
        assert!(rt.has_mark(1..3, Mark::Bold));
        assert!(!rt.has_mark(0..3, Mark::Bold));
        rt.toggle_mark(0..4, Mark::Bold);
        assert!(rt.has_mark(0..4, Mark::Bold));
        rt.toggle_mark(0..4, Mark::Bold);
        assert!(rt.runs.is_empty());
    }

    #[test]
    fn delete_across_runs() {
        let mut rt = RichText::plain("abcdef");
        rt.toggle_mark(2..4, Mark::Italic);
        rt.delete(1..5);
        assert_eq!(rt.text, "af");
        assert!(rt.runs.is_empty());
    }

    #[test]
    fn split_and_append_roundtrip() {
        let mut rt = RichText::plain("héllo wörld");
        rt.toggle_mark(0..6, Mark::Bold);
        let original = rt.clone();
        let tail = rt.split_off(7);
        rt.append(&tail);
        assert_eq!(rt, original);
    }

    #[test]
    fn normalize_snaps_to_char_boundaries() {
        let mut rt = RichText {
            text: "ñb".into(),
            runs: vec![
                Run {
                    len: 1,
                    style: bold(),
                },
                Run {
                    len: 2,
                    style: Style::default(),
                },
            ],
        };
        rt.normalize();
        assert_eq!(rt.runs[0].len, 2);
        assert_eq!(rt.runs.iter().map(|r| r.len).sum::<usize>(), 3);
    }

    #[test]
    fn json_roundtrip() {
        let mut doc = Document::default();
        doc.blocks.push(Block::heading(2, "Title"));
        doc.blocks
            .push(Block::new(BlockKind::CheckList { checked: true }, "done"));
        doc.blocks
            .push(Block::new(BlockKind::Table(Table::new(2, 2)), ""));
        doc.blocks[0].text.toggle_mark(0..0, Mark::Bold);
        let json = doc.to_json();
        assert_eq!(Document::from_json(&json).unwrap(), doc);
    }
}
