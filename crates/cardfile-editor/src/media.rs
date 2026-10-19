//! Inline playback of audio and video attachments.
//!
//! Players are created when the user first presses play (the attachment bytes are
//! loaded then) and dropped when another document is shown. While something plays,
//! a short timer repaints the editor so the progress bar and video frames advance.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use cardfile_doc::Attachment;
use cardfile_media::{MediaError, MediaKind, PlaybackState, Player};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, AppContext as _, Bounds, Context, CursorStyle, Entity, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels, Styled as _,
    Task, canvas, div, px,
};

use crate::editor::RichEditor;

/// Players and decoded frames of the current document, keyed by blob reference.
#[derive(Default)]
pub(crate) struct MediaState {
    players: HashMap<String, Player>,
    /// Blobs that failed to open (unsupported codec, malformed data), with a
    /// localized message.
    failed: HashMap<String, String>,
    /// Blobs whose player is being opened on the background executor.
    loading: HashSet<String>,
    tick: Option<Task<()>>,
}

impl MediaState {
    /// Stop everything (a different document is shown).
    pub(crate) fn clear(&mut self) {
        for p in self.players.values() {
            p.pause();
        }
        self.players.clear();
        self.failed.clear();
        // A load still in flight finds its blob gone from `loading` and is ignored.
        self.loading.clear();
        self.tick = None;
    }

    fn any_playing(&self) -> bool {
        self.players
            .values()
            .any(|p| p.state() == PlaybackState::Playing)
    }
}

/// Whether an attachment can play inline.
pub(crate) fn playable(att: &Attachment) -> Option<MediaKind> {
    cardfile_media::media_kind(&att.mime, Some(&att.file_name))
}

fn format_time(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// What to do with a player once it has loaded.
#[derive(Clone, Copy)]
enum AfterLoad {
    Toggle,
    Seek(f32),
}

/// A player-opening failure as a localized message for the UI.
fn error_message(e: &MediaError) -> String {
    let key = match e {
        MediaError::Unsupported(_) => "editor.media.unsupported",
        MediaError::Malformed(_) => "editor.media.malformed",
        _ => "editor.media.failed",
    };
    cardfile_i18n::t(key)
}

impl RichEditor {
    /// The player for `att`, once loaded. When it isn't, loading starts (the blob
    /// read and container parsing run on the background executor) and `then` is
    /// applied when it is ready.
    fn player_or_load(
        &mut self,
        att: &Attachment,
        then: AfterLoad,
        cx: &mut Context<Self>,
    ) -> Option<Player> {
        if let Some(p) = self.media.players.get(&att.blob) {
            return Some(p.clone());
        }
        if self.media.failed.contains_key(&att.blob) {
            // Unsupported inline: hand it to the system player.
            if matches!(then, AfterLoad::Toggle) {
                Self::open_externally(att, cx);
            }
            return None;
        }
        if !self.media.loading.insert(att.blob.clone()) {
            return None;
        }
        let store = self.blobs.clone();
        let blob = att.blob.clone();
        let att_mime = att.mime.clone();
        let file_name = att.file_name.clone();
        let task = cx.background_spawn(async move {
            let (mime, bytes) = store?.get(&blob)?;
            let mime = if mime == "application/octet-stream" {
                att_mime
            } else {
                mime
            };
            Some(Player::open(Arc::from(bytes), &mime, Some(&file_name)))
        });
        let att = att.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.media.loading.remove(&att.blob) {
                    // Another document was shown meanwhile.
                    return;
                }
                match result {
                    Some(Ok(p)) => {
                        this.media.players.insert(att.blob.clone(), p.clone());
                        this.apply_after_load(&att, &p, then, cx);
                    }
                    Some(Err(e)) => {
                        tracing::warn!("cannot play {}: {e}", att.file_name);
                        this.media
                            .failed
                            .insert(att.blob.clone(), error_message(&e));
                        if matches!(then, AfterLoad::Toggle) {
                            Self::open_externally(&att, cx);
                        }
                    }
                    // The data is missing: let the system try.
                    None if matches!(then, AfterLoad::Toggle) => Self::open_externally(&att, cx),
                    None => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
        None
    }

    fn open_externally(att: &Attachment, cx: &mut Context<Self>) {
        cx.emit(crate::editor::EditorEvent::OpenAttachment {
            blob: att.blob.clone(),
            file_name: att.file_name.clone(),
        });
    }

    fn apply_after_load(
        &mut self,
        att: &Attachment,
        p: &Player,
        then: AfterLoad,
        cx: &mut Context<Self>,
    ) {
        match then {
            AfterLoad::Toggle => {
                // One thing plays at a time.
                for (blob, other) in &self.media.players {
                    if *blob != att.blob {
                        other.pause();
                    }
                }
                p.toggle();
                self.ensure_media_tick(cx);
            }
            AfterLoad::Seek(fraction) => {
                if let Some(total) = p.duration() {
                    p.seek(total.mul_f32(fraction.clamp(0., 1.)));
                    self.ensure_media_tick(cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn media_toggle(&mut self, att: &Attachment, cx: &mut Context<Self>) {
        if let Some(p) = self.player_or_load(att, AfterLoad::Toggle, cx) {
            self.apply_after_load(att, &p, AfterLoad::Toggle, cx);
        }
    }

    pub(crate) fn media_seek(&mut self, att: &Attachment, fraction: f32, cx: &mut Context<Self>) {
        let then = AfterLoad::Seek(fraction);
        if let Some(p) = self.player_or_load(att, then, cx) {
            self.apply_after_load(att, &p, then, cx);
        }
    }

    fn media_mute(&mut self, att: &Attachment, cx: &mut Context<Self>) {
        if let Some(p) = self.media.players.get(&att.blob) {
            p.set_volume(if p.volume() > 0. { 0. } else { 1. });
            cx.notify();
        }
    }

    /// Repaint while anything plays; the task ends itself when playback stops.
    fn ensure_media_tick(&mut self, cx: &mut Context<Self>) {
        if self.media.tick.is_some() {
            return;
        }
        self.media.tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let keep = this.update(cx, |this, cx| {
                    cx.notify();
                    let playing = this.media.any_playing();
                    if !playing {
                        this.media.tick = None;
                    }
                    playing
                });
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        }));
    }

    /// The inline player for an audio attachment.
    pub(crate) fn render_media(
        &mut self,
        ix: usize,
        att: &Attachment,
        _kind: MediaKind,
        selected: bool,
        colors: MediaColors,
        entity: &Entity<Self>,
    ) -> AnyElement {
        let player = self.media.players.get(&att.blob).cloned();
        let failed = self.media.failed.get(&att.blob).cloned();
        let state = player.as_ref().map(|p| p.state());
        let playing = state == Some(PlaybackState::Playing);
        let position = player.as_ref().map(|p| p.position()).unwrap_or_default();
        let duration = player.as_ref().and_then(|p| p.duration());
        let fraction = match duration {
            Some(d) if !d.is_zero() => (position.as_secs_f32() / d.as_secs_f32()).clamp(0., 1.),
            _ => 0.,
        };
        let muted = player.as_ref().is_some_and(|p| p.volume() == 0.);

        let toggle_att = att.clone();
        let ent = entity.clone();
        let play = div()
            .id(("media-play", ix))
            .size(px(30.))
            .flex_shrink_0()
            .rounded_full()
            .bg(colors.primary)
            .text_color(gpui_kit::white())
            .flex()
            .items_center()
            .justify_center()
            .cursor(CursorStyle::PointingHand)
            .child(if playing { "❚❚" } else { "▶" })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                ent.update(cx, |e, cx| e.media_toggle(&toggle_att, cx));
            });

        // Click anywhere on the bar to seek; its bounds are captured while painting.
        let bounds: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
        let paint_bounds = bounds.clone();
        let seek_att = att.clone();
        let ent = entity.clone();
        let bar = div()
            .id(("media-bar", ix))
            .relative()
            .flex_1()
            .h(px(6.))
            .rounded_full()
            .bg(colors.track)
            .cursor(CursorStyle::PointingHand)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(gpui_kit::relative(fraction))
                    .rounded_full()
                    .bg(colors.primary),
            )
            .child(
                canvas(move |b, _, _| paint_bounds.set(b), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_mouse_down(MouseButton::Left, move |ev, _, cx| {
                cx.stop_propagation();
                let b = bounds.get();
                if b.size.width > px(0.) {
                    let f = (ev.position.x - b.origin.x) / b.size.width;
                    ent.update(cx, |e, cx| e.media_seek(&seek_att, f, cx));
                }
            });

        let mute_att = att.clone();
        let ent = entity.clone();
        let time = match duration {
            _ if self.media.loading.contains(&att.blob) => cardfile_i18n::t("editor.media.loading"),
            Some(d) => format!("{} / {}", format_time(position), format_time(d)),
            None => format_time(position),
        };
        let controls = h_flex()
            .gap(px(10.))
            .items_center()
            .child(play)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(colors.fg)
                                    .child(att.file_name.clone()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(px(11.))
                                    .text_color(colors.muted)
                                    .child(time),
                            ),
                    )
                    .child(bar),
            )
            .child(
                div()
                    .id(("media-mute", ix))
                    .flex_shrink_0()
                    .cursor(CursorStyle::PointingHand)
                    .text_color(colors.muted)
                    .child(if muted { "🔇" } else { "🔊" })
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        ent.update(cx, |e, cx| e.media_mute(&mute_att, cx));
                    }),
            );

        let open_att = att.clone();
        let ent = entity.clone();
        v_flex()
            .gap(px(8.))
            .p(px(10.))
            .rounded(px(8.))
            .border_1()
            .border_color(if selected {
                colors.primary
            } else {
                colors.border
            })
            .child(controls)
            .when_some(failed, |d, err| {
                d.child(
                    h_flex()
                        .id(("media-open", ix))
                        .gap(px(6.))
                        .text_size(px(11.))
                        .text_color(colors.muted)
                        .cursor(CursorStyle::PointingHand)
                        .child(format!("{err} — "))
                        .child(
                            div()
                                .text_color(colors.primary)
                                .child(cardfile_i18n::t("editor.media.open_external")),
                        )
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            cx.stop_propagation();
                            ent.update(cx, |_, cx| {
                                cx.emit(crate::editor::EditorEvent::OpenAttachment {
                                    blob: open_att.blob.clone(),
                                    file_name: open_att.file_name.clone(),
                                })
                            });
                        }),
                )
            })
            .into_any_element()
    }
}

/// Theme colours the player needs.
#[derive(Clone, Copy)]
pub(crate) struct MediaColors {
    pub fg: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub border: Hsla,
    pub track: Hsla,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(format_time(Duration::from_secs(5)), "0:05");
        assert_eq!(format_time(Duration::from_secs(125)), "2:05");
        assert_eq!(format_time(Duration::from_secs(3725)), "1:02:05");
    }
}
