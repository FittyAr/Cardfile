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
    /// The worker thread panicked (e.g. a decoder bug on a corrupt file): playback is
    /// over for good.
    failed: AtomicBool,
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
            failed: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub(crate) fn is_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire) || self.is_failed()
    }

    pub(crate) fn is_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    /// Called when the worker thread died: stop and stay ended.
    pub(crate) fn mark_failed(&self) {
        self.failed.store(true, Ordering::Release);
        self.ended.store(true, Ordering::Release);
        self.set_playing(false);
        self.audio.close();
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

    /// Restarts the clock at `to` and clears the ended flag, atomically with respect
    /// to [`Shared::mark_ended_if_drained`]: both run under the ring lock, so the worker
    /// can never mark the *old* generation's end after the seek. Returns the new
    /// generation.
    pub(crate) fn seek(&self, to: Duration) -> u64 {
        self.audio
            .begin_seek(to, || self.ended.store(false, Ordering::Release))
    }

    /// Called by the worker thread: marks the media ended if the output consumed every
    /// sample of the current generation.
    pub(crate) fn mark_ended_if_drained(&self) {
        self.audio.if_drained(|| {
            if !self.ended.swap(true, Ordering::AcqRel) {
                self.set_playing(false);
            }
        });
    }
}
