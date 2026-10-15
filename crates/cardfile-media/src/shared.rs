//! State shared between the [`Player`](crate::Player) handle and its worker thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::MediaKind;
use crate::audio::AudioShared;

/// Everything the handle and the worker thread both need.
pub(crate) struct Shared {
    pub(crate) kind: MediaKind,
    pub(crate) duration: Option<Duration>,
    pub(crate) audio: Arc<AudioShared>,
    playing: AtomicBool,
    ended: AtomicBool,
    pub(crate) shutdown: AtomicBool,
}

impl Shared {
    pub(crate) fn new(
        kind: MediaKind,
        duration: Option<Duration>,
        audio: Arc<AudioShared>,
    ) -> Self {
        Self {
            kind,
            duration,
            audio,
            playing: AtomicBool::new(false),
            ended: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub(crate) fn is_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }

    pub(crate) fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    /// The playback clock: the audio sample counter, clamped to the duration.
    pub(crate) fn position(&self) -> Duration {
        let pos = self.audio.position();
        match self.duration {
            Some(d) => pos.min(d),
            None => pos,
        }
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
        self.audio.set_playing(playing);
    }

    /// Clears the ended flag (after a seek).
    pub(crate) fn clear_ended(&self) {
        self.ended.store(false, Ordering::Release);
    }

    /// Called by the worker thread when the whole media has finished playing.
    pub(crate) fn mark_ended(&self) {
        if !self.ended.swap(true, Ordering::AcqRel) {
            self.set_playing(false);
        }
    }
}
