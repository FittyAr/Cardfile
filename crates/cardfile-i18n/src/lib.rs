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
    for_display(substitute(&t_raw(key), args))
}

/// Replace every `%{name}` in `template` in a single pass. Values are inserted
/// verbatim: a value that itself contains `%{other}` (a card title, a file name) is
/// never expanded by a later argument.
fn substitute(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("%{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let value = after
            .find('}')
            .and_then(|end| Some((args.iter().find(|(n, _)| *n == &after[..end])?.1, end)));
        match value {
            Some((value, end)) => {
                out.push_str(value);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str("%{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Set the UI locale (e.g. `"en"`, `"es"`). Unknown locales fall back to English.
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// The current UI locale code (e.g. `"en"`).
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

    #[test]
    fn substitution_is_single_pass() {
        let args = [("title", "%{count} cards"), ("count", "3")];
        assert_eq!(substitute("%{title}: %{count}", &args), "%{count} cards: 3");
        // Unknown and unterminated placeholders stay as they are.
        assert_eq!(substitute("%{nope} %{count", &args), "%{nope} %{count");
        assert_eq!(substitute("100%{count}%", &args), "1003%");
    }
}
