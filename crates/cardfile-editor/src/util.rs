//! Small helpers shared by the editor views.

use std::ops::Range;
use std::path::Path;

use unicode_segmentation::UnicodeSegmentation;

/// Byte ranges of words that `check` rejects. Skips numbers, URLs and very short words.
pub fn misspelled_ranges(text: &str, check: impl Fn(&str) -> bool) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for (i, word) in text.split_word_bound_indices() {
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        if letters < 2 || word.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        // Skip parts of URLs and e-mail addresses.
        let ctx_start = text[..i].rfind(char::is_whitespace).map_or(0, |p| p + 1);
        let ctx_end = text[i..]
            .find(char::is_whitespace)
            .map_or(text.len(), |p| i + p);
        let token = &text[ctx_start..ctx_end];
        if token.contains("://") || token.contains('@') || token.starts_with("www.") {
            continue;
        }
        if !check(word) {
            out.push(i..i + word.len());
        }
    }
    out
}

/// MIME type guessed from a file extension.
pub fn mime_for_path(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "zip" => "application/zip",
        "json" => "application/json",
        "csv" => "text/csv",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
    .to_owned()
}

/// Human-readable byte size.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024. && u < UNITS.len() - 1 {
        v /= 1024.;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_misspellings() {
        let r = misspelled_ranges("helo world see https://exampel.com a1b2", |w| {
            w != "helo" && w != "exampel"
        });
        assert_eq!(r, vec![0..4]);
    }

    #[test]
    fn mime_and_size() {
        assert_eq!(mime_for_path(Path::new("a.PNG")), "image/png");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1536), "1.5 KB");
    }
}
