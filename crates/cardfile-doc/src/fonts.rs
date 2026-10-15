//! Fonts bundled with Cardfile so text looks the same on every system: Inter for
//! text and the interface, JetBrains Mono for code. Both are under the SIL Open
//! Font License 1.1 (`assets/fonts/*-LICENSE.txt`). Scripts they don't cover
//! (Chinese, Japanese, Korean, Arabic, Hebrew…) fall back to system fonts.

/// Family name of the text font.
pub const TEXT_FAMILY: &str = "Inter";
/// Family name of the monospace font.
pub const MONO_FAMILY: &str = "JetBrains Mono";

pub const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
pub const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
pub const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
pub const INTER_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");
pub const INTER_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/Inter-Italic.ttf");
pub const INTER_BOLD_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/Inter-BoldItalic.ttf");
pub const MONO_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
pub const MONO_BOLD: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf");
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
