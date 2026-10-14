//! Drives the editor through GPUI's test harness: real key bindings and text input.

use cardfile_doc::{BlockKind, Document, Mark};
use cardfile_editor::RichEditor;
use gpui_kit::{Entity, Focusable as _, TestAppContext, VisualTestContext};

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
