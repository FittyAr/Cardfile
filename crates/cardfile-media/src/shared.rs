//! State shared between the [`Player`](crate::Player) handle and its worker threads.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::MediaKind;
use crate::audio::AudioShared;
use crate::video::VideoShared;

/// Wall clock used when there is no audio to follow (video without an audio
/// track, or after the audio track has ended).
#[derive(Debug)]
struct WallClock {
    /// Position at `anchor` (or the frozen position when paused).
    base: Duration,
    /// When playing: the instant `base` was taken.
    anchor: Option<Instant>,
}

impl WallClock {
    fn position(&self) -> Duration {
        match self.anchor {
            Some(anchor) => self.base + anchor.elapsed(),
            None => self.base,
        }
    }
}

/// Everything the handle and the worker threads both need.
pub(crate) struct Shared {
    pub(crate) kind: MediaKind,
    pub(crate) duration: Option<Duration>,
    pub(crate) video_size: Option<(u32, u32)>,
    pub(crate) audio: Option<Arc<AudioShared>>,
    pub(crate) video: Option<Arc<VideoShared>>,
    wall: Mutex<WallClock>,
    playing: AtomicBool,
    ended: AtomicBool,
    pub(crate) shutdown: AtomicBool,
}

impl Shared {
    pub(crate) fn new(
        kind: MediaKind,
        duration: Option<Duration>,
        video_size: Option<(u32, u32)>,
        audio: Option<Arc<AudioShared>>,
        video: Option<Arc<VideoShared>>,
    ) -> Self {
        Self {
            kind,
            duration,
            video_size,
            audio,
            video,
            wall: Mutex::new(WallClock {
                base: Duration::ZERO,
                anchor: None,
            }),
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

    /// The master clock.
    ///
    /// While the audio track is still producing samples its sample counter is
    /// the clock (so video follows what is heard); the wall clock is re-synced to
    /// it on every call so that, once the audio ends before the video does, the
    /// wall clock continues seamlessly from the last audio position.
    pub(crate) fn position(&self) -> Duration {
        let pos = match &self.audio {
            Some(audio) if !audio.is_drained() || self.kind == MediaKind::Audio => {
                let pos = audio.position();
                let mut wall = self.wall.lock();
                wall.base = pos;
                if wall.anchor.is_some() {
                    wall.anchor = Some(Instant::now());
                }
                pos
            }
            _ => self.wall.lock().position(),
        };
        match self.duration {
            Some(d) => pos.min(d),
            None => pos,
        }
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        let mut wall = self.wall.lock();
        if playing {
            if wall.anchor.is_none() {
                wall.anchor = Some(Instant::now());
            }
        } else {
            wall.base = wall.position();
            wall.anchor = None;
        }
        self.playing.store(playing, Ordering::Release);
        if let Some(audio) = &self.audio {
            audio.set_playing(playing);
        }
    }

    /// Moves every clock to `to` and clears the ended flag.
    pub(crate) fn seek_clock(&self, to: Duration) {
        let mut wall = self.wall.lock();
        wall.base = to;
        if wall.anchor.is_some() {
            wall.anchor = Some(Instant::now());
        }
        self.ended.store(false, Ordering::Release);
    }

    /// Called by a worker thread when the whole media has finished playing.
    pub(crate) fn mark_ended(&self) {
        if !self.ended.swap(true, Ordering::AcqRel) {
            self.set_playing(false);
        }
    }
}
