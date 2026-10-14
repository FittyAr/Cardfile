//! Localization for Cardfile.
//!
//! Strings live in `locales/<lang>.yml` at the repository root and are compiled in.
//! English (`en`) is the source language and the fallback for missing keys.

rust_i18n::i18n!("../../locales", fallback = "en");

mod visual;
pub use visual::{has_rtl, visual};

/// Locales written right to left.
const RTL_LOCALES: &[&str] = &["ar", "fa", "he", "ur"];

/// Whether the current locale is written right to left.
pub fn is_rtl_locale() -> bool {
    let locale = locale();
    let lang = locale.split(['-', '_']).next().unwrap_or_default();
    RTL_LOCALES.contains(&lang)
}

/// Prepare a UI string for display: right-to-left text is put in visual order,
/// because the UI toolkit lays text out left to right only.
fn for_display(s: String) -> String {
    if is_rtl_locale() || has_rtl(&s) {
        visual(&s)
    } else {
        s
    }
}

/// Translate `key` in the current locale, ready to show in the UI.
pub fn t(key: &str) -> String {
    for_display(t_raw(key))
}

/// Translate `key` in logical order, for text that is stored or exported (card
/// titles, the welcome card) rather than shown directly.
pub fn t_raw(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

/// Translate `key`, substitute `%{name}` placeholders and prepare it for display.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t_raw(key);
    for (name, value) in args {
        s = s.replace(&format!("%{{{name}}}"), value);
    }
    for_display(s)
}

/// Set the UI locale (e.g. `"en"`, `"es"`). Unknown locales fall back to English.
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

pub fn locale() -> String {
    rust_i18n::locale().to_string()
}

/// Locales compiled into the binary.
pub fn available_locales() -> Vec<String> {
    let mut v: Vec<String> = rust_i18n::available_locales!()
        .into_iter()
        .map(|l| l.to_string())
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_and_substitutes() {
        set_locale("en");
        assert_eq!(t("app.name"), "Cardfile");
        assert_eq!(t_args("test.greeting", &[("name", "Ada")]), "Hello, Ada!");
        assert!(available_locales().contains(&"en".to_owned()));
        assert!(!is_rtl_locale());
    }
}
