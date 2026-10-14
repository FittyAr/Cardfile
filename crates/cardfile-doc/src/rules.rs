//! Markdown-style input rules: typing `# ` turns a paragraph into a heading,
//! `**bold**` becomes bold text, and so on. The user never sees the markers.

use std::ops::Range;

use crate::model::{BlockKind, Mark};

/// Result of a block-level rule: replace the block kind and drop `strip` leading bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockRule {
    pub kind: BlockKind,
    pub strip: usize,
}

/// Check the start of a paragraph after the user typed a space.
///
/// `text` is the paragraph text up to and including the caret.
pub fn block_rule_on_space(text: &str) -> Option<BlockRule> {
    let rule = |kind, strip| Some(BlockRule { kind, strip });
    match text {
        "# " => rule(BlockKind::Heading(1), 2),
        "## " => rule(BlockKind::Heading(2), 3),
        "### " => rule(BlockKind::Heading(3), 4),
        "- " | "* " | "+ " => rule(BlockKind::BulletList, 2),
        "> " => rule(BlockKind::Quote, 2),
        "[] " => rule(BlockKind::CheckList { checked: false }, 3),
        "[ ] " => rule(BlockKind::CheckList { checked: false }, 4),
        "[x] " | "[X] " => rule(BlockKind::CheckList { checked: true }, 4),
        _ => {
            // "1. " / "1) "
            let digits = text.bytes().take_while(u8::is_ascii_digit).count();
            let rest = &text[digits..];
            if digits > 0 && digits <= 3 && (rest == ". " || rest == ") ") {
                rule(BlockKind::NumberedList, text.len())
            } else {
                None
            }
        }
    }
}

/// Check a whole paragraph when the user presses Enter.
pub fn block_rule_on_enter(text: &str) -> Option<BlockRule> {
    let t = text.trim_end();
    if t == "---" || t == "***" || t == "___" {
        return Some(BlockRule {
            kind: BlockKind::Divider,
            strip: text.len(),
        });
    }
    if let Some(lang) = t.strip_prefix("```")
        && !lang.contains('`')
        && !lang.contains(' ')
    {
        return Some(BlockRule {
            kind: BlockKind::Code {
                language: lang.to_owned(),
            },
            strip: text.len(),
        });
    }
    None
}

/// An inline rule match: remove the marker ranges and apply `mark` to `content`.
///
/// Ranges are byte offsets in the text *before* any removal; `content` is where the
/// marked text ends up *after* the markers are removed.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineRule {
    pub open: Range<usize>,
    pub close: Range<usize>,
    pub content: Range<usize>,
    pub mark: Mark,
}

/// Check for a completed inline marker pair ending at `caret` (right after the user
/// typed the closing character).
pub fn inline_rule(text: &str, caret: usize) -> Option<InlineRule> {
    let before = text.get(..caret)?;
    // Longest delimiters first.
    const DELIMS: [(&str, Mark); 5] = [
        ("**", Mark::Bold),
        ("__", Mark::Bold),
        ("~~", Mark::Strike),
        ("*", Mark::Italic),
        ("`", Mark::Code),
    ];
    for (delim, mark) in DELIMS {
        let Some(head) = before.strip_suffix(delim) else {
            continue;
        };
        // For single `*`, don't fire on the second char of an unfinished `**`.
        if delim == "*" && head.ends_with('*') {
            continue;
        }
        let Some(open_at) = head.rfind(delim) else {
            continue;
        };
        let inner = &head[open_at + delim.len()..];
        if inner.is_empty() || inner.starts_with(' ') || inner.ends_with(' ') {
            continue;
        }
        if delim == "*" && (inner.starts_with('*') || head[..open_at].ends_with('*')) {
            continue;
        }
        // Opening delimiter must start a word.
        if head[..open_at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        let d = delim.len();
        let close_start = caret - d;
        return Some(InlineRule {
            open: open_at..open_at + d,
            close: close_start..caret,
            content: open_at..open_at + inner.len(),
            mark,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_rules() {
        assert_eq!(
            block_rule_on_space("## ").map(|r| r.kind),
            Some(BlockKind::Heading(2))
        );
        assert_eq!(block_rule_on_space("12. ").map(|r| r.strip), Some(4));
        assert_eq!(block_rule_on_space("a "), None);
        assert_eq!(
            block_rule_on_enter("```rust").map(|r| r.kind),
            Some(BlockKind::Code {
                language: "rust".into()
            })
        );
        assert_eq!(
            block_rule_on_enter("---").map(|r| r.kind),
            Some(BlockKind::Divider)
        );
    }

    #[test]
    fn inline_bold() {
        let text = "say **hi**";
        let r = inline_rule(text, text.len()).unwrap();
        assert_eq!(r.mark, Mark::Bold);
        assert_eq!(&text[r.open.clone()], "**");
        assert_eq!(r.close, 8..10);
        assert_eq!(r.content, 4..6);
    }

    #[test]
    fn inline_italic_not_on_bold_prefix() {
        assert!(inline_rule("**hi*", 5).is_none());
        let r = inline_rule("an *em*", 7).unwrap();
        assert_eq!(r.mark, Mark::Italic);
    }

    #[test]
    fn inline_requires_word_start_and_content() {
        assert!(inline_rule("a**b**", 6).is_none());
        assert!(inline_rule("** **", 5).is_none());
        assert!(inline_rule("``", 2).is_none());
        assert_eq!(inline_rule("`x`", 3).map(|r| r.mark), Some(Mark::Code));
    }
}
