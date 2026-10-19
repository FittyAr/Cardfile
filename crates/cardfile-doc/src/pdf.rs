//! PDF export with `krilla`.
//!
//! krilla draws glyphs but does no layout, so this module does its own line breaking
//! (measuring with `harfrust`, the maintained HarfBuzz port) and pagination.
//! Fonts come from the operating system so nothing has to be bundled.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use krilla::Document as PdfDocument;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image as PdfImage;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, TextDirection};

use crate::export::BlobResolver;
use crate::model::{Align, Block, BlockKind, Document, RichText, Style, TextSize};

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("no usable font found on this system")]
    NoFont,
    #[error("PDF generation failed: {0}")]
    Pdf(String),
}

const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 56.0;
const BODY: f32 = 11.0;
const INDENT: f32 = 18.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
    Mono,
    /// Covers scripts the main fonts lack (Chinese, Japanese, Korean, …).
    Fallback,
}

struct LoadedFont {
    shaper: harfrust::Font,
    font: Font,
    upem: f32,
    ascent: f32,
}

struct Fonts {
    faces: Vec<(Face, LoadedFont)>,
}

/// Candidate font files per face, in order of preference.
fn candidates(face: Face) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        dirs.push(Path::new(&windir).join("Fonts"));
    }
    for d in [
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/dejavu",
        "/usr/share/fonts/TTF",
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/liberation",
        "/usr/share/fonts/truetype/noto",
        "/usr/share/fonts/noto",
        "/usr/share/fonts/opentype/noto",
        "/usr/share/fonts/noto-cjk",
        "/usr/share/fonts/google-noto-cjk",
        "/usr/share/fonts/truetype/wqy",
        "/usr/share/fonts/wenquanyi/wqy-microhei",
        "/System/Library/Fonts/Supplemental",
    ] {
        dirs.push(PathBuf::from(d));
    }
    let names: &[&str] = match face {
        Face::Regular => &[
            "segoeui.ttf",
            "arial.ttf",
            "DejaVuSans.ttf",
            "LiberationSans-Regular.ttf",
            "NotoSans-Regular.ttf",
            "Arial.ttf",
        ],
        Face::Bold => &[
            "segoeuib.ttf",
            "arialbd.ttf",
            "DejaVuSans-Bold.ttf",
            "LiberationSans-Bold.ttf",
            "NotoSans-Bold.ttf",
            "Arial Bold.ttf",
        ],
        Face::Italic => &[
            "segoeuii.ttf",
            "ariali.ttf",
            "DejaVuSans-Oblique.ttf",
            "LiberationSans-Italic.ttf",
            "NotoSans-Italic.ttf",
            "Arial Italic.ttf",
        ],
        Face::BoldItalic => &[
            "segoeuiz.ttf",
            "arialbi.ttf",
            "DejaVuSans-BoldOblique.ttf",
            "LiberationSans-BoldItalic.ttf",
            "NotoSans-BoldItalic.ttf",
            "Arial Bold Italic.ttf",
        ],
        Face::Mono => &[
            "consola.ttf",
            "cour.ttf",
            "DejaVuSansMono.ttf",
            "LiberationMono-Regular.ttf",
            "NotoSansMono-Regular.ttf",
            "Courier New.ttf",
        ],
        Face::Fallback => &[
            "msyh.ttc",
            "YuGothR.ttc",
            "msgothic.ttc",
            "malgun.ttf",
            "NotoSansCJK-Regular.ttc",
            "NotoSansCJKsc-Regular.otf",
            "NotoSansSC-Regular.otf",
            "wqy-microhei.ttc",
            "DroidSansFallbackFull.ttf",
        ],
    };
    names
        .iter()
        .flat_map(|n| dirs.iter().map(move |d| d.join(n)))
        .collect()
}

/// The bundled font for a face, so PDFs look the same everywhere.
fn bundled(face: Face) -> Option<&'static [u8]> {
    use crate::fonts::*;
    Some(match face {
        Face::Regular => INTER_REGULAR,
        Face::Bold => INTER_BOLD,
        Face::Italic => INTER_ITALIC,
        Face::BoldItalic => INTER_BOLD_ITALIC,
        Face::Mono => MONO_REGULAR,
        Face::Fallback => return None,
    })
}

fn load(face: Face) -> Option<LoadedFont> {
    let embedded = bundled(face).map(<[u8]>::to_vec);
    let files = candidates(face)
        .into_iter()
        .filter_map(|p| std::fs::read(p).ok());
    for bytes in embedded.into_iter().chain(files) {
        let Some(shaper) = harfrust::Font::new(bytes.clone(), 0) else {
            continue;
        };
        let upem = f32::from(shaper.units_per_em().max(1));
        // Ascent relative to the font size; the exact value only nudges the baseline.
        let ascent = 0.92;
        let bytes = Arc::new(bytes);
        let Some(font) = Font::new(bytes.into(), 0) else {
            continue;
        };
        return Some(LoadedFont {
            shaper,
            font,
            upem,
            ascent,
        });
    }
    None
}

impl Fonts {
    fn load() -> Result<Self, PdfError> {
        let regular = load(Face::Regular).ok_or(PdfError::NoFont)?;
        let mut faces = Vec::new();
        for face in [
            Face::Bold,
            Face::Italic,
            Face::BoldItalic,
            Face::Mono,
            Face::Fallback,
        ] {
            if let Some(f) = load(face) {
                faces.push((face, f));
            }
        }
        faces.insert(0, (Face::Regular, regular));
        Ok(Self { faces })
    }

    fn get(&self, face: Face) -> &LoadedFont {
        let fallback = match face {
            Face::BoldItalic => Face::Bold,
            _ => Face::Regular,
        };
        self.faces
            .iter()
            .find(|(f, _)| *f == face)
            .or_else(|| self.faces.iter().find(|(f, _)| *f == fallback))
            .map(|(_, l)| l)
            .unwrap_or(&self.faces[0].1)
    }

    /// Whether `face` has a glyph for `ch` (shaping it does not give `.notdef`).
    fn covers(&self, face: Face, ch: char) -> bool {
        if ch.is_whitespace() || ch.is_control() {
            return true;
        }
        let f = self.get(face);
        let shaper = harfrust::ShaperFont::new(&f.shaper);
        let mut buf = harfrust::Buffer::new();
        let mut tmp = [0u8; 4];
        buf.push_str(ch.encode_utf8(&mut tmp));
        buf.guess_segment_properties();
        harfrust::shape(&shaper, &mut buf, harfrust::ShapeOptions::new()).is_ok()
            && buf.glyph_infos().iter().all(|g| g.glyph_id != 0)
    }

    /// Split `text` into runs drawn with `face`, or with the fallback font where
    /// `face` has no glyphs.
    fn runs(&self, face: Face, text: &str) -> Vec<(Face, String)> {
        let has_fallback = self.faces.iter().any(|(f, _)| *f == Face::Fallback);
        let mut out: Vec<(Face, String)> = Vec::new();
        for ch in text.chars() {
            let use_face =
                if has_fallback && !self.covers(face, ch) && self.covers(Face::Fallback, ch) {
                    Face::Fallback
                } else {
                    face
                };
            match out.last_mut() {
                Some((f, s)) if *f == use_face => s.push(ch),
                _ => out.push((use_face, ch.to_string())),
            }
        }
        out
    }

    fn width(&self, face: Face, text: &str, size: f32) -> f32 {
        let f = self.get(face);
        let shaper = harfrust::ShaperFont::new(&f.shaper);
        let mut buf = harfrust::Buffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        if harfrust::shape(&shaper, &mut buf, harfrust::ShapeOptions::new()).is_err() {
            return 0.;
        }
        buf.glyph_positions()
            .iter()
            .map(|p| p.x_advance as f32)
            .sum::<f32>()
            / f.upem
            * size
    }
}

/// A styled piece of a laid-out line.
#[derive(Clone, Debug)]
struct Piece {
    text: String,
    face: Face,
    size: f32,
    color: (u8, u8, u8),
    background: Option<(u8, u8, u8)>,
    underline: bool,
    strike: bool,
    width: f32,
}

#[derive(Clone, Debug, Default)]
struct Line {
    pieces: Vec<Piece>,
    width: f32,
}

fn hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

const TEXT: (u8, u8, u8) = (31, 35, 40);
const MUTED: (u8, u8, u8) = (101, 109, 118);
const LINK: (u8, u8, u8) = (9, 105, 218);
const RULE: (u8, u8, u8) = (208, 215, 222);
const CODE_BG: (u8, u8, u8) = (246, 248, 250);

struct BlockStyle {
    size: f32,
    bold: bool,
    italic: bool,
    mono: bool,
    color: (u8, u8, u8),
    strike: bool,
}

fn face_for(bold: bool, italic: bool, mono: bool) -> Face {
    match (mono, bold, italic) {
        (true, _, _) => Face::Mono,
        (_, true, true) => Face::BoldItalic,
        (_, true, false) => Face::Bold,
        (_, false, true) => Face::Italic,
        _ => Face::Regular,
    }
}

/// Break rich text into lines no wider than `max`.
fn layout_text(fonts: &Fonts, text: &RichText, bs: &BlockStyle, max: f32) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    let spans: Vec<(std::ops::Range<usize>, Style)> =
        text.spans().map(|(r, s)| (r, s.clone())).collect();
    for (range, style) in spans {
        let face = face_for(
            bs.bold || style.bold,
            bs.italic || style.italic,
            bs.mono || style.code,
        );
        let color = style
            .color
            .as_deref()
            .and_then(hex)
            .unwrap_or(if style.link.is_some() { LINK } else { bs.color });
        let background = style
            .highlight
            .as_deref()
            .and_then(hex)
            .or((style.code && !bs.mono).then_some(CODE_BG));
        let base = Piece {
            text: String::new(),
            face,
            size: bs.size,
            color,
            background,
            underline: style.underline || style.link.is_some(),
            strike: style.strike || bs.strike,
            width: 0.,
        };
        let segment = &text.text[range];
        for (i, hard_line) in segment.split('\n').enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }
            // Split into words keeping trailing spaces attached.
            let mut words: Vec<&str> = Vec::new();
            let mut start = 0;
            for (idx, ch) in hard_line.char_indices() {
                if ch == ' ' {
                    words.push(&hard_line[start..idx + 1]);
                    start = idx + 1;
                }
            }
            if start < hard_line.len() {
                words.push(&hard_line[start..]);
            }
            for word in words {
                let runs = fonts.runs(face, word);
                let w: f32 = runs.iter().map(|(f, s)| fonts.width(*f, s, bs.size)).sum();
                let line = lines.last_mut().unwrap_or_else(|| unreachable!());
                if line.width + w > max && line.width > 0. && !word.trim().is_empty() {
                    lines.push(Line::default());
                }
                // A single word longer than the line (or a run of CJK text, which has
                // no spaces) is broken by characters.
                if w > max {
                    for (run_face, run) in &runs {
                        let piece_base = Piece {
                            face: *run_face,
                            ..base.clone()
                        };
                        for ch in run.chars() {
                            let s = ch.to_string();
                            let cw = fonts.width(*run_face, &s, bs.size);
                            let line = lines.last_mut().unwrap_or_else(|| unreachable!());
                            if line.width + cw > max && line.width > 0. {
                                lines.push(Line::default());
                            }
                            push_piece(
                                lines.last_mut().unwrap_or_else(|| unreachable!()),
                                &piece_base,
                                &s,
                                cw,
                            );
                        }
                    }
                    continue;
                }
                for (run_face, run) in runs {
                    let rw = fonts.width(run_face, &run, bs.size);
                    let piece_base = Piece {
                        face: run_face,
                        ..base.clone()
                    };
                    push_piece(
                        lines.last_mut().unwrap_or_else(|| unreachable!()),
                        &piece_base,
                        &run,
                        rw,
                    );
                }
            }
        }
    }
    lines
}

fn push_piece(line: &mut Line, base: &Piece, text: &str, width: f32) {
    if let Some(last) = line.pieces.last_mut()
        && last.face == base.face
        && last.color == base.color
        && last.background == base.background
        && last.underline == base.underline
        && last.strike == base.strike
    {
        last.text.push_str(text);
        last.width += width;
    } else {
        line.pieces.push(Piece {
            text: text.to_owned(),
            width,
            ..base.clone()
        });
    }
    line.width += width;
}

fn fill(c: (u8, u8, u8)) -> Fill {
    Fill {
        paint: rgb::Color::new(c.0, c.1, c.2).into(),
        opacity: NormalizedF32::ONE,
        rule: Default::default(),
    }
}

fn stroke(c: (u8, u8, u8), width: f32) -> Stroke {
    Stroke {
        paint: rgb::Color::new(c.0, c.1, c.2).into(),
        width,
        ..Default::default()
    }
}

/// Page-by-page writer with a vertical cursor.
struct Writer<'a> {
    doc: PdfDocument,
    fonts: &'a Fonts,
    /// Draw commands for the current page; replayed on page finish.
    ops: Vec<Op>,
    y: f32,
}

enum Op {
    Text {
        x: f32,
        baseline: f32,
        piece: Piece,
    },
    Rect {
        rect: Rect,
        color: (u8, u8, u8),
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: (u8, u8, u8),
        width: f32,
    },
    Frame {
        rect: Rect,
        color: (u8, u8, u8),
        width: f32,
    },
    Image {
        image: PdfImage,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

impl<'a> Writer<'a> {
    fn new(fonts: &'a Fonts, title: &str) -> Self {
        let mut doc = PdfDocument::new();
        doc.set_metadata(
            Metadata::new()
                .title(title.to_owned())
                .creator("Cardfile".to_owned()),
        );
        Self {
            doc,
            fonts,
            ops: Vec::new(),
            y: MARGIN,
        }
    }

    fn content_width(&self) -> f32 {
        PAGE_W - 2. * MARGIN
    }

    fn ensure(&mut self, height: f32) {
        if self.y + height > PAGE_H - MARGIN && self.y > MARGIN {
            self.flush_page();
        }
    }

    fn flush_page(&mut self) {
        let settings = PageSettings::from_wh(PAGE_W, PAGE_H).unwrap_or_default();
        let mut page = self.doc.start_page_with(settings);
        let mut s = page.surface();
        for op in self.ops.drain(..) {
            draw(&mut s, self.fonts, op);
        }
        s.finish();
        page.finish();
        self.y = MARGIN;
    }

    fn finish(mut self) -> Result<Vec<u8>, PdfError> {
        if !self.ops.is_empty() {
            self.flush_page();
        }
        self.doc
            .finish()
            .map_err(|e| PdfError::Pdf(format!("{e:?}")))
    }

    /// Emit laid-out lines starting at `x`, aligned within `width`.
    fn lines(&mut self, lines: &[Line], x: f32, width: f32, align: Align, line_height: f32) {
        for line in lines {
            self.ensure(line_height);
            // Right-to-left lines start at the right and run their pieces backwards;
            // each piece is shaped right to left by the text renderer.
            let text: String = line.pieces.iter().map(|p| p.text.as_str()).collect();
            let rtl = crate::bidi::is_rtl_paragraph(&text);
            let align = if rtl && align == Align::Left {
                Align::Right
            } else {
                align
            };
            let pieces: Vec<&Piece> = if rtl {
                line.pieces.iter().rev().collect()
            } else {
                line.pieces.iter().collect()
            };
            let free = (width - line.width).max(0.);
            let mut cx = x + match align {
                Align::Center => free / 2.,
                Align::Right => free,
                _ => 0.,
            };
            let size = line.pieces.first().map_or(BODY, |p| p.size);
            let ascent = self.fonts.get(Face::Regular).ascent * size;
            let baseline = self.y + (line_height - size * 1.2) / 2. + ascent;
            for piece in pieces {
                if let Some(bg) = piece.background
                    && let Some(r) = Rect::from_xywh(cx, self.y + 1., piece.width, line_height - 2.)
                {
                    self.ops.push(Op::Rect { rect: r, color: bg });
                }
                self.ops.push(Op::Text {
                    x: cx,
                    baseline,
                    piece: piece.clone(),
                });
                if piece.underline {
                    self.ops.push(Op::Line {
                        x1: cx,
                        y1: baseline + 1.5,
                        x2: cx + piece.width,
                        y2: baseline + 1.5,
                        color: piece.color,
                        width: 0.6,
                    });
                }
                if piece.strike {
                    let y = baseline - size * 0.3;
                    self.ops.push(Op::Line {
                        x1: cx,
                        y1: y,
                        x2: cx + piece.width,
                        y2: y,
                        color: piece.color,
                        width: 0.6,
                    });
                }
                cx += piece.width;
            }
            self.y += line_height;
        }
    }
}

impl Writer<'_> {
    /// Emit a quote: its text indented past a bar on the left. The bar is drawn one
    /// segment per line, after the page break that line may cause, so every segment
    /// sits next to its text on the same page.
    fn quote(&mut self, lines: &[Line], x: f32, width: f32, align: Align, lh: f32) {
        let empty = [Line {
            pieces: Vec::new(),
            width: 0.,
        }];
        let lines = if lines.is_empty() { &empty[..] } else { lines };
        for line in lines {
            self.ensure(lh);
            if let Some(r) = Rect::from_xywh(x, self.y, 3., lh) {
                self.ops.push(Op::Rect {
                    rect: r,
                    color: RULE,
                });
            }
            self.lines(std::slice::from_ref(line), x + 14., width - 14., align, lh);
        }
    }
}

fn draw(s: &mut Surface<'_>, fonts: &Fonts, op: Op) {
    match op {
        Op::Text { x, baseline, piece } => {
            if piece.text.trim().is_empty() {
                return;
            }
            s.set_stroke(None);
            s.set_fill(Some(fill(piece.color)));
            let font = fonts.get(piece.face).font.clone();
            s.draw_text(
                Point::from_xy(x, baseline),
                font,
                piece.size,
                &piece.text,
                false,
                TextDirection::Auto,
            );
        }
        Op::Rect { rect, color } => {
            s.set_stroke(None);
            s.set_fill(Some(fill(color)));
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(p) = pb.finish() {
                s.draw_path(&p);
            }
        }
        Op::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        } => {
            s.set_fill(None);
            s.set_stroke(Some(stroke(color, width)));
            let mut pb = PathBuilder::new();
            pb.move_to(x1, y1);
            pb.line_to(x2, y2);
            if let Some(p) = pb.finish() {
                s.draw_path(&p);
            }
            s.set_stroke(None);
        }
        Op::Frame { rect, color, width } => {
            s.set_fill(None);
            s.set_stroke(Some(stroke(color, width)));
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(p) = pb.finish() {
                s.draw_path(&p);
            }
            s.set_stroke(None);
        }
        Op::Image { image, x, y, w, h } => {
            if let Some(size) = Size::from_wh(w, h) {
                s.push_transform(&Transform::from_translate(x, y));
                s.draw_image(image, size);
                s.pop();
            }
        }
    }
}

fn block_style(block: &Block) -> BlockStyle {
    let mut bs = BlockStyle {
        size: BODY * block.size.scale(),
        bold: false,
        italic: false,
        mono: false,
        color: TEXT,
        strike: false,
    };
    match block.kind {
        BlockKind::Heading(1) => (bs.size, bs.bold) = (20., true),
        BlockKind::Heading(2) => (bs.size, bs.bold) = (16., true),
        BlockKind::Heading(_) => (bs.size, bs.bold) = (13.5, true),
        BlockKind::Quote => (bs.italic, bs.color) = (true, MUTED),
        BlockKind::Code { .. } => (bs.mono, bs.size) = (true, 9.5),
        BlockKind::CheckList { checked: true } => (bs.strike, bs.color) = (true, MUTED),
        _ => {}
    }
    if block.size != TextSize::Normal && matches!(block.kind, BlockKind::Heading(_)) {
        bs.size *= 1.0;
    }
    bs
}

fn load_image(blobs: &dyn BlobResolver, id: &str, mime: &str) -> Option<PdfImage> {
    let (m, bytes) = blobs.resolve(id)?;
    // The document's MIME type wins: sealed blobs are stored as octet-stream.
    let mime = if mime.is_empty() { m } else { mime.to_owned() };
    let data: krilla::Data = bytes.into();
    match mime.as_str() {
        "image/png" => PdfImage::from_png(data, true).ok(),
        "image/jpeg" | "image/jpg" => PdfImage::from_jpeg(data, true).ok(),
        "image/gif" => PdfImage::from_gif(data, true).ok(),
        "image/webp" => PdfImage::from_webp(data, true).ok(),
        _ => None,
    }
}

/// Render a document (with its title) to PDF bytes.
pub fn to_pdf(doc: &Document, title: &str, blobs: &dyn BlobResolver) -> Result<Vec<u8>, PdfError> {
    let fonts = Fonts::load()?;
    let mut w = Writer::new(&fonts, title);
    let full = w.content_width();

    if !title.is_empty() {
        let bs = BlockStyle {
            size: 24.,
            bold: true,
            italic: false,
            mono: false,
            color: TEXT,
            strike: false,
        };
        let lines = layout_text(&fonts, &RichText::plain(title), &bs, full);
        w.lines(&lines, MARGIN, full, Align::Left, 24. * 1.35);
        w.y += 10.;
    }

    let mut numbers: Vec<usize> = Vec::new();
    for block in &doc.blocks {
        let indent = f32::from(block.indent) * INDENT;
        if !matches!(block.kind, BlockKind::NumberedList) {
            if block.kind.is_list() {
                numbers.truncate(block.indent as usize);
            } else {
                numbers.clear();
            }
        }
        let bs = block_style(block);
        let lh = bs.size * 1.45;
        match &block.kind {
            BlockKind::Paragraph | BlockKind::Heading(_) => {
                if matches!(block.kind, BlockKind::Heading(_)) {
                    w.y += 6.;
                }
                let lines = layout_text(&fonts, &block.text, &bs, full - indent);
                w.lines(&lines, MARGIN + indent, full - indent, block.align, lh);
                w.y += 4.;
            }
            BlockKind::BulletList | BlockKind::NumberedList | BlockKind::CheckList { .. } => {
                let gutter = 16.;
                let x = MARGIN + indent + gutter;
                let lines = layout_text(&fonts, &block.text, &bs, full - indent - gutter);
                w.ensure(lh);
                let top = w.y;
                let marker_x = MARGIN + indent;
                match &block.kind {
                    BlockKind::NumberedList => {
                        let level = block.indent as usize;
                        numbers.resize(level + 1, 0);
                        numbers.truncate(level + 1);
                        numbers[level] += 1;
                        let label = format!("{}.", numbers[level]);
                        let pw = fonts.width(Face::Regular, &label, bs.size);
                        let piece = Piece {
                            text: label,
                            face: Face::Regular,
                            size: bs.size,
                            color: MUTED,
                            background: None,
                            underline: false,
                            strike: false,
                            width: pw,
                        };
                        let baseline = top
                            + (lh - bs.size * 1.2) / 2.
                            + fonts.get(Face::Regular).ascent * bs.size;
                        w.ops.push(Op::Text {
                            x: marker_x,
                            baseline,
                            piece,
                        });
                    }
                    BlockKind::CheckList { checked } => {
                        let side = bs.size * 0.8;
                        if let Some(r) =
                            Rect::from_xywh(marker_x, top + (lh - side) / 2., side, side)
                        {
                            if *checked {
                                w.ops.push(Op::Rect {
                                    rect: r,
                                    color: LINK,
                                });
                            } else {
                                w.ops.push(Op::Frame {
                                    rect: r,
                                    color: MUTED,
                                    width: 0.8,
                                });
                            }
                        }
                    }
                    _ => {
                        let d = bs.size * 0.32;
                        if let Some(r) =
                            Rect::from_xywh(marker_x + 2., top + lh / 2. - d / 2., d, d)
                        {
                            w.ops.push(Op::Rect {
                                rect: r,
                                color: MUTED,
                            });
                        }
                    }
                }
                w.lines(&lines, x, full - indent - gutter, block.align, lh);
                w.y += 2.;
            }
            BlockKind::Quote => {
                let lines = layout_text(&fonts, &block.text, &bs, full - indent - 14.);
                w.quote(&lines, MARGIN + indent, full - indent, block.align, lh);
                w.y += 6.;
            }
            BlockKind::Code { .. } => {
                let pad = 8.;
                let lines = layout_text(&fonts, &block.text, &bs, full - indent - 2. * pad);
                // Draw the background per page chunk: simple approach, one rect per line.
                w.y += 2.;
                for line in &lines {
                    w.ensure(lh);
                    if let Some(r) = Rect::from_xywh(MARGIN + indent, w.y, full - indent, lh) {
                        w.ops.push(Op::Rect {
                            rect: r,
                            color: CODE_BG,
                        });
                    }
                    w.lines(
                        std::slice::from_ref(line),
                        MARGIN + indent + pad,
                        full - indent - 2. * pad,
                        Align::Left,
                        lh,
                    );
                }
                w.y += 8.;
            }
            BlockKind::Divider => {
                w.ensure(14.);
                w.y += 7.;
                w.ops.push(Op::Line {
                    x1: MARGIN,
                    y1: w.y,
                    x2: PAGE_W - MARGIN,
                    y2: w.y,
                    color: RULE,
                    width: 0.8,
                });
                w.y += 7.;
            }
            BlockKind::Image(img) => {
                let Some(image) = load_image(blobs, &img.blob, &img.mime) else {
                    continue;
                };
                let (pw, ph) = image.size();
                if pw == 0 || ph == 0 {
                    continue;
                }
                // Editor widths are logical pixels (96 dpi) → points.
                let mut iw = img.width.map_or(full, |px| px * 0.75).min(full - indent);
                let mut ih = match (img.width, img.height) {
                    // Stretched in the editor: keep the same proportions it shows.
                    (Some(w), Some(h)) if w > 0. => iw * h / w,
                    _ => iw * ph as f32 / pw as f32,
                };
                let max_h = PAGE_H - 2. * MARGIN;
                if ih > max_h {
                    iw *= max_h / ih;
                    ih = max_h;
                }
                w.ensure(ih + 6.);
                let free = full - indent - iw;
                let x = MARGIN
                    + indent
                    + match block.align {
                        Align::Center => free / 2.,
                        Align::Right => free,
                        _ => 0.,
                    };
                w.ops.push(Op::Image {
                    image,
                    x,
                    y: w.y,
                    w: iw,
                    h: ih,
                });
                w.y += ih + 8.;
            }
            BlockKind::Attachment(att) => {
                let bs = BlockStyle {
                    size: 10.,
                    bold: false,
                    italic: true,
                    mono: false,
                    color: MUTED,
                    strike: false,
                };
                let lines = layout_text(
                    &fonts,
                    &RichText::plain(&format!("[{}]", att.file_name)),
                    &bs,
                    full,
                );
                w.lines(&lines, MARGIN + indent, full - indent, Align::Left, 15.);
                w.y += 4.;
            }
            BlockKind::Table(table) => {
                let cols = table.cols().max(1);
                let tw = full - indent;
                let cw = tw / cols as f32;
                let pad = 5.;
                let cell_lh = BODY * 1.4;
                w.y += 4.;
                for (r, row) in table.rows.iter().enumerate() {
                    let header = r == 0 && table.header;
                    let cbs = BlockStyle {
                        size: BODY,
                        bold: header,
                        italic: false,
                        mono: false,
                        color: TEXT,
                        strike: false,
                    };
                    let laid: Vec<Vec<Line>> = row
                        .iter()
                        .map(|c| layout_text(&fonts, c, &cbs, cw - 2. * pad))
                        .collect();
                    let rows_n = laid.iter().map(Vec::len).max().unwrap_or(1).max(1);
                    let rh = rows_n as f32 * cell_lh + 2. * pad;
                    w.ensure(rh);
                    let top = w.y;
                    if header && let Some(rect) = Rect::from_xywh(MARGIN + indent, top, tw, rh) {
                        w.ops.push(Op::Rect {
                            rect,
                            color: CODE_BG,
                        });
                    }
                    for (c, lines) in laid.iter().enumerate() {
                        let x = MARGIN + indent + c as f32 * cw;
                        w.y = top + pad;
                        w.lines(lines, x + pad, cw - 2. * pad, Align::Left, cell_lh);
                        if let Some(rect) = Rect::from_xywh(x, top, cw, rh) {
                            w.ops.push(Op::Frame {
                                rect,
                                color: RULE,
                                width: 0.6,
                            });
                        }
                    }
                    w.y = top + rh;
                }
                w.y += 8.;
            }
        }
    }
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::NoBlobs;
    use crate::model::Table;

    #[test]
    fn renders_cjk_and_rtl_text() {
        if Fonts::load().is_err() {
            return;
        }
        let doc = Document {
            version: crate::DOC_VERSION,
            blocks: vec![
                Block::paragraph("中文 日本語 한국어"),
                Block::paragraph("שלום עולם"),
                Block::paragraph("مرحبا بالعالم"),
            ],
        };
        let pdf = to_pdf(&doc, "多语言", &crate::NoBlobs).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn quote_bar_follows_its_lines_across_a_page_break() {
        let Ok(fonts) = Fonts::load() else {
            return;
        };
        let mut w = Writer::new(&fonts, "T");
        let lh = BODY * 1.45;
        // Room for one line on this page; the next two go to the next page.
        w.y = PAGE_H - MARGIN - lh * 1.5;
        let line = |t: &str| Line {
            pieces: vec![Piece {
                text: t.into(),
                face: Face::Regular,
                size: BODY,
                color: TEXT,
                background: None,
                underline: false,
                strike: false,
                width: 10.,
            }],
            width: 10.,
        };
        w.quote(
            &[line("a"), line("b"), line("c")],
            MARGIN,
            300.,
            Align::Left,
            lh,
        );
        // Only the new page's ops remain: two bar segments starting at the top margin.
        let bars: Vec<f32> = w
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Rect { rect, .. } => Some(rect.top()),
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2);
        assert!((bars[0] - MARGIN).abs() < 0.01, "{bars:?}");
        assert!((bars[1] - (MARGIN + lh)).abs() < 0.01, "{bars:?}");
    }

    #[test]
    fn renders_a_pdf_when_fonts_exist() {
        if Fonts::load().is_err() {
            eprintln!("no system fonts; skipping");
            return;
        }
        let mut doc = crate::from_markdown(
            "# Title\n\nSome **bold** text that is long enough to wrap across more than one line of the page so the line breaker runs.\n\n- one\n- two\n\n1. a\n\n- [x] done\n\n> quote\n\n```\ncode\n```\n\n---\n",
        );
        let mut table = Table::new(2, 3);
        table.rows[1][2] = RichText::plain("cell");
        doc.blocks.push(Block::new(BlockKind::Table(table), ""));
        for _ in 0..80 {
            doc.blocks
                .push(Block::paragraph("filler paragraph to force several pages"));
        }
        let bytes = to_pdf(&doc, "Card", &NoBlobs).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 1000);
    }
}
