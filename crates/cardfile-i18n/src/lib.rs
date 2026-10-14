//! Localization for Cardfile.
//!
//! Strings live in `locales/<lang>.yml` at the repository root and are compiled in.
//! English (`en`) is the source language and the fallback for missing keys.

rust_i18n::i18n!("../../locales", fallback = "en");

/// Translate `key` in the current locale.
pub fn t(key: &str) -> String {
    rust_i18n::t!(key).into_owned()
}

/// Translate `key` and substitute `%{name}` placeholders.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (name, value) in args {
        s = s.replace(&format!("%{{{name}}}"), value);
    }
    s
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
    }
}
