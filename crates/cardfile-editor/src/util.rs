//! Small helpers shared by the editor views.

use std::ops::Range;
use std::path::Path;

use unicode_segmentation::UnicodeSegmentation;

/// Byte ranges of words that `check` rejects. Skips numbers, URLs and very short words.
///
/// Runs in linear time: the whitespace-separated tokens (used to recognise URLs and
/// e-mail addresses) are found in one pass and walked alongside the words.
pub fn misspelled_ranges(text: &str, mut check: impl FnMut(&str) -> bool) -> Vec<Range<usize>> {
    let tokens = whitespace_tokens(text);
    let mut tok = 0;
    let mut out = Vec::new();
    for (i, word) in text.split_word_bound_indices() {
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        if letters < 2 || word.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        // Skip parts of URLs and e-mail addresses.
        while tokens.get(tok).is_some_and(|t| t.end <= i) {
            tok += 1;
        }
        if let Some(t) = tokens.get(tok).filter(|t| t.start <= i) {
            let token = &text[t.clone()];
            if token.contains("://") || token.contains('@') || token.starts_with("www.") {
                continue;
            }
        }
        if !check(word) {
            out.push(i..i + word.len());
        }
    }
    out
}

/// Byte ranges of the runs of non-whitespace characters, in order.
fn whitespace_tokens(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push(s..i);
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(s..text.len());
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

/// Human-readable byte size, with localized units.
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = [
        "editor.size_units.bytes",
        "editor.size_units.kb",
        "editor.size_units.mb",
        "editor.size_units.gb",
    ];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024. && u < UNITS.len() - 1 {
        v /= 1024.;
        u += 1;
    }
    let n = if u == 0 {
        bytes.to_string()
    } else {
        format!("{v:.1}")
    };
    cardfile_i18n::t_args(UNITS[u], &[("n", &n)])
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
        let r = misspelled_ranges("mail helo@exampel.org  www.exampel.net helo", |w| {
            w != "helo" && w != "exampel"
        });
        assert_eq!(r, vec![39..43]);
    }

    #[test]
    fn misspellings_scale_linearly() {
        // One huge token used to make every word rescan the whole text.
        let text = "ab".repeat(200_000) + &" helo".repeat(50_000);
        let r = misspelled_ranges(&text, |w| w != "helo");
        assert_eq!(r.len(), 50_000);
    }

    #[test]
    fn mime_and_size() {
        assert_eq!(mime_for_path(Path::new("a.PNG")), "image/png");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1536), "1.5 KB");
    }
}
