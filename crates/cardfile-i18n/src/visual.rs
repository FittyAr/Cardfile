//! Visual ordering of right-to-left text for widgets without bidi support.

use unicode_bidi::BidiInfo;

/// Whether `text` contains any right-to-left character (Hebrew, Arabic, …).
pub fn has_rtl(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(c as u32,
            0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
    })
}

/// A label in visual order for widgets that lay text out left to right only.
pub fn visual(text: &str) -> String {
    if !has_rtl(text) {
        return text.to_owned();
    }
    let shaped = ar_reshaper::reshape_line(text);
    let info = BidiInfo::new(&shaped, None);
    let mut out = String::with_capacity(shaped.len());
    for para in &info.paragraphs {
        let line = para.range.clone();
        let (levels, runs) = info.visual_runs(para, line);
        for run in runs {
            let piece = &shaped[run.clone()];
            if levels[run.start].is_rtl() {
                out.extend(piece.chars().rev().map(mirror_bracket));
            } else {
                out.push_str(piece);
            }
        }
    }
    out
}

fn mirror_bracket(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(s: &str) -> String {
        visual(s)
    }

    #[test]
    fn display_reorders_hebrew() {
        assert_eq!(display("abc"), "abc");
        // Hebrew has no joining forms: plain reversal of the RTL run.
        assert_eq!(display("שלום"), "םולש");
        assert_eq!(display("a שלום b"), "a םולש b");
        // Digits keep their order inside right-to-left text.
        assert_eq!(display("שלום 12"), "12 םולש");
    }

    #[test]
    fn display_reshapes_arabic() {
        let out = display("لا");
        // Lam-alef becomes a single presentation-form ligature.
        assert_eq!(out.chars().count(), 1);
        assert!(out.chars().all(|c| ('\u{FB50}'..='\u{FEFF}').contains(&c)));
    }
}
