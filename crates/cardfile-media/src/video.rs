//! Video: MP4 demux with `re_mp4`, H.264 decode with OpenH264, BGRA conversion.
//!
//! The worker thread keeps a small queue of decoded frames ahead of the clock
//! (bounded by count *and* bytes) and publishes the frame due at the current
//! position. Frames are timed by their composition timestamp; since OpenH264
//! emits pictures in display order, each output picture takes the smallest
//! pending composition timestamp, which handles B-frame reordering.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use openh264::OpenH264API;
use openh264::decoder::{DecodedYUV, Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use parking_lot::Mutex;
use re_mp4::{Mp4, StsdBoxContent, TrackKind};

use crate::shared::Shared;
use crate::{MediaError, MediaKind, VideoFrame};

/// Upper bound of frames decoded ahead.
const MAX_QUEUE_FRAMES: usize = 8;
/// Lower bound of frames decoded ahead (wins over the byte budget).
const MIN_QUEUE_FRAMES: usize = 2;
/// Byte budget for the frames decoded ahead.
const QUEUE_BYTES: usize = 64 * 1024 * 1024;
/// Poll interval while paused (commands still wake the thread at once).
const PAUSED_WAIT: Duration = Duration::from_millis(30);
/// Longest sleep while playing.
const PLAYING_WAIT: Duration = Duration::from_millis(15);

/// An H.264 track ready to decode.
pub(crate) struct VideoTrack {
    samples: Vec<re_mp4::Sample>,
    timescale: u64,
    length_size: usize,
    /// SPS + PPS in Annex B form, prepended to every sync sample.
    header: Vec<u8>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) duration: Option<Duration>,
}

/// Parses `data` as MP4 and picks its H.264 track.
///
/// `Ok(None)` means the file has no video track at all (e.g. audio-only MP4);
/// a video track in another codec is [`MediaError::Unsupported`].
pub(crate) fn open_track(data: &[u8]) -> Result<Option<VideoTrack>, MediaError> {
    // `re_mp4` parses untrusted input; contain any panic to this call.
    let mp4 = std::panic::catch_unwind(AssertUnwindSafe(|| Mp4::read_bytes(data)))
        .map_err(|_| MediaError::Malformed("MP4 parser panicked".to_owned()))?
        .map_err(|err| MediaError::Malformed(format!("MP4: {err}")))?;

    let mut other_codec = None;
    for track in mp4.tracks().values() {
        if track.kind != Some(TrackKind::Video) {
            continue;
        }
        let stsd = &track.trak(&mp4).mdia.minf.stbl.stsd;
        let StsdBoxContent::Avc1(avc1) = &stsd.contents else {
            other_codec = Some(
                track
                    .codec_string(&mp4)
                    .unwrap_or_else(|| "unknown".to_owned()),
            );
            continue;
        };
        if track.samples.is_empty() || track.timescale == 0 {
            continue;
        }
        let avcc = &avc1.avcc.contents;
        let mut header = Vec::new();
        for nal in avcc
            .sequence_parameter_sets
            .iter()
            .chain(avcc.picture_parameter_sets.iter())
        {
            header.extend_from_slice(&[0, 0, 0, 1]);
            header.extend_from_slice(&nal.bytes);
        }
        let end_ticks = track
            .samples
            .iter()
            .map(|s| s.composition_timestamp.max(0) as u64 + s.duration)
            .max()
            .unwrap_or(track.duration);
        let width = if track.width > 0 {
            track.width
        } else {
            avc1.width
        };
        let height = if track.height > 0 {
            track.height
        } else {
            avc1.height
        };
        return Ok(Some(VideoTrack {
            samples: track.samples.clone(),
            timescale: track.timescale,
            length_size: usize::from(avcc.length_size_minus_one & 0b11) + 1,
            header,
            width: u32::from(width),
            height: u32::from(height),
            duration: Some(ticks_to_duration(end_ticks as i64, track.timescale)),
        }));
    }
    match other_codec {
        Some(codec) => Err(MediaError::Unsupported(format!(
            "video codec {codec} (only H.264 is played inline)"
        ))),
        None => Ok(None),
    }
}

fn ticks_to_duration(ticks: i64, timescale: u64) -> Duration {
    if ticks <= 0 || timescale == 0 {
        return Duration::ZERO;
    }
    let nanos = u128::from(ticks.unsigned_abs()) * 1_000_000_000 / u128::from(timescale);
    Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
}

/// Frames published to the handle.
#[derive(Default)]
pub(crate) struct VideoShared {
    pub(crate) current: Mutex<Option<Arc<VideoFrame>>>,
    pub(crate) poster: Mutex<Option<Arc<VideoFrame>>>,
}

/// Commands from the handle to the video thread.
pub(crate) enum VideoCmd {
    /// Show the frame at this position and continue decoding from there.
    Seek(Duration),
    /// Re-check state (play/pause/shutdown).
    Wake,
}

struct QueuedFrame {
    pts: Duration,
    frame: Arc<VideoFrame>,
}

struct DecodeState {
    decoder: Option<Decoder>,
    /// Next sample (decode order) to feed.
    next: usize,
    /// Composition timestamps of samples fed but not yet output.
    pending: BinaryHeap<Reverse<i64>>,
    /// Pictures before this composition timestamp are dropped unconverted.
    target_cts: Option<i64>,
    eof: bool,
    seq: u64,
    annexb: Vec<u8>,
}

fn new_decoder() -> Option<Decoder> {
    // Flushing after every packet breaks B-frame reordering; we flush at EOF.
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    match Decoder::with_api_config(OpenH264API::from_source(), config) {
        Ok(decoder) => Some(decoder),
        Err(err) => {
            tracing::warn!("cannot create H.264 decoder: {err}");
            None
        }
    }
}

/// Converts a decoded picture to BGRA8 (alpha is always 255).
fn to_frame(yuv: &DecodedYUV<'_>, seq: u64) -> Option<VideoFrame> {
    let (w, h) = yuv.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let mut bgra = vec![0u8; w * h * 4];
    // `write_rgba8` panics only if the buffer size differs from `w * h * 4` (it
    // is sized from the same dimensions above) or the picture is not I420,
    // which is the only format OpenH264's decoder produces.
    yuv.write_rgba8(&mut bgra);
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Some(VideoFrame {
        width: u32::try_from(w).ok()?,
        height: u32::try_from(h).ok()?,
        bgra,
        seq,
    })
}

impl DecodeState {
    fn new() -> Self {
        Self {
            decoder: new_decoder(),
            next: 0,
            pending: BinaryHeap::new(),
            target_cts: None,
            eof: false,
            seq: 0,
            annexb: Vec::new(),
        }
    }

    /// Restarts decoding from the sync sample at or before `to`.
    fn seek(&mut self, track: &VideoTrack, to: Duration) {
        let to_ticks = i64::try_from(to.as_nanos() * u128::from(track.timescale) / 1_000_000_000)
            .unwrap_or(i64::MAX);
        // The picture to show: the last one that starts at or before `to`.
        let target = track
            .samples
            .iter()
            .map(|s| s.composition_timestamp)
            .filter(|cts| *cts <= to_ticks)
            .max()
            .unwrap_or(0);
        let start = track
            .samples
            .iter()
            .rposition(|s| s.is_sync && s.composition_timestamp <= target)
            .unwrap_or(0);
        self.decoder = new_decoder();
        self.next = start;
        self.pending.clear();
        self.target_cts = Some(target);
        self.eof = false;
    }

    /// Feeds one sample (or flushes at the end) and queues any finished picture.
    fn step(
        &mut self,
        track: &VideoTrack,
        data: &[u8],
        queue: &mut VecDeque<QueuedFrame>,
        video: &VideoShared,
    ) {
        let Some(decoder) = self.decoder.as_mut() else {
            self.eof = true;
            return;
        };
        let Some(sample) = track.samples.get(self.next).copied() else {
            match decoder.flush_remaining() {
                Ok(pictures) => {
                    for yuv in &pictures {
                        emit(
                            yuv,
                            &mut self.pending,
                            self.target_cts,
                            &mut self.seq,
                            track,
                            queue,
                            video,
                        );
                    }
                }
                Err(err) => tracing::debug!("H.264 flush error: {err}"),
            }
            self.eof = true;
            return;
        };
        self.next += 1;
        let Some(bytes) = data.get(sample.byte_range()) else {
            tracing::debug!("video sample {} out of range", sample.id);
            return;
        };
        self.annexb.clear();
        if sample.is_sync {
            self.annexb.extend_from_slice(&track.header);
        }
        avcc_to_annexb(bytes, track.length_size, &mut self.annexb);
        self.pending.push(Reverse(sample.composition_timestamp));
        match decoder.decode(&self.annexb) {
            Ok(Some(yuv)) => emit(
                &yuv,
                &mut self.pending,
                self.target_cts,
                &mut self.seq,
                track,
                queue,
                video,
            ),
            Ok(None) => {}
            Err(err) => {
                tracing::debug!("H.264 decode error: {err}");
                let cts = sample.composition_timestamp;
                self.pending.retain(|Reverse(p)| *p != cts);
            }
        }
    }
}

fn emit(
    yuv: &DecodedYUV<'_>,
    pending: &mut BinaryHeap<Reverse<i64>>,
    target_cts: Option<i64>,
    seq: &mut u64,
    track: &VideoTrack,
    queue: &mut VecDeque<QueuedFrame>,
    video: &VideoShared,
) {
    let Some(Reverse(cts)) = pending.pop() else {
        return;
    };
    if target_cts.is_some_and(|t| cts < t) {
        return;
    }
    *seq += 1;
    let Some(frame) = to_frame(yuv, *seq) else {
        return;
    };
    let frame = Arc::new(frame);
    {
        let mut poster = video.poster.lock();
        if poster.is_none() {
            *poster = Some(frame.clone());
        }
    }
    {
        let mut current = video.current.lock();
        if current.is_none() {
            *current = Some(frame.clone());
        }
    }
    queue.push_back(QueuedFrame {
        pts: ticks_to_duration(cts, track.timescale),
        frame,
    });
}

/// Converts length-prefixed NAL units (MP4 "AVCC") to Annex B start codes.
fn avcc_to_annexb(sample: &[u8], length_size: usize, out: &mut Vec<u8>) {
    let mut i = 0;
    while i + length_size <= sample.len() {
        let len = sample[i..i + length_size]
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        i += length_size;
        let Some(nal) = sample.get(i..i + len) else {
            break;
        };
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(nal);
        i += len;
    }
}

/// Video thread body.
pub(crate) fn run(
    shared: Arc<Shared>,
    video: Arc<VideoShared>,
    track: VideoTrack,
    data: Arc<[u8]>,
    rx: Receiver<VideoCmd>,
) {
    let frame_bytes = (track.width as usize * track.height as usize * 4).max(1);
    let max_queue = (QUEUE_BYTES / frame_bytes).clamp(MIN_QUEUE_FRAMES, MAX_QUEUE_FRAMES);
    let video_end = track.duration.unwrap_or_default();
    let mut state = DecodeState::new();
    let mut queue: VecDeque<QueuedFrame> = VecDeque::new();
    let mut pending_seek: Option<Duration> = None;

    'main: loop {
        if shared.is_shutdown() {
            break;
        }
        loop {
            match rx.try_recv() {
                Ok(VideoCmd::Seek(to)) => pending_seek = Some(to),
                Ok(VideoCmd::Wake) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'main,
            }
        }
        if let Some(to) = pending_seek.take() {
            state.seek(&track, to);
            queue.clear();
        }

        let pos = shared.position();

        // Present the newest frame that is due.
        let mut newest = None;
        while queue.front().is_some_and(|f| f.pts <= pos) {
            newest = queue.pop_front();
        }
        if let Some(f) = newest {
            *video.current.lock() = Some(f.frame);
            state.target_cts = None;
        }

        if state.eof && queue.is_empty() && shared.kind == MediaKind::Video {
            let audio_done = shared.audio.as_ref().is_none_or(|a| a.is_drained());
            if audio_done && pos >= video_end && !shared.is_ended() && shared.is_playing() {
                shared.mark_ended();
            }
        }

        if queue.len() < max_queue && !state.eof {
            state.step(&track, &data, &mut queue, &video);
            continue;
        }

        let wait = if shared.is_playing() {
            queue
                .front()
                .map_or(PLAYING_WAIT, |f| f.pts.saturating_sub(pos))
                .clamp(Duration::from_millis(1), PLAYING_WAIT)
        } else {
            PAUSED_WAIT
        };
        match rx.recv_timeout(wait) {
            Ok(VideoCmd::Seek(to)) => pending_seek = Some(to),
            Ok(VideoCmd::Wake) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avcc_conversion() {
        let sample = [0, 0, 0, 2, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC];
        let mut out = Vec::new();
        avcc_to_annexb(&sample, 4, &mut out);
        assert_eq!(out, vec![0, 0, 0, 1, 0xAA, 0xBB, 0, 0, 0, 1, 0xCC]);
    }

    #[test]
    fn avcc_truncated_is_ignored() {
        let sample = [0, 0, 0, 9, 0xAA];
        let mut out = Vec::new();
        avcc_to_annexb(&sample, 4, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn ticks() {
        assert_eq!(ticks_to_duration(90_000, 90_000), Duration::from_secs(1));
        assert_eq!(ticks_to_duration(-5, 90_000), Duration::ZERO);
        assert_eq!(ticks_to_duration(5, 0), Duration::ZERO);
    }
}
