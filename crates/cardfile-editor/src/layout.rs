//! Geometry of a shaped text slot: rows, caret positions and hit testing.
//!
//! GPUI shapes one [`WrappedLine`] per `\n`-separated paragraph. We compute the visual
//! rows ourselves from the wrap boundaries so caret placement respects text alignment.

use std::sync::Arc;

use gpui_kit::{Bounds, LineLayout, Pixels, Point, TextAlign, WrappedLine, point, px};

#[derive(Clone, Debug)]
pub struct Row {
    /// Byte range in the slot text.
    pub start: usize,
    pub end: usize,
    /// x of `start` in the unwrapped line layout.
    x0: Pixels,
    /// Horizontal offset applied by alignment.
    pub dx: Pixels,
    /// Top of the row relative to the slot origin.
    pub y: Pixels,
}

#[derive(Clone, Debug)]
pub struct Para {
    /// The paragraph's layout before wrapping.
    pub unwrapped: Arc<LineLayout>,
    /// Byte offset of this paragraph in the slot text.
    pub start: usize,
    pub rows: Vec<Row>,
}

/// Layout of one slot, recorded during paint for hit testing and caret movement.
#[derive(Clone, Debug)]
pub struct SlotLayout {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
    pub paras: Vec<Para>,
}

impl SlotLayout {
    pub fn new(
        lines: &[WrappedLine],
        bounds: Bounds<Pixels>,
        line_height: Pixels,
        align: TextAlign,
    ) -> Self {
        let width = bounds.size.width;
        let mut paras = Vec::with_capacity(lines.len());
        let mut start = 0;
        let mut y = px(0.);
        for line in lines {
            let layout = &line.unwrapped_layout;
            let mut cuts: Vec<usize> = line
                .wrap_boundaries()
                .iter()
                .filter_map(|b| {
                    layout
                        .runs
                        .get(b.run_ix)?
                        .glyphs
                        .get(b.glyph_ix)
                        .map(|g| g.index)
                })
                .collect();
            cuts.insert(0, 0);
            cuts.push(line.len());
            let mut rows = Vec::with_capacity(cuts.len() - 1);
            for w in cuts.windows(2) {
                let (s, e) = (w[0], w[1].max(w[0]));
                let x0 = layout.x_for_index(s);
                let row_width = layout.x_for_index(e) - x0;
                let free = (width - row_width).max(px(0.));
                let dx = match align {
                    TextAlign::Center => free / 2.,
                    TextAlign::Right => free,
                    _ => px(0.),
                };
                rows.push(Row {
                    start: start + s,
                    end: start + e,
                    x0,
                    dx,
                    y,
                });
                y += line_height;
            }
            paras.push(Para {
                unwrapped: line.unwrapped_layout.clone(),
                start,
                rows,
            });
            start += line.len() + 1; // skip '\n'
        }
        if paras.is_empty() {
            // Nothing shaped (empty slot): one empty row keeps the caret placeable.
            y += line_height;
        }
        Self {
            bounds,
            line_height,
            paras,
        }
    }

    pub fn height(&self) -> Pixels {
        let rows: usize = self.paras.iter().map(|p| p.rows.len()).sum();
        self.line_height * rows.max(1)
    }

    fn rows(&self) -> impl Iterator<Item = (&Para, &Row)> {
        self.paras
            .iter()
            .flat_map(|p| p.rows.iter().map(move |r| (p, r)))
    }

    /// Row containing `offset` (the caret at a wrap boundary belongs to the next row).
    fn row_for(&self, offset: usize) -> Option<(&Para, &Row)> {
        let mut found = None;
        for (p, r) in self.rows() {
            if offset >= r.start && offset <= r.end {
                found = Some((p, r));
                if offset < r.end {
                    break;
                }
            }
        }
        found.or_else(|| self.rows().last())
    }

    /// Caret position (top-left of the caret) relative to the window.
    pub fn position_for(&self, offset: usize) -> Point<Pixels> {
        let origin = self.bounds.origin;
        match self.row_for(offset) {
            Some((p, r)) => {
                let local = offset.clamp(r.start, r.end) - p.start;
                let x = p.unwrapped.x_for_index(local) - r.x0 + r.dx;
                point(origin.x + x, origin.y + r.y)
            }
            None => origin,
        }
    }

    /// Closest text offset to a window-space point.
    pub fn offset_for(&self, pos: Point<Pixels>) -> usize {
        let local_y = pos.y - self.bounds.origin.y;
        let local_x = pos.x - self.bounds.origin.x;
        let mut best: Option<(&Para, &Row)> = None;
        for (p, r) in self.rows() {
            best = Some((p, r));
            if local_y < r.y + self.line_height {
                break;
            }
        }
        let Some((p, r)) = best else { return 0 };
        let x = local_x - r.dx + r.x0;
        let ix = p.unwrapped.closest_index_for_x(x) + p.start;
        ix.clamp(r.start, r.end)
    }

    /// Start and end offsets of the visual row holding `offset` (Home/End).
    pub fn row_bounds(&self, offset: usize) -> (usize, usize) {
        self.row_for(offset)
            .map_or((0, 0), |(_, r)| (r.start, r.end))
    }

    /// Selection rectangles for the byte range `s..e` (window space).
    pub fn selection_rects(&self, s: usize, e: usize) -> Vec<Bounds<Pixels>> {
        let mut rects = Vec::new();
        let origin = self.bounds.origin;
        for (p, r) in self.rows() {
            let rs = s.max(r.start);
            let re = e.min(r.end);
            let covers_newline =
                e > r.end && s <= r.end && p.rows.last().is_some_and(|last| last.end == r.end);
            if rs > re || (rs == re && !covers_newline) {
                continue;
            }
            let layout = &p.unwrapped;
            let x1 = layout.x_for_index(rs - p.start) - r.x0 + r.dx;
            let mut x2 = layout.x_for_index(re - p.start) - r.x0 + r.dx;
            if covers_newline {
                x2 += px(6.); // show that the line break is selected
            }
            rects.push(Bounds::from_corners(
                point(origin.x + x1, origin.y + r.y),
                point(origin.x + x2, origin.y + r.y + self.line_height),
            ));
        }
        if rects.is_empty() && s == 0 && e > 0 && self.paras.is_empty() {
            rects.push(Bounds::new(
                origin,
                gpui_kit::size(px(6.), self.line_height),
            ));
        }
        rects
    }
}
