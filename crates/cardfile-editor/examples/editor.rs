//! Standalone editor demo: `cargo run -p cardfile-editor --example editor`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cardfile_editor::{BlobStore, RichEditor};
use gpui_kit::*;

#[derive(Default)]
struct MemBlobs(Mutex<HashMap<String, (String, Vec<u8>)>>);

impl BlobStore for MemBlobs {
    fn put(&self, data: &[u8], mime: &str, _: Option<&str>) -> Option<String> {
        let id = format!(
            "{:x}",
            data.len() * 31 + data.first().copied().unwrap_or(0) as usize
        );
        self.0
            .lock()
            .ok()?
            .insert(id.clone(), (mime.to_owned(), data.to_vec()));
        Some(id)
    }
    fn get(&self, id: &str) -> Option<(String, Vec<u8>)> {
        self.0.lock().ok()?.get(id).cloned()
    }
}

struct Demo {
    editor: Entity<RichEditor>,
    toolbar: Entity<cardfile_editor::EditorToolbar>,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use gpui_kit::component::ActiveTheme as _;
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(self.toolbar.clone())
            .child(div().flex_1().min_h_0().child(self.editor.clone()))
    }
}

fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::AllAssets).run(|cx| {
        gpui_kit::init(cx);
        cardfile_editor::init(cx);
        let md = "# Welcome\n\nThis is **bold**, *italic* and `code`. Try typing `**bold**` or `# ` at the start of a line.\n\n- First\n  - Nested\n- Second\n\n1. One\n2. Two\n\n- [ ] Todo\n- [x] Done\n\n> A quote\n\n```rust\nfn main() {}\n```\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n---\n\nThe end.";
        let doc = cardfile_doc::from_markdown(md);
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(900.), px(700.)), cx))),
            ..Default::default()
        };
        let _ = gpui_kit::open_window(opts, cx, |window, cx| {
            let editor = cx.new(|cx| {
                let mut e = RichEditor::new(doc, cx);
                e.set_blob_store(Arc::new(MemBlobs::default()));
                e
            });
            editor.focus_handle(cx).focus(window, cx);
            let toolbar = cx.new(|cx| cardfile_editor::EditorToolbar::new(editor.clone(), cx));
            cx.new(|_| Demo { editor, toolbar })
        });
    });
}
