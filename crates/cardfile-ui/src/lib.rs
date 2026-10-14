//! GPUI views for Cardfile.

use gpui_kit::component::*;
use gpui_kit::*;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, Cardfile!")
    }
}

/// Run the application.
pub fn run() -> anyhow::Result<()> {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        if let Err(err) =
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| Hello))
        {
            eprintln!("failed to open window: {err}");
        }
    });
    Ok(())
}
