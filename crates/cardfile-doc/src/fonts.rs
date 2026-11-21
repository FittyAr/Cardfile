//! Fonts bundled with Cardfile so text looks the same on every system: Inter for
//! text and the interface, JetBrains Mono for code. Both are under the SIL Open
//! Font License 1.1 (`assets/fonts/*-LICENSE.txt`). Scripts they don't cover
//! (Chinese, Japanese, Korean, Arabic, Hebrew…) fall back to system fonts.

/// Family name of the text font.
pub const TEXT_FAMILY: &str = "Inter";
/// Family name of the monospace font.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// Inter Regular (weight 400).
pub const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
/// Inter Medium (weight 500).
pub const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
/// Inter SemiBold (weight 600).
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
/// Inter Bold (weight 700).
pub const INTER_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");
/// Inter Italic (weight 400).
pub const INTER_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/Inter-Italic.ttf");
/// Inter Bold Italic (weight 700).
pub const INTER_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/Inter-BoldItalic.ttf");
/// JetBrains Mono Regular, for code.
pub const MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
/// JetBrains Mono Bold, for bold code.
pub const MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
/// JetBrains Mono Italic, for italic code.
pub const MONO_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf");

/// Every bundled font file.
pub const ALL: &[&[u8]] = &[
    INTER_REGULAR,
    INTER_MEDIUM,
    INTER_SEMIBOLD,
    INTER_BOLD,
    INTER_ITALIC,
    INTER_BOLD_ITALIC,
    MONO_REGULAR,
    MONO_BOLD,
    MONO_ITALIC,
];
