//! Right-to-left text (Hebrew, Arabic, Persian, …).
//!
//! GPUI lays out every shaped run left to right, so right-to-left text (which the
//! platform shapes correctly, Arabic joining forms included) comes out in logical
//! order. [`fix_lines`] moves the glyphs to their visual positions after shaping,
//! and the hit-testing helpers here understand the result.
//!
//! For plain labels outside the editor, [`display`] returns a string already in
//! visual order (with Arabic letters reshaped to their presentation forms, so the
//! reversed text still joins correctly).

use std::sync::Arc;

use gpui_kit::{LineLayout, Pixels, WrappedLine, WrappedLineLayout, px};
use unicode_bidi::{BidiInfo, Direction, Level};

/// Whether `text` contains any right-to-left character.
pub fn has_rtl(text: &str) -> bool {
    text.chars().any(cardfile_doc::bidi::is_rtl_char)
}

/// Whether a paragraph's first strong character is right-to-left (it should be
/// aligned to the right).
pub fn is_rtl_paragraph(text: &str) -> bool {
    // Same rule as the exporters, so a card reads the same on screen and on paper.
    cardfile_doc::bidi::is_rtl_paragraph(text)
}

/// The paragraph's base level: right-to-left when its first strong character is.
fn base_level(text: &str) -> Level {
    if is_rtl_paragraph(text) {
        Level::rtl()
    } else {
        Level::ltr()
    }
}

/// Embedding level of every byte of `text`.
fn levels(text: &str) -> Vec<Level> {
    BidiInfo::new(text, Some(base_level(text))).levels
}

/// Lay the shaped glyphs out again in visual order. The platform shapes each run
/// correctly (Arabic joining included) but GPUI places runs and glyphs in logical
/// order; here every glyph keeps its advance and moves to its visual position
/// following the Unicode bidirectional algorithm. Returns `None` for text without
/// right-to-left characters.
fn reorder(text: &str, layout: &LineLayout) -> Option<LineLayout> {
    if !has_rtl(text) {
        return None;
    }
    // Advance of each glyph: the distance to the next glyph in paint order.
    let mut glyphs: Vec<(usize, usize, usize, Pixels)> = Vec::new(); // run, glyph, index, advance
    let flat: Vec<(usize, usize, Pixels, usize)> = layout
        .runs
        .iter()
        .enumerate()
        .flat_map(|(r, run)| {
            run.glyphs
                .iter()
                .enumerate()
                .map(move |(g, glyph)| (r, g, glyph.position.x, glyph.index))
        })
        .collect();
    for (k, (r, g, x, index)) in flat.iter().enumerate() {
        let next = flat.get(k + 1).map_or(layout.width, |n| n.2);
        glyphs.push((*r, *g, *index, (next - *x).max(px(0.))));
    }
    let info = BidiInfo::new(text, Some(base_level(text)));
    let mut fixed = LineLayout {
        font_size: layout.font_size,
        width: layout.width,
        ascent: layout.ascent,
        descent: layout.descent,
        runs: layout.runs.clone(),
        len: layout.len,
    };
    let mut x = px(0.);
    for para in &info.paragraphs {
        let (levels, runs) = info.visual_runs(para, para.range.clone());
        for run in runs {
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            let mut members: Vec<&(usize, usize, usize, Pixels)> =
                glyphs.iter().filter(|g| run.contains(&g.2)).collect();
            members.sort_by_key(|g| g.2);
            if rtl {
                members.reverse();
            }
            for (r, g, _, adv) in members {
                if let Some(glyph) = fixed
                    .runs
                    .get_mut(*r)
                    .and_then(|run| run.glyphs.get_mut(*g))
                {
                    glyph.position.x = x;
                }
                x += *adv;
            }
        }
    }
    Some(fixed)
}

/// Fix the right-to-left runs of shaped lines in place.
pub fn fix_lines(lines: &mut [WrappedLine]) {
    for line in lines {
        let text = line.text.clone();
        let Some(fixed) = reorder(&text, &line.unwrapped_layout) else {
            continue;
        };
        let replaced = WrappedLineLayout {
            unwrapped_layout: Arc::new(fixed),
            wrap_boundaries: line.wrap_boundaries.clone(),
            wrap_width: line.wrap_width,
        };
        let layout: &mut Arc<WrappedLineLayout> = &mut *line;
        *layout = Arc::new(replaced);
    }
}

/// One glyph cluster's extent, for hit testing.
struct Cell {
    index: usize,
    left: Pixels,
    right: Pixels,
    rtl: bool,
}

fn cells(text: &str, layout: &LineLayout) -> Vec<Cell> {
    let levels = if has_rtl(text) {
        levels(text)
    } else {
        Vec::new()
    };
    let mut glyphs: Vec<(Pixels, usize)> = layout
        .runs
        .iter()
        .flat_map(|r| r.glyphs.iter().map(|g| (g.position.x, g.index)))
        .collect();
    glyphs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::with_capacity(glyphs.len());
    for (k, (x, index)) in glyphs.iter().enumerate() {
        let right = glyphs.get(k + 1).map_or(layout.width, |g| g.0);
        out.push(Cell {
            index: *index,
            left: *x,
            right: right.max(*x),
            rtl: levels.get(*index).is_some_and(|l| l.is_rtl()),
        });
    }
    out
}

fn next_char(text: &str, index: usize) -> usize {
    text[index.min(text.len())..]
        .chars()
        .next()
        .map_or(text.len(), |c| index + c.len_utf8())
}

/// x of the caret before byte `index`, understanding right-to-left runs.
pub fn x_for_index(text: &str, layout: &LineLayout, index: usize) -> Pixels {
    if !has_rtl(text) {
        return layout.x_for_index(index);
    }
    let cells = cells(text, layout);
    if let Some(c) = cells.iter().find(|c| c.index == index) {
        return if c.rtl { c.right } else { c.left };
    }
    // End of the line (or inside a cluster): after the logically last glyph.
    match cells
        .iter()
        .filter(|c| c.index < index)
        .max_by_key(|c| c.index)
    {
        Some(c) if c.rtl => c.left,
        Some(c) => c.right,
        None => px(0.),
    }
}

/// The caret offset closest to `x`.
pub fn index_for_x(text: &str, layout: &LineLayout, x: Pixels) -> usize {
    if !has_rtl(text) {
        return layout.closest_index_for_x(x);
    }
    let cells = cells(text, layout);
    let Some(cell) = cells.iter().find(|c| x < c.right).or_else(|| cells.last()) else {
        return 0;
    };
    let before = x < (cell.left + cell.right) / 2.;
    // In right-to-left text the logical start of a glyph is its right edge.
    if before != cell.rtl {
        cell.index
    } else {
        next_char(text, cell.index)
    }
}

/// A label in visual order for widgets that lay text out left to right only.
pub fn display(text: &str) -> String {
    cardfile_i18n::visual(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(glyphs: &[&[(usize, f32)]], width: f32) -> LineLayout {
        use gpui_kit::{FontId, GlyphId, ShapedGlyph, ShapedRun, point};
        LineLayout {
            font_size: px(14.),
            width: px(width),
            ascent: px(10.),
            descent: px(4.),
            runs: glyphs
                .iter()
                .map(|run| ShapedRun {
                    font_id: FontId(0),
                    glyphs: run
                        .iter()
                        .map(|(index, x)| ShapedGlyph {
                            id: GlyphId(0),
                            position: point(px(*x), px(0.)),
                            index: *index,
                            is_emoji: false,
                        })
                        .collect(),
                })
                .collect(),
            len: 7,
        }
    }

    fn positions(l: &LineLayout) -> Vec<(usize, f32)> {
        let mut v: Vec<(usize, f32)> = l
            .runs
            .iter()
            .flat_map(|r| r.glyphs.iter().map(|g| (g.index, f32::from(g.position.x))))
            .collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        v
    }

    #[test]
    fn reorders_rtl_paragraph_with_digits() {
        // "אב 12" shaped in logical order: alef, bet, space, then the digits.
        let text = "\u{5d0}\u{5d1} 12";
        let l = layout(
            &[&[(0, 0.), (2, 10.), (4, 20.)], &[(5, 25.), (6, 33.)]],
            41.,
        );
        let fixed = reorder(text, &l).unwrap();
        // Visual order, left to right: "12", space, bet, alef.
        assert_eq!(
            positions(&fixed),
            [(5, 0.), (6, 8.), (4, 16.), (2, 21.), (0, 31.)]
        );
        // Hit testing: the caret before alef (offset 0) sits at its right edge.
        assert_eq!(f32::from(x_for_index(text, &fixed, 0)), 41.);
        assert_eq!(index_for_x(text, &fixed, px(40.)), 0);
        assert!(reorder("plain", &l).is_none());
    }

    #[test]
    fn ltr_paragraph_keeps_latin_order() {
        // Latin first, so the paragraph stays left to right; only "אב" turns around.
        let text = "ab \u{5d0}\u{5d1}";
        let l = layout(&[&[(0, 0.), (1, 8.), (2, 16.)], &[(3, 21.), (5, 31.)]], 41.);
        let fixed = reorder(text, &l).unwrap();
        assert_eq!(
            positions(&fixed),
            [(0, 0.), (1, 8.), (2, 16.), (5, 21.), (3, 31.)]
        );
    }

    #[test]
    fn detects_direction() {
        assert!(!has_rtl("hello 你好"));
        assert!(has_rtl("שלום"));
        assert!(is_rtl_paragraph("שלום world"));
        assert!(!is_rtl_paragraph("hello שלום"));
        assert!(is_rtl_paragraph("123 مرحبا"));
    }
}
