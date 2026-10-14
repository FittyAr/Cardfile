//! Cardfile's WYSIWYG rich text editor, built on GPUI.
//!
//! The editor edits a [`cardfile_doc::Document`] directly; users never see Markdown.
//! [`model`] holds the pure editing logic, the rest is the GPUI view.

mod dialogs;
pub mod editor;
mod element;
mod find;
pub mod history;
mod layout;
pub mod model;
pub mod pos;
mod render;
mod slash;
pub mod toolbar;
pub mod util;

pub use editor::{
    BlobStore, EditorEvent, KEY_CONTEXT, RichEditor, SpellChecker, default_key_bindings, init,
};
pub use element::parse_hex;
pub use model::Model;
pub use pos::{Pos, Selection, Slot};
pub use toolbar::EditorToolbar;
