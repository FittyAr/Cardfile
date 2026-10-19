//! Exporting cards to Markdown, HTML, PDF and JSON, and opening attachments.

use std::path::{Path, PathBuf};

use cardfile_core::{CardFilter, CardId, UserId};
use cardfile_doc::{Document, MarkdownOptions};
use cardfile_i18n::{t, t_args};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{App, AppContext as _, PathPromptOptions, Window};
use serde::Serialize;

use crate::state::{AppState, DbBlobs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
    Pdf,
    Json,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Json => "json",
        }
    }
}

/// A filesystem-safe file name from a card title.
pub fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    let name: String = trimmed.chars().take(80).collect();
    if name.is_empty() { "card".into() } else { name }
}

#[derive(Serialize)]
struct JsonCard<'a> {
    id: String,
    title: &'a str,
    created_at: i64,
    updated_at: i64,
    tags: Vec<String>,
    document: &'a Document,
}

fn render(
    format: Format,
    title: &str,
    doc: &Document,
    blobs: &DbBlobs,
    relative_images: bool,
) -> anyhow::Result<Vec<u8>> {
    Ok(match format {
        Format::Markdown => {
            let opts = MarkdownOptions {
                html_for_extras: true,
                image_links_relative: relative_images,
                exact_heading_levels: false,
            };
            cardfile_doc::to_markdown(doc, Some(title), &opts, blobs).into_bytes()
        }
        Format::Html => cardfile_doc::to_html(doc, title, blobs).into_bytes(),
        Format::Pdf => cardfile_doc::pdf::to_pdf(doc, title, blobs)?,
        Format::Json => serde_json::to_vec_pretty(doc)?,
    })
}

fn notify_done(window: &mut Window, cx: &mut App, result: anyhow::Result<PathBuf>) {
    match result {
        Ok(path) => window.push_notification(
            Notification::success(t_args(
                "export.done",
                &[("path", &path.display().to_string())],
            )),
            cx,
        ),
        Err(e) => {
            tracing::error!("export: {e:#}");
            window.push_notification(
                Notification::error(t_args("export.failed", &[("error", &e.to_string())])),
                cx,
            );
        }
    }
}

/// Ask where to save one card and write it.
pub fn export_card(
    _id: CardId,
    title: String,
    doc: Document,
    format: Format,
    window: &mut Window,
    cx: &mut App,
) {
    let dir = directories::UserDirs::new()
        .and_then(|d| d.document_dir().map(Path::to_owned))
        .unwrap_or_else(|| PathBuf::from("."));
    let suggested = format!("{}.{}", file_name(&title), format.extension());
    let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
    let blobs = DbBlobs::new(AppState::db(cx));
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let task = cx.background_spawn(async move {
                let bytes = render(format, &title, &doc, &blobs, false)?;
                std::fs::write(&path, bytes)?;
                anyhow::Ok(path)
            });
            let result = task.await;
            let _ = cx.update(|window, cx| notify_done(window, cx, result));
        })
        .detach();
}

/// Export every live card of the user into a folder. Encrypted and private cards are skipped.
pub fn export_all(owner: UserId, format: Format, window: &mut Window, cx: &mut App) {
    let rx = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(t("export.choose_folder").into()),
    });
    let db = AppState::db(cx);
    window
        .spawn(cx, async move |cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(base) = paths.into_iter().next() else {
                return;
            };
            let task = cx.background_spawn(async move {
                let stamp = jiff::Zoned::now().strftime("%Y-%m-%d").to_string();
                let dir = base.join(format!("Cardfile export {stamp}"));
                std::fs::create_dir_all(&dir)?;
                let blobs = DbBlobs::new(db.clone());
                let tags = db.list_tags(owner)?;
                let summaries = db.list_cards(owner, &CardFilter::default())?;
                let mut skipped = 0;
                let mut json_cards = Vec::new();
                let mut docs = Vec::new();
                for s in &summaries {
                    if s.lock_mode.hides_content() {
                        skipped += 1;
                        continue;
                    }
                    let card = db.card(s.id)?;
                    let doc = Document::from_json(&card.content).unwrap_or_default();
                    docs.push((card, doc));
                }
                let mut used = std::collections::HashSet::new();
                for (card, doc) in &docs {
                    if format == Format::Json {
                        json_cards.push(JsonCard {
                            id: card.id.to_string(),
                            title: &card.title,
                            created_at: card.created_at,
                            updated_at: card.updated_at,
                            tags: tags
                                .iter()
                                .filter(|t| card.tags.contains(&t.id))
                                .map(|t| t.name.clone())
                                .collect(),
                            document: doc,
                        });
                        continue;
                    }
                    let mut name = file_name(&card.title);
                    let mut n = 2;
                    // Windows and macOS file names are case-insensitive.
                    while !used.insert(name.to_lowercase()) {
                        name = format!("{} ({n})", file_name(&card.title));
                        n += 1;
                    }
                    let bytes =
                        render(format, &card.title, doc, &blobs, format == Format::Markdown)?;
                    std::fs::write(dir.join(format!("{name}.{}", format.extension())), bytes)?;
                    if format == Format::Markdown {
                        for id in doc.blob_ids() {
                            if let Some((_, data)) = blobs.read(&id)? {
                                let bdir = dir.join("blobs");
                                std::fs::create_dir_all(&bdir)?;
                                std::fs::write(bdir.join(cardfile_doc::blob_file_name(&id)), data)?;
                            }
                        }
                    }
                }
                if format == Format::Json {
                    let json = serde_json::to_vec_pretty(&serde_json::json!({
                        "format": "cardfile-export",
                        "version": 1,
                        "cards": json_cards,
                    }))?;
                    std::fs::write(dir.join("cards.json"), json)?;
                }
                anyhow::Ok((dir, skipped))
            });
            let result = task.await;
            let _ = cx.update(|window, cx| match result {
                Ok((dir, skipped)) => {
                    if skipped > 0 {
                        window.push_notification(
                            Notification::info(t_args(
                                "export.skipped",
                                &[("n", &skipped.to_string())],
                            )),
                            cx,
                        );
                    }
                    notify_done(window, cx, Ok(dir));
                }
                Err(e) => notify_done(window, cx, Err(e)),
            });
        })
        .detach();
}

/// Save an attachment to a temporary folder and open it with the system handler.
pub fn open_attachment(blob: String, file_name: String, window: &mut Window, cx: &mut App) {
    let blobs = DbBlobs::new(AppState::db(cx));
    let task = cx.background_spawn(async move {
        let (_, data) = blobs
            .read(&blob)?
            .ok_or_else(|| anyhow::anyhow!("attachment not found"))?;
        let dir = std::env::temp_dir().join("cardfile-attachments");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(self::file_name(&file_name));
        std::fs::write(&path, data)?;
        // `that` (not `that_detached`) reports when no application is associated with
        // the file type, e.g. a video format and no media player installed.
        open::that(&path).map_err(|e| {
            tracing::warn!("open {}: {e}", path.display());
            anyhow::anyhow!(cardfile_i18n::t_args(
                "attachment.no_app",
                &[("name", &file_name)]
            ))
        })?;
        anyhow::Ok(())
    });
    window
        .spawn(cx, async move |cx| {
            if let Err(e) = task.await {
                let _ = cx.update(|window, cx| {
                    window.push_notification(Notification::error(e.to_string()), cx);
                });
            }
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn sanitizes_names() {
        assert_eq!(file_name("a/b:c*?"), "a_b_c__");
        assert_eq!(file_name("  "), "card");
        assert_eq!(file_name("Notas de café"), "Notas de café");
    }
}
