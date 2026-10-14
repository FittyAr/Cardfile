//! Inline audio and video playback for Cardfile attachments.
//!
//! This crate is a pure engine: it knows nothing about GPUI or the rest of
//! Cardfile. The UI hands it the attachment bytes and polls a [`Player`] every
//! frame for its position, state and (for video) the latest decoded frame.
//!
//! # Supported media
//!
//! | Kind  | Formats | Implementation |
//! |-------|---------|----------------|
//! | Audio | MP3, WAV, OGG/Vorbis, FLAC, AAC (ADTS) and M4A (AAC in MP4) | `symphonia` decode, `rodio`/`cpal` output |
//! | Video | MP4 with H.264 video, plus its AAC (or MP3) track if present | `re_mp4` demux, OpenH264 decode |
//!
//! Anything else is reported as unsupported — [`media_kind`] returns `None`, or
//! [`Player::open`] fails with [`MediaError::Unsupported`] — so the UI can fall
//! back to opening the file in the system player. No GPL/LGPL code (ffmpeg,
//! GStreamer, libvlc) is used.
//!
//! # Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use cardfile_media::{media_kind, Player};
//!
//! # fn main() -> Result<(), cardfile_media::MediaError> {
//! let bytes: Arc<[u8]> = std::fs::read("clip.mp4").unwrap_or_default().into();
//! if media_kind("video/mp4", Some("clip.mp4")).is_some() {
//!     let player = Player::open(bytes, "video/mp4", Some("clip.mp4"))?;
//!     player.play();
//!     // Every UI frame:
//!     let _pos = player.position();
//!     if let Some(frame) = player.current_frame() {
//!         // Upload `frame.bgra` only when `frame.seq` changed.
//!         let _ = (frame.width, frame.height, frame.seq);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Threads, clocks and memory
//!
//! * [`Player::open`] only parses headers. Decoding happens on one audio thread
//!   and one video thread per player; every method of [`Player`] is cheap
//!   (atomics or a short mutex) and safe to call from the UI thread each frame.
//! * The audio output device is opened lazily on the first [`Player::play`].
//!   The master clock is the number of audio frames handed to the device, so
//!   video follows what is heard. Video without audio (or after the audio track
//!   ended) follows a wall clock.
//! * **No output device** (CI, headless Linux, no sound card): `open` and
//!   `play` still succeed. Audio is decoded and consumed silently at real-time
//!   speed, so position, seeking and the `Ended` state behave exactly as with a
//!   device. The same behaviour can be forced with [`AudioOutput::Null`].
//! * Memory is bounded: about half a second of decoded audio and at most
//!   8 decoded video frames (and no more than 64 MiB of them, but always at
//!   least 2) are kept ahead of the clock. The attachment bytes are shared via
//!   the `Arc<[u8]>` passed to `open`, never copied.
//!
//! # Limitations
//!
//! * MP4 edit lists are ignored; the first composition timestamp is time zero.
//! * Video seeking decodes from the previous key frame, so a seek can take as
//!   long as decoding one GOP.
//! * If the output device disappears while playing, the clock stops until the
//!   player is reopened.

mod audio;
mod kind;
mod shared;
mod video;

use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

pub use kind::{MediaKind, media_kind};

use audio::{AudioCmd, AudioShared};
use shared::Shared;
use video::{VideoCmd, VideoShared};

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

/// A decoded video frame.
#[derive(Clone, PartialEq, Eq)]
pub struct VideoFrame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels as BGRA8, row-major, `width * 4` bytes per row, no padding.
    /// Alpha is always 255, so premultiplied and straight alpha are identical.
    pub bgra: Vec<u8>,
    /// Unique, increasing id of this frame within its player. A different
    /// `seq` means a different picture, so the UI can skip re-uploading.
    pub seq: u64,
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("seq", &self.seq)
            .finish_non_exhaustive()
    }
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

/// Owns the worker threads; dropped when the last [`Player`] clone goes away.
struct Handle {
    shared: Arc<Shared>,
    audio_tx: Option<Sender<AudioCmd>>,
    video_tx: Option<Sender<VideoCmd>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared
            .shutdown
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(audio) = &self.shared.audio {
            audio.set_playing(false);
            audio.close();
        }
        // Wake the threads so they notice the flag right away. They are not
        // joined, so dropping never blocks the UI thread; each exits within one
        // decode step and releases the audio device.
        if let Some(tx) = &self.audio_tx {
            let _ = tx.send(AudioCmd::Wake);
        }
        if let Some(tx) = &self.video_tx {
            let _ = tx.send(VideoCmd::Wake);
        }
    }
}

/// A media player for one attachment.
///
/// Cheap to clone (an `Arc` inside), `Send + Sync`. All decoding runs on the
/// player's own threads, which stop (and the audio device is released) when the
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
    /// Only container headers are parsed here; decoding starts on background
    /// threads. The kind is chosen with [`media_kind`]; an MP4 without a video
    /// track is played as audio (check [`Player::kind`]).
    ///
    /// # Errors
    ///
    /// * [`MediaError::Unsupported`] if the media is not inline-playable (unknown
    ///   type, non-H.264 video, unsupported audio codec).
    /// * [`MediaError::Malformed`] if the container cannot be parsed.
    /// * [`MediaError::Thread`] if a worker thread cannot be spawned.
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

        let video_track = match kind {
            MediaKind::Video => video::open_track(&data)?,
            MediaKind::Audio => None,
        };
        let audio_track = match audio::open_track(data.clone(), mime, ext.as_deref()) {
            Ok(track) => track,
            // A video still plays without its sound.
            Err(err) if video_track.is_some() => {
                tracing::warn!("ignoring the audio track of a video: {err}");
                None
            }
            Err(err) => return Err(err),
        };
        if video_track.is_none() && audio_track.is_none() {
            return Err(MediaError::Unsupported(
                "no playable audio or video track".to_owned(),
            ));
        }
        let kind = if video_track.is_some() {
            MediaKind::Video
        } else {
            MediaKind::Audio
        };

        let duration = match (
            video_track.as_ref().and_then(|v| v.duration),
            audio_track.as_ref().and_then(|a| a.duration),
        ) {
            (Some(v), Some(a)) => Some(v.max(a)),
            (v, a) => v.or(a),
        };
        let video_size = video_track.as_ref().map(|v| (v.width, v.height));
        let audio_shared = audio_track
            .as_ref()
            .map(|t| Arc::new(AudioShared::new(t.rate, t.channels)));
        let video_shared = video_track
            .as_ref()
            .map(|_| Arc::new(VideoShared::default()));
        let shared = Arc::new(Shared::new(
            kind,
            duration,
            video_size,
            audio_shared.clone(),
            video_shared.clone(),
        ));

        // Build the handle first so that a failed spawn below still shuts down
        // whatever was already started.
        let mut handle = Handle {
            shared: shared.clone(),
            audio_tx: None,
            video_tx: None,
        };
        if let (Some(track), Some(audio)) = (audio_track, audio_shared) {
            let (tx, rx) = mpsc::channel();
            let shared = shared.clone();
            let output = options.audio_output;
            std::thread::Builder::new()
                .name("cardfile-media-audio".to_owned())
                .spawn(move || audio::run(shared, audio, track, rx, output))?;
            handle.audio_tx = Some(tx);
        }
        if let (Some(track), Some(video)) = (video_track, video_shared) {
            let (tx, rx) = mpsc::channel();
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("cardfile-media-video".to_owned())
                .spawn(move || video::run(shared, video, track, data, rx))?;
            handle.video_tx = Some(tx);
        }
        Ok(Self {
            handle: Arc::new(handle),
        })
    }

    fn shared(&self) -> &Shared {
        &self.handle.shared
    }

    /// What was actually found in the file.
    pub fn kind(&self) -> MediaKind {
        self.shared().kind
    }

    /// Starts or resumes playback. From [`PlaybackState::Ended`] it restarts at
    /// the beginning.
    pub fn play(&self) {
        if self.shared().is_ended() {
            self.seek(Duration::ZERO);
        }
        self.shared().set_playing(true);
        if let Some(tx) = &self.handle.audio_tx {
            let _ = tx.send(AudioCmd::Play);
        }
        self.wake_video();
    }

    /// Pauses playback, keeping the position.
    pub fn pause(&self) {
        self.shared().set_playing(false);
        self.wake_video();
    }

    /// Plays when paused or ended, pauses when playing.
    pub fn toggle(&self) {
        match self.state() {
            PlaybackState::Playing => self.pause(),
            PlaybackState::Paused | PlaybackState::Ended => self.play(),
        }
    }

    /// Jumps to `to` (clamped to the duration), keeping the play/pause state.
    /// [`Player::position`] reflects the new position immediately; the frame at
    /// that position appears once decoded.
    pub fn seek(&self, to: Duration) {
        let shared = self.shared();
        let to = match shared.duration {
            Some(d) => to.min(d),
            None => to,
        };
        shared.seek_clock(to);
        if let (Some(audio), Some(tx)) = (&shared.audio, &self.handle.audio_tx) {
            let generation = audio.begin_seek(to);
            let _ = tx.send(AudioCmd::Seek { generation, to });
        }
        if let Some(tx) = &self.handle.video_tx {
            let _ = tx.send(VideoCmd::Seek(to));
        }
    }

    /// Sets the volume, clamped to `0.0..=1.0` (non-finite values are ignored).
    pub fn set_volume(&self, volume: f32) {
        if !volume.is_finite() {
            return;
        }
        if let Some(audio) = &self.shared().audio {
            audio.set_volume(volume.clamp(0.0, 1.0));
        }
    }

    /// Current volume in `0.0..=1.0` (1.0 for media without audio).
    pub fn volume(&self) -> f32 {
        self.shared().audio.as_ref().map_or(1.0, |a| a.volume())
    }

    /// Current playback position.
    pub fn position(&self) -> Duration {
        self.shared().position()
    }

    /// Total duration, when the container declares it.
    pub fn duration(&self) -> Option<Duration> {
        self.shared().duration
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

    /// Video dimensions in pixels (video only).
    pub fn video_size(&self) -> Option<(u32, u32)> {
        self.shared().video_size
    }

    /// Latest decoded frame for the current position (video only). Its `seq`
    /// changes when a new frame is ready, so the UI can avoid re-uploading.
    /// Before playback starts this is the first frame, once decoded.
    pub fn current_frame(&self) -> Option<Arc<VideoFrame>> {
        self.shared().video.as_ref()?.current.lock().clone()
    }

    /// The first frame of the video, for a thumbnail before playing.
    ///
    /// Decoding it is the video thread's first job after [`Player::open`], so it
    /// is normally available within milliseconds; this never blocks and returns
    /// `None` until then (and always for audio).
    pub fn poster(&self) -> Option<Arc<VideoFrame>> {
        self.shared().video.as_ref()?.poster.lock().clone()
    }

    fn wake_video(&self) {
        if let Some(tx) = &self.handle.video_tx {
            let _ = tx.send(VideoCmd::Wake);
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
    fn unsupported_type_is_rejected() {
        let err = Player::open(Arc::from(&b"nope"[..]), "video/webm", Some("a.webm"))
            .err()
            .map(|e| matches!(e, MediaError::Unsupported(_)));
        assert_eq!(err, Some(true));
    }

    #[test]
    fn garbage_audio_is_an_error_not_a_panic() {
        let res = Player::open(Arc::from(&[0u8; 64][..]), "audio/mpeg", Some("a.mp3"));
        assert!(res.is_err());
    }

    #[test]
    fn garbage_video_is_an_error_not_a_panic() {
        let res = Player::open(Arc::from(&[1u8; 64][..]), "video/mp4", Some("a.mp4"));
        assert!(res.is_err());
    }
}
