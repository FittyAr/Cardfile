//! Asset source: NoteChest's own files plus the Lucide icon set from gpui-kit.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// Files the UI references as `notechest/<name>`: the origami NC symbol on a square
/// canvas (the charcoal wordmark of the full logo is unreadable on the dark themes,
/// and every screen that shows the logo also spells the name out).
const FILES: &[(&str, &[u8])] = &[
    (
        "notechest/logo.png",
        include_bytes!("../../../assets/brand/notechest/png/notechest-512.png"),
    ),
    (
        "notechest/icon.png",
        include_bytes!("../../../assets/brand/notechest/png/notechest-128.png"),
    ),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = FILES.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut out: Vec<SharedString> = FILES
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| (*p).into())
            .collect();
        out.extend(gpui_kit::assets::AllAssets.list(path)?);
        Ok(out)
    }
}
