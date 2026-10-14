//! The card created on first run.

use cardfile_doc::Document;

pub fn welcome_document() -> Document {
    cardfile_doc::from_markdown(&cardfile_i18n::t("welcome.body"))
}
