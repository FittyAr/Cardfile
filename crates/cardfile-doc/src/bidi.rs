//! Text direction helpers shared by the exporters and the editor, so both decide a
//! paragraph's direction the same way.

use unicode_bidi::Direction;

/// Whether `c` may belong to a right-to-left script (Hebrew, Arabic, Syriac, Thaana,
/// N'Ko, …). A cheap pre-filter only: it also matches weak characters of those
/// blocks (Arabic-Indic digits, marks), so the paragraph direction is decided by
/// [`is_rtl_paragraph`]. The byte order mark (U+FEFF) is excluded.
pub fn is_rtl_char(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFE | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Whether the first character with a strong direction in `text` is right-to-left,
/// i.e. the paragraph reads from the right (Unicode bidi rules P2/P3).
pub fn is_rtl_paragraph(text: &str) -> bool {
    text.chars().any(is_rtl_char) && unicode_bidi::get_base_direction(text) == Direction::Rtl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_direction() {
        assert!(is_rtl_paragraph("123 שלום"));
        assert!(is_rtl_paragraph("مرحبا world"));
        assert!(!is_rtl_paragraph("hello שלום"));
        assert!(!is_rtl_paragraph("你好"));
        assert!(!is_rtl_paragraph(""));
    }

    #[test]
    fn weak_characters_do_not_decide_direction() {
        // A byte order mark and Arabic-Indic digits are not strong characters.
        assert!(!is_rtl_paragraph("\u{FEFF}hello"));
        assert!(!is_rtl_paragraph("\u{0661}\u{0662} hello"));
        assert!(!is_rtl_paragraph("\u{0661}\u{0662}"));
        assert!(is_rtl_paragraph("\u{FEFF}שלום"));
    }
}
