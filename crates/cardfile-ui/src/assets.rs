//! Asset source: Cardfile's own files plus the Lucide icon set from gpui-kit.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// Files under `assets/` that the UI references as `cardfile/<name>`.
const FILES: &[(&str, &[u8])] = &[
    (
        "cardfile/logo.png",
        include_bytes!("../../../assets/logo_trasparente.png"),
    ),
    (
        "cardfile/icon.png",
        include_bytes!("../../../assets/logo_trasparente_1.png"),
    ),
    (
        "cardfile/wordmark.png",
        include_bytes!("../../../assets/logo_trasparente_2.png"),
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
