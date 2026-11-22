//! The card created on first run.

use notechest_doc::Document;

pub fn welcome_document() -> Document {
    notechest_doc::from_markdown(&notechest_i18n::t_raw("welcome.body"))
}
