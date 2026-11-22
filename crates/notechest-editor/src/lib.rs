//! NoteChest's WYSIWYG rich text editor, built on GPUI.
//!
//! The editor edits a [`notechest_doc::Document`] directly; users never see Markdown.
//! The `model` module holds the pure editing logic, the rest is the GPUI view.

pub mod bidi;
mod dialogs;
mod drag;
pub mod editor;
mod element;
mod find;
mod history;
mod image_resize;
mod layout;
mod media;
mod model;
mod pos;
mod render;
mod slash;
mod spell;
pub mod toolbar;
pub mod util;

pub use drag::DraggedBlock;
pub use editor::{
    BlobStore, EditorEvent, KEY_CONTEXT, RichEditor, SpellChecker, default_key_bindings, init,
};
pub use element::parse_hex;
pub use toolbar::EditorToolbar;
