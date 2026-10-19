//! Drives the editor through GPUI's test harness: real key bindings and text input.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cardfile_doc::{Block, BlockKind, Document, Image, Mark};
use cardfile_editor::{BlobStore, RichEditor};
use gpui_kit::{ClipboardItem, Entity, Focusable as _, TestAppContext, VisualTestContext};

fn setup(cx: &mut TestAppContext, doc: Document) -> (Entity<RichEditor>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cardfile_editor::init(cx);
    });
    let (editor, vcx) = cx.add_window_view(|_, cx| RichEditor::new(doc, cx));
    vcx.update(|window, cx| {
        let focus = editor.focus_handle(cx);
        focus.focus(window, cx);
    });
    vcx.run_until_parked();
    (editor, vcx)
}

fn texts(editor: &Entity<RichEditor>, cx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(cx, |e, _| {
        e.document()
            .blocks
            .iter()
            .map(|b| b.text.text.clone())
            .collect()
    })
}

#[gpui_kit::test]
fn typing_enter_and_backspace(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    cx.simulate_input("hello");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("world");
    assert_eq!(texts(&editor, cx), ["hello", "world"]);
    cx.simulate_keystrokes("backspace backspace");
    assert_eq!(texts(&editor, cx), ["hello", "wor"]);
    cx.simulate_keystrokes("ctrl-z");
    assert_eq!(texts(&editor, cx), ["hello", "world"]);
}

#[gpui_kit::test]
fn shortcuts_format_text(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    cx.simulate_input("abc");
    cx.simulate_keystrokes("shift-left shift-left ctrl-b");
    editor.read_with(cx, |e, _| {
        assert!(e.document().blocks[0].text.has_mark(1..3, Mark::Bold));
        assert!(!e.document().blocks[0].text.has_mark(0..1, Mark::Bold));
    });
    cx.simulate_keystrokes("ctrl-alt-2");
    editor.read_with(cx, |e, _| {
        assert_eq!(e.document().blocks[0].kind, BlockKind::Heading(2))
    });
}

#[gpui_kit::test]
fn markdown_shortcuts_while_typing(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    for ch in ["-", " ", "x"] {
        cx.simulate_input(ch);
    }
    editor.read_with(cx, |e, _| {
        assert_eq!(e.document().blocks[0].kind, BlockKind::BulletList)
    });
    cx.simulate_keystrokes("enter tab");
    cx.simulate_input("y");
    editor.read_with(cx, |e, _| assert_eq!(e.document().blocks[1].indent, 1));
}

#[gpui_kit::test]
fn read_only_blocks_edits(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::from_plain_text("fixed"));
    editor.update(cx, |e, cx| e.set_read_only(true, cx));
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace enter");
    assert_eq!(texts(&editor, cx), ["fixed"]);
}

#[gpui_kit::test]
fn copy_paste_keeps_formatting(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    cx.simulate_input("ab");
    cx.simulate_keystrokes("ctrl-a ctrl-b ctrl-c right");
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(texts(&editor, cx), ["abab"]);
    editor.read_with(cx, |e, _| {
        assert!(e.document().blocks[0].text.has_mark(0..4, Mark::Bold))
    });
}

/// Counts writes; hands back what was stored.
#[derive(Default)]
struct Store {
    puts: AtomicUsize,
    data: Mutex<Vec<(String, Vec<u8>)>>,
}

impl BlobStore for Store {
    fn put(&self, data: &[u8], mime: &str, _: Option<&str>) -> Option<String> {
        let n = self.puts.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut d) = self.data.lock() {
            d.push((mime.to_owned(), data.to_vec()));
        }
        Some(format!("blob{n}"))
    }
    fn get(&self, id: &str) -> Option<(String, Vec<u8>)> {
        let n: usize = id.strip_prefix("blob")?.parse().ok()?;
        self.data.lock().ok()?.get(n).cloned()
    }
}

#[gpui_kit::test]
fn inserting_files_stores_in_the_background(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    let store = Arc::new(Store::default());
    editor.update(cx, |e, cx| {
        e.set_blob_store(store.clone());
        e.insert_attachment_bytes(b"abc", "text/plain", "a.txt", cx);
    });
    cx.run_until_parked();
    assert_eq!(store.puts.load(Ordering::SeqCst), 1);
    editor.read_with(cx, |e, _| {
        assert!(
            e.document()
                .blocks
                .iter()
                .any(|b| matches!(&b.kind, BlockKind::Attachment(a) if a.blob == "blob0"))
        );
    });
}

#[gpui_kit::test]
fn read_only_writes_no_blobs(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::from_plain_text("fixed"));
    let store = Arc::new(Store::default());
    editor.update(cx, |e, cx| {
        e.set_blob_store(store.clone());
        e.set_read_only(true, cx);
        e.insert_image_bytes(b"png", "image/png", None, cx);
        e.insert_attachment_bytes(b"abc", "text/plain", "a.txt", cx);
    });
    // A rich fragment with an image on the clipboard.
    let mut frag = Document::from_plain_text("x");
    frag.blocks.push(Block::new(
        BlockKind::Image(Image {
            blob: "blob0".into(),
            mime: "image/png".into(),
            width: None,
            height: None,
            alt: String::new(),
        }),
        "",
    ));
    cx.write_to_clipboard(ClipboardItem::new_string_with_json_metadata(
        "x".into(),
        frag,
    ));
    cx.simulate_keystrokes("ctrl-v");
    cx.run_until_parked();
    assert_eq!(store.puts.load(Ordering::SeqCst), 0);
    assert_eq!(texts(&editor, cx), ["fixed"]);
}

#[gpui_kit::test]
fn markdown_pasted_into_code_block_stays_verbatim(cx: &mut TestAppContext) {
    let mut doc = Document::from_plain_text("");
    doc.blocks[0].kind = BlockKind::Code {
        language: String::new(),
    };
    let (editor, cx) = setup(cx, doc);
    let md = "# title\n- **item**";
    cx.write_to_clipboard(ClipboardItem::new_string(md.into()));
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(texts(&editor, cx), [md]);
}

#[gpui_kit::test]
fn slash_menu_inserts_heading(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    cx.simulate_input("/");
    cx.simulate_input("h");
    cx.simulate_input("2");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("Title");
    editor.read_with(cx, |e, _| {
        assert_eq!(e.document().blocks[0].kind, BlockKind::Heading(2));
        assert_eq!(e.document().blocks[0].text.text, "Title");
    });
}

#[gpui_kit::test]
fn slash_menu_escape_keeps_text(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::default());
    cx.simulate_input("/");
    cx.simulate_keystrokes("escape enter");
    assert_eq!(texts(&editor, cx), ["/", ""]);
}

#[gpui_kit::test]
fn find_selects_matches(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx, Document::from_plain_text("one two one"));
    cx.simulate_keystrokes("ctrl-home ctrl-f");
    cx.simulate_input("one");
    cx.simulate_keystrokes("escape");
    // Escape closes the bar and returns to the editor with the first match selected.
    editor.read_with(cx, |e, _| assert_eq!(e.selected_text(), "one"));
    cx.simulate_input("ONE");
    assert_eq!(texts(&editor, cx), ["ONE two one"]);
}
