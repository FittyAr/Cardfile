//! The GPUI element that shapes and paints one text slot (a block or a table cell).

use std::ops::Range;

use cardfile_doc::{RichText, Style as RunStyle};
use gpui_kit::{
    App, AvailableSpace, Bounds, Element, ElementId, Entity, FontStyle, FontWeight,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId, Pixels, SharedString,
    StrikethroughStyle, Style, TextAlign, TextRun, UnderlineStyle, Window, WrappedLine, fill, font,
    px, size,
};

use crate::editor::RichEditor;
use crate::layout::SlotLayout;
use crate::pos::Slot;

/// Visual parameters for a slot, resolved from the block kind and theme.
#[derive(Clone, Debug)]
pub struct SlotStyle {
    pub font_family: SharedString,
    pub mono_family: SharedString,
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub color: Hsla,
    pub muted: Hsla,
    pub link: Hsla,
    pub code_bg: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
    pub misspelled: Hsla,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub strike: bool,
    pub align: TextAlign,
    pub placeholder: Option<SharedString>,
}

pub struct SlotElement {
    editor: Entity<RichEditor>,
    slot: Slot,
    text: RichText,
    style: SlotStyle,
}

impl SlotElement {
    pub fn new(editor: Entity<RichEditor>, slot: Slot, text: RichText, style: SlotStyle) -> Self {
        Self {
            editor,
            slot,
            text,
            style,
        }
    }
}

/// Build shaping runs: document styles, plus IME composition and spelling overlays.
pub(crate) fn build_runs(
    text: &RichText,
    st: &SlotStyle,
    marked: Option<Range<usize>>,
    misspelled: &[Range<usize>],
) -> Vec<TextRun> {
    // Collect every boundary where any attribute may change.
    let mut cuts: Vec<usize> = vec![0, text.len()];
    for (r, _) in text.spans() {
        cuts.push(r.start);
        cuts.push(r.end);
    }
    for r in marked.iter().chain(misspelled) {
        cuts.push(r.start.min(text.len()));
        cuts.push(r.end.min(text.len()));
    }
    cuts.sort_unstable();
    cuts.dedup();
    let spans: Vec<(Range<usize>, RunStyle)> = text.spans().map(|(r, s)| (r, s.clone())).collect();
    let mut runs = Vec::with_capacity(cuts.len());
    for w in cuts.windows(2) {
        let (s, e) = (w[0], w[1]);
        if e <= s {
            continue;
        }
        let style = spans
            .iter()
            .find(|(r, _)| r.start <= s && s < r.end)
            .map(|(_, st)| st.clone())
            .unwrap_or_default();
        let mono = st.mono || style.code;
        let mut f = font(if mono {
            st.mono_family.clone()
        } else {
            st.font_family.clone()
        });
        if st.bold || style.bold {
            f.weight = FontWeight::BOLD;
        }
        if st.italic || style.italic {
            f.style = FontStyle::Italic;
        }
        let link = style.link.is_some();
        let mut color = if link { st.link } else { st.color };
        if let Some(c) = style.color.as_deref().and_then(parse_hex) {
            color = c;
        }
        let background_color = style
            .highlight
            .as_deref()
            .and_then(parse_hex)
            .map(|c| c.opacity(0.45))
            .or_else(|| (style.code && !st.mono).then_some(st.code_bg));
        let mut underline = (style.underline || link).then(|| UnderlineStyle {
            thickness: px(1.),
            color: Some(color),
            wavy: false,
        });
        if misspelled.iter().any(|r| r.start <= s && s < r.end) {
            underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(st.misspelled),
                wavy: true,
            });
        }
        if marked.as_ref().is_some_and(|r| r.start <= s && s < r.end) {
            underline = Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(color),
                wavy: false,
            });
        }
        let strikethrough = (style.strike || st.strike).then(|| StrikethroughStyle {
            thickness: px(1.),
            color: Some(color),
        });
        runs.push(TextRun {
            len: e - s,
            font: f,
            color,
            background_color,
            underline,
            strikethrough,
        });
    }
    runs
}

/// Parse `#rrggbb` / `#rgb` into a color.
pub fn parse_hex(s: &str) -> Option<Hsla> {
    let hex = s.strip_prefix('#')?;
    let v = match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok()?,
        3 => {
            let n = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
            (r * 17) << 16 | (g * 17) << 8 | (b * 17)
        }
        _ => return None,
    };
    Some(gpui_kit::rgb(v).into())
}

fn shape(
    window: &mut Window,
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    wrap: Option<Pixels>,
) -> Vec<WrappedLine> {
    window
        .text_system()
        .shape_text(
            SharedString::from(text.to_owned()),
            font_size,
            runs,
            wrap,
            None,
        )
        .map(|lines| lines.into_iter().collect())
        .unwrap_or_default()
}

pub struct Prepaint {
    lines: Vec<WrappedLine>,
    placeholder: Vec<WrappedLine>,
}

impl IntoElement for SlotElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SlotElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text = self.text.clone();
        let st = self.style.clone();
        let mut style = Style::default();
        style.size.width = gpui_kit::relative(1.).into();
        let layout = window.request_measured_layout(style, move |known, available, window, _cx| {
            let wrap = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            });
            let runs = build_runs(&text, &st, None, &[]);
            let lines = shape(window, &text.text, &runs, st.font_size, wrap);
            let rows: usize = lines.iter().map(|l| l.wrap_boundaries().len() + 1).sum();
            let width = wrap.unwrap_or_else(|| {
                lines
                    .iter()
                    .map(|l| l.width())
                    .fold(px(0.), |a, b| a.max(b))
            });
            size(width, st.line_height * rows.max(1))
        });
        let _ = cx;
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let marked = editor.marked_range_in(self.slot);
        let misspelled = editor.misspelled_in(self.slot, &self.text.text);
        let runs = build_runs(&self.text, &self.style, marked, &misspelled);
        let lines = shape(
            window,
            &self.text.text,
            &runs,
            self.style.font_size,
            Some(bounds.size.width),
        );
        let placeholder = match (&self.style.placeholder, self.text.is_empty()) {
            (Some(p), true) => {
                let run = TextRun {
                    len: p.len(),
                    font: font(self.style.font_family.clone()),
                    color: self.style.muted,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                shape(
                    window,
                    p,
                    &[run],
                    self.style.font_size,
                    Some(bounds.size.width),
                )
            }
            _ => Vec::new(),
        };
        Prepaint { lines, placeholder }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let lh = self.style.line_height;
        let layout = SlotLayout::new(&prepaint.lines, bounds, lh, self.style.align);
        let (focused, selected, caret) = {
            let editor = self.editor.read(cx);
            let focused = editor.focus_handle.is_focused(window);
            let selected = editor.selected_range_in(self.slot, self.text.len());
            let head = editor.model.sel.head;
            let caret = (focused
                && head.slot == self.slot
                && editor.caret_visible()
                && !editor.is_read_only())
            .then_some(head.offset);
            (focused, selected, caret)
        };

        // Selection.
        if let Some(range) = selected {
            let color = if focused {
                self.style.selection
            } else {
                self.style.selection.opacity(0.5)
            };
            for rect in layout.selection_rects(range.start, range.end) {
                window.paint_quad(fill(rect, color));
            }
        }

        // Text.
        for (line, p) in prepaint.lines.iter().zip(&layout.paras) {
            let y = p.rows.first().map_or(px(0.), |r| r.y);
            let origin = gpui_kit::point(bounds.origin.x, bounds.origin.y + y);
            let para_bounds = Bounds::new(origin, size(bounds.size.width, lh * p.rows.len()));
            let _ =
                line.paint_background(origin, lh, self.style.align, Some(para_bounds), window, cx);
            let _ = line.paint(origin, lh, self.style.align, Some(para_bounds), window, cx);
        }
        for line in &prepaint.placeholder {
            let _ = line.paint(
                bounds.origin,
                lh,
                self.style.align,
                Some(bounds),
                window,
                cx,
            );
        }

        // Caret.
        if let Some(offset) = caret {
            let at = layout.position_for(offset.min(self.text.len()));
            let h = lh * 0.85;
            let top = at.y + (lh - h) / 2.;
            window.paint_quad(fill(
                Bounds::new(gpui_kit::point(at.x, top), size(px(1.5), h)),
                self.style.caret,
            ));
        }

        let slot = self.slot;
        let refresh = self
            .editor
            .update(cx, |editor, _| editor.store_layout(slot, layout));
        if refresh {
            window.refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_hex;

    #[test]
    fn hex_colors() {
        assert!(parse_hex("#ff0000").is_some());
        assert!(parse_hex("#f00").is_some());
        assert!(parse_hex("red").is_none());
        assert!(parse_hex("#12345").is_none());
    }
}
