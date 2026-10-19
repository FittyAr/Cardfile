//! Inline audio playback for Cardfile attachments.
//!
//! This crate is a pure engine: it knows nothing about GPUI or the rest of
//! Cardfile. The UI hands it the attachment bytes and polls a [`Player`] every
//! frame for its position and state.
//!
//! # Supported media
//!
//! Only patent-free audio formats are played inline:
//!
//! | Kind  | Formats | Implementation |
//! |-------|---------|----------------|
//! | Audio | MP3, WAV (PCM), OGG/Vorbis, FLAC | `symphonia` decode, `rodio`/`cpal` output |
//!
//! Everything else (all video, AAC/M4A, Opus, ...) is reported as unsupported —
//! [`media_kind`] returns `None`, or [`Player::open`] fails with
//! [`MediaError::Unsupported`] — so the UI can open the file in the system
//! player instead. Patent-encumbered codecs (H.264, AAC) are deliberately not
//! included, and no GPL/LGPL code (ffmpeg, GStreamer, libvlc) is used.
//!
//! # Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use cardfile_media::{media_kind, Player};
//!
//! # fn main() -> Result<(), cardfile_media::MediaError> {
//! let bytes: Arc<[u8]> = std::fs::read("song.mp3").unwrap_or_default().into();
//! if media_kind("audio/mpeg", Some("song.mp3")).is_some() {
//!     let player = Player::open(bytes, "audio/mpeg", Some("song.mp3"))?;
//!     player.play();
//!     // Every UI frame:
//!     let _pos = player.position();
//!     let _state = player.state();
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Threads, clocks and memory
//!
//! * [`Player::open`] only parses headers. Decoding happens on one audio thread
//!   per player; every method of [`Player`] is cheap (atomics or a short mutex)
//!   and safe to call from the UI thread each frame.
//! * The audio output device is opened lazily on the first [`Player::play`].
//!   The clock is the number of audio frames handed to the device, so the
//!   position always matches what is heard.
//! * **No output device** (CI, headless Linux, no sound card): `open` and
//!   `play` still succeed. Audio is decoded and consumed silently at real-time
//!   speed, so position, seeking and the `Ended` state behave exactly as with a
//!   device. The same behaviour can be forced with [`AudioOutput::Null`].
//! * Memory is bounded: about half a second of decoded audio is kept ahead of
//!   the clock. The attachment bytes are shared via the `Arc<[u8]>` passed to
//!   `open`, never copied.
//!
//! # Limitations
//!
//! * If the output device disappears while playing, the clock stops until the
//!   player is reopened.

mod audio;
mod kind;
mod shared;

use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

pub use kind::{MediaKind, media_kind};

use audio::{AudioCmd, AudioShared};
use shared::Shared;

/// Errors from [`Player::open`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MediaError {
    /// The format or codec cannot be played inline; open it externally instead.
    #[error("unsupported media: {0}")]
    Unsupported(String),
    /// The file looks like a supported format but could not be parsed.
    #[error("malformed media: {0}")]
    Malformed(String),
    /// A worker thread could not be started.
    #[error("cannot start media thread: {0}")]
    Thread(#[from] std::io::Error),
}

/// Playback state as shown by the UI's play/pause button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackState {
    /// Not playing (initial state, or after [`Player::pause`]).
    Paused,
    /// Playing.
    Playing,
    /// Reached the end. [`Player::play`] restarts from the beginning.
    Ended,
}

/// Where a player sends its audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioOutput {
    /// The system's default output device, falling back to [`AudioOutput::Null`]
    /// when there is none.
    #[default]
    Default,
    /// No device: audio is decoded and discarded at real-time speed. Useful for
    /// tests and for muted previews.
    Null,
}

/// Options for [`Player::open_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PlayerOptions {
    /// Audio output selection.
    pub audio_output: AudioOutput,
}

impl PlayerOptions {
    /// Sets [`PlayerOptions::audio_output`].
    #[must_use]
    pub fn audio_output(mut self, output: AudioOutput) -> Self {
        self.audio_output = output;
        self
    }
}

/// Owns the worker thread; dropped when the last [`Player`] clone goes away.
struct Handle {
    shared: Arc<Shared>,
    audio_tx: Sender<AudioCmd>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared
            .shutdown
            .store(true, std::sync::atomic::Ordering::Release);
        self.shared.audio.set_playing(false);
        self.shared.audio.close();
        // Wake the thread so it notices the flag right away. It is not joined,
        // so dropping never blocks the UI thread; it exits within one decode
        // step and releases the audio device.
        let _ = self.audio_tx.send(AudioCmd::Wake);
    }
}

/// A media player for one attachment.
///
/// Cheap to clone (an `Arc` inside), `Send + Sync`. All decoding runs on the
/// player's own thread, which stops (and the audio device is released) when the
/// last clone is dropped.
#[derive(Clone)]
pub struct Player {
    handle: Arc<Handle>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("kind", &self.kind())
            .field("state", &self.state())
            .field("position", &self.position())
            .field("duration", &self.duration())
            .finish()
    }
}

impl Player {
    /// Opens `data` for playback with default options. See [`Player::open_with`].
    pub fn open(data: Arc<[u8]>, mime: &str, file_name: Option<&str>) -> Result<Self, MediaError> {
        Self::open_with(data, mime, file_name, PlayerOptions::default())
    }

    /// Opens `data` for playback, starting paused at position zero.
    ///
    /// Only container headers are parsed here; decoding starts on a background
    /// thread. Whether the attachment is accepted at all is decided by
    /// [`media_kind`].
    ///
    /// # Errors
    ///
    /// * [`MediaError::Unsupported`] if the media is not inline-playable (unknown
    ///   or patent-encumbered type, or a codec other than MP3, PCM, Vorbis or
    ///   FLAC inside a supported container, e.g. Opus in OGG).
    /// * [`MediaError::Malformed`] if the container cannot be parsed.
    /// * [`MediaError::Thread`] if the worker thread cannot be spawned.
    ///
    /// A missing audio output device is *not* an error (see the crate docs).
    pub fn open_with(
        data: Arc<[u8]>,
        mime: &str,
        file_name: Option<&str>,
        options: PlayerOptions,
    ) -> Result<Self, MediaError> {
        let kind = media_kind(mime, file_name)
            .ok_or_else(|| MediaError::Unsupported(format!("type {mime:?}")))?;
        let ext = kind::extension(file_name);

        // Probing runs third-party parsers over untrusted bytes: a panic there must
        // not take the caller (the UI thread) down.
        let track = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            audio::open_track(data, mime, ext.as_deref())
        }))
        .map_err(|_| MediaError::Malformed("the media parser crashed".to_owned()))??;
        let audio = Arc::new(AudioShared::new(track.rate, track.channels));
        let shared = Arc::new(Shared::new(kind, track.duration, audio.clone()));

        let (tx, rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let output = options.audio_output;
        std::thread::Builder::new()
            .name("cardfile-media-audio".to_owned())
            .spawn(move || audio::run(thread_shared, audio, track, rx, output))?;
        Ok(Self {
            handle: Arc::new(Handle {
                shared,
                audio_tx: tx,
            }),
        })
    }

    fn shared(&self) -> &Shared {
        &self.handle.shared
    }

    /// What kind of media this player plays.
    pub fn kind(&self) -> MediaKind {
        self.shared().kind
    }

    /// Starts or resumes playback. From [`PlaybackState::Ended`] it restarts at
    /// the beginning. Does nothing once the player [failed](Player::is_failed).
    pub fn play(&self) {
        if self.shared().is_failed() {
            return;
        }
        if self.shared().is_ended() {
            self.seek(Duration::ZERO);
        }
        self.shared().set_playing(true);
        let _ = self.handle.audio_tx.send(AudioCmd::Play);
    }

    /// Pauses playback, keeping the position.
    pub fn pause(&self) {
        self.shared().set_playing(false);
    }

    /// Plays when paused or ended, pauses when playing.
    pub fn toggle(&self) {
        match self.state() {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused | PlaybackState::Ended => self.play(),
        }
    }

    /// Jumps to `to` (clamped to the duration), keeping the play/pause state.
    /// [`Player::position`] reflects the new position immediately.
    pub fn seek(&self, to: Duration) {
        let shared = self.shared();
        let to = match shared.duration {
            Some(d) => to.min(d),
            None => to,
        };
        if shared.is_failed() {
            return;
        }
        let generation = shared.seek(to);
        let _ = self.handle.audio_tx.send(AudioCmd::Seek { generation, to });
    }

    /// Sets the volume, clamped to `0.0..=1.0` (non-finite values are ignored).
    pub fn set_volume(&self, volume: f32) {
        if !volume.is_finite() {
            return;
        }
        self.shared().audio.set_volume(volume.clamp(0.0, 1.0));
    }

    /// Current volume in `0.0..=1.0`.
    pub fn volume(&self) -> f32 {
        self.shared().audio.volume()
    }

    /// Current playback position.
    pub fn position(&self) -> Duration {
        self.shared().position()
    }

    /// Total duration, when the container declares it.
    pub fn duration(&self) -> Option<Duration> {
        self.shared().duration
    }

    /// Whether decoding failed for good (the decoder crashed on a corrupt file).
    /// The state then stays [`PlaybackState::Ended`] and [`Player::play`] does
    /// nothing; the UI can offer to open the file in the system player instead.
    pub fn is_failed(&self) -> bool {
        self.shared().is_failed()
    }

    /// Current playback state.
    pub fn state(&self) -> PlaybackState {
        let shared = self.shared();
        if shared.is_ended() {
            PlaybackState::Ended
        } else if shared.is_playing() {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_is_send_sync_clone() {
        fn assert_traits<T: Send + Sync + Clone + 'static>() {}
        assert_traits::<Player>();
    }

    #[test]
    fn unsupported_types_are_rejected() {
        for (mime, name) in [
            ("video/webm", "a.webm"),
            ("video/mp4", "a.mp4"),
            ("audio/aac", "a.aac"),
            ("audio/mp4", "a.m4a"),
            ("audio/opus", "a.opus"),
        ] {
            let err = Player::open(Arc::from(&b"nope"[..]), mime, Some(name))
                .err()
                .map(|e| matches!(e, MediaError::Unsupported(_)));
            assert_eq!(err, Some(true), "{mime}");
        }
    }

    #[test]
    fn garbage_audio_is_an_error_not_a_panic() {
        let res = Player::open(Arc::from(&[0u8; 64][..]), "audio/mpeg", Some("a.mp3"));
        assert!(res.is_err());
    }
}
