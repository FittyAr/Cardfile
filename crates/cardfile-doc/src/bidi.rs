//! Text direction helpers shared by the exporters.

/// Whether `c` belongs to a right-to-left script (Hebrew, Arabic, Syriac, Thaana,
/// N'Ko, …).
pub fn is_rtl_char(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Whether the first character with a strong direction in `text` is right-to-left,
/// i.e. the paragraph reads from the right.
pub fn is_rtl_paragraph(text: &str) -> bool {
    for c in text.chars() {
        if is_rtl_char(c) {
            return true;
        }
        if c.is_alphabetic() {
            return false;
        }
    }
    false
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
}
