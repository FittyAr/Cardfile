//! Audio: symphonia demux/decode on a worker thread, rodio/cpal output.
//!
//! The decoder thread fills a small ring of interleaved `f32` chunks (about half
//! a second). A custom rodio [`Source`] drains it on the audio callback and
//! counts the frames it hands out; that counter *is* the playback clock, so the
//! position always matches what is being heard. Without an output device the
//! same source is drained by the worker thread at real-time speed ("null
//! output"), so the clock and the ended/seek logic behave identically.

use std::collections::VecDeque;
use std::io::Cursor;
use std::num::NonZero;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, SampleRate, Source};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase};

use crate::shared::Shared;
use crate::{AudioOutput, MediaError, MediaKind};

/// How much decoded audio is buffered ahead of the output, in seconds.
const RING_SECONDS: f64 = 0.5;
/// How long the worker sleeps when there is nothing to decode.
const IDLE_WAIT: Duration = Duration::from_millis(10);
/// Packets decoded per loop iteration before commands are checked again.
const PACKETS_PER_TURN: usize = 8;

/// An opened, ready-to-decode audio track.
pub(crate) struct AudioTrack {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    time_base: Option<TimeBase>,
    pub(crate) rate: u32,
    pub(crate) channels: u16,
    pub(crate) duration: Option<Duration>,
}

/// Probes `data` and opens its first decodable audio track.
///
/// Returns `Ok(None)` when the container has no decodable audio track (used for
/// MP4 videos without sound).
pub(crate) fn open_track(
    data: Arc<[u8]>,
    mime: &str,
    ext: Option<&str>,
) -> Result<Option<AudioTrack>, MediaError> {
    let mut hint = Hint::new();
    if let Some(ext) = ext {
        hint.with_extension(ext);
    }
    if !mime.is_empty() {
        hint.mime_type(mime);
    }
    let mss = MediaSourceStream::new(
        Box::new(Cursor::new(data)),
        MediaSourceStreamOptions::default(),
    );
    let reader = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(sym_err)?;

    let codecs = symphonia::default::get_codecs();
    let opts = AudioDecoderOptions::default().gapless(true);
    let mut chosen = None;
    for track in reader.tracks() {
        if track.track_type() != Some(TrackType::Audio) {
            continue;
        }
        let Some(params) = track.codec_params.as_ref().and_then(|p| p.audio()) else {
            continue;
        };
        if let Ok(decoder) = codecs.make_audio_decoder(params, &opts) {
            chosen = Some((track.clone(), decoder));
            break;
        }
    }
    let Some((track, decoder)) = chosen else {
        return Ok(None);
    };

    let params = decoder.codec_params();
    let rate = params
        .sample_rate
        .filter(|r| *r > 0)
        .ok_or_else(|| MediaError::Malformed("audio track has no sample rate".to_owned()))?;
    let channels = params
        .channels
        .as_ref()
        .map(|c| c.count())
        .filter(|c| *c > 0)
        .and_then(|c| u16::try_from(c).ok())
        .unwrap_or(2);

    let duration = match (track.time_base, track.duration) {
        (Some(tb), Some(d)) => tb.calc_duration(d).map(time_to_duration),
        _ => None,
    }
    .or_else(|| {
        track
            .num_frames
            .map(|n| Duration::from_secs_f64(n as f64 / f64::from(rate)))
    });

    Ok(Some(AudioTrack {
        reader,
        decoder,
        track_id: track.id,
        time_base: track.time_base,
        rate,
        channels,
        duration,
    }))
}

fn sym_err(err: SymError) -> MediaError {
    match err {
        SymError::Unsupported(what) => MediaError::Unsupported(what.to_owned()),
        other => MediaError::Malformed(other.to_string()),
    }
}

fn time_to_duration(t: Time) -> Duration {
    let ns = t.as_nanos();
    if ns <= 0 {
        Duration::ZERO
    } else {
        Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX))
    }
}

/// Decoded samples waiting for the output.
#[derive(Default)]
struct Ring {
    chunks: VecDeque<Vec<f32>>,
    samples: usize,
    /// Generation the queued chunks belong to (bumped on every seek).
    generation: u64,
    /// The decoder reached the end of the track for `generation`.
    eof: bool,
}

/// Audio state shared between the handle, the decoder thread and the output.
pub(crate) struct AudioShared {
    rate: u32,
    channels: u16,
    capacity: usize,
    ring: Mutex<Ring>,
    generation: AtomicU64,
    /// Position (ns) at which the current generation starts.
    base_ns: AtomicU64,
    /// Frames handed to the output since `base_ns`.
    played: AtomicU64,
    playing: AtomicBool,
    /// `f32` bits.
    volume: AtomicU32,
    /// The output consumed every sample up to the end of the track.
    drained: AtomicBool,
    /// The output source should terminate.
    closed: AtomicBool,
}

impl AudioShared {
    pub(crate) fn new(rate: u32, channels: u16) -> Self {
        let capacity = (f64::from(rate) * f64::from(channels) * RING_SECONDS) as usize;
        Self {
            rate,
            channels,
            capacity: capacity.max(4096),
            ring: Mutex::new(Ring::default()),
            generation: AtomicU64::new(0),
            base_ns: AtomicU64::new(0),
            played: AtomicU64::new(0),
            playing: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            drained: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        }
    }

    pub(crate) fn position(&self) -> Duration {
        let base = Duration::from_nanos(self.base_ns.load(Ordering::Acquire));
        let frames = self.played.load(Ordering::Acquire);
        base + Duration::from_secs_f64(frames as f64 / f64::from(self.rate))
    }

    pub(crate) fn is_drained(&self) -> bool {
        self.drained.load(Ordering::Acquire)
    }

    pub(crate) fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
    }

    pub(crate) fn set_volume(&self, volume: f32) {
        self.volume.store(volume.to_bits(), Ordering::Release);
    }

    pub(crate) fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Acquire))
    }

    /// Drops everything queued and restarts the clock at `to`. Returns the new
    /// generation, which the decoder thread must tag its chunks with.
    pub(crate) fn begin_seek(&self, to: Duration) -> u64 {
        let mut ring = self.ring.lock();
        let generation = ring.generation.wrapping_add(1);
        ring.generation = generation;
        ring.chunks.clear();
        ring.samples = 0;
        ring.eof = false;
        self.base_ns.store(
            u64::try_from(to.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Release,
        );
        self.played.store(0, Ordering::Release);
        self.drained.store(false, Ordering::Release);
        self.generation.store(generation, Ordering::Release);
        generation
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    fn has_room(&self) -> bool {
        self.ring.lock().samples < self.capacity
    }

    fn push(&self, generation: u64, chunk: Vec<f32>) {
        if chunk.is_empty() {
            return;
        }
        let mut ring = self.ring.lock();
        if ring.generation == generation {
            ring.samples += chunk.len();
            ring.chunks.push_back(chunk);
        }
    }

    fn set_eof(&self, generation: u64) {
        let mut ring = self.ring.lock();
        if ring.generation == generation {
            ring.eof = true;
        }
    }
}

/// The rodio source that plays the ring. Every frame is either entirely real
/// samples or entirely silence, so channels never get misaligned.
pub(crate) struct StreamSource {
    shared: Arc<AudioShared>,
    local: Vec<f32>,
    pos: usize,
    generation: u64,
    left_in_frame: u16,
    silent: bool,
    volume: f32,
}

impl StreamSource {
    pub(crate) fn new(shared: Arc<AudioShared>) -> Self {
        let generation = shared.generation.load(Ordering::Acquire);
        Self {
            shared,
            local: Vec::new(),
            pos: 0,
            generation,
            left_in_frame: 0,
            silent: true,
            volume: 1.0,
        }
    }

    /// Called at a frame boundary: decides whether the next frame is real.
    fn start_frame(&mut self) {
        let generation = self.shared.generation.load(Ordering::Acquire);
        if generation != self.generation {
            self.generation = generation;
            self.local.clear();
            self.pos = 0;
        }
        self.volume = self.shared.volume();
        self.silent = true;
        if !self.shared.playing.load(Ordering::Acquire) {
            return;
        }
        if self.pos >= self.local.len() {
            let mut ring = self.shared.ring.lock();
            if ring.generation != self.generation {
                return;
            }
            match ring.chunks.pop_front() {
                Some(chunk) => {
                    ring.samples -= chunk.len();
                    self.local = chunk;
                    self.pos = 0;
                }
                None => {
                    if ring.eof {
                        self.shared.drained.store(true, Ordering::Release);
                    }
                    return;
                }
            }
        }
        if self.pos < self.local.len() {
            self.silent = false;
            self.shared.played.fetch_add(1, Ordering::AcqRel);
        }
    }
}

impl Iterator for StreamSource {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left_in_frame == 0 {
            if self.shared.closed.load(Ordering::Acquire) {
                return None;
            }
            self.start_frame();
            self.left_in_frame = self.shared.channels;
        }
        self.left_in_frame -= 1;
        if self.silent {
            return Some(0.0);
        }
        let sample = self.local.get(self.pos).copied().unwrap_or(0.0);
        self.pos += 1;
        Some(sample * self.volume)
    }
}

impl Source for StreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(self.shared.channels).unwrap_or(NonZero::<u16>::MIN)
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.shared.rate).unwrap_or(NonZero::<u32>::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Commands from the handle to the audio thread.
pub(crate) enum AudioCmd {
    /// Playback started: open the output if it is not open yet.
    Play,
    /// Seek the decoder; chunks must be tagged with `generation`.
    Seek { generation: u64, to: Duration },
    /// Re-check the shutdown flag.
    Wake,
}

enum Output {
    /// Not opened yet (opened lazily on the first `play`).
    Pending(StreamSource),
    /// Playing through a real device; the source lives in the device mixer.
    Device {
        /// Kept alive for as long as playback should be audible.
        _sink: MixerDeviceSink,
    },
    /// No device: the source is drained at real-time speed by this thread.
    Null {
        source: StreamSource,
        last: Instant,
        carry: f64,
    },
}

fn open_device() -> Option<MixerDeviceSink> {
    // cpal backends talk to the OS audio stack; never let a misbehaving backend
    // take the app down with it.
    let opened = std::panic::catch_unwind(AssertUnwindSafe(|| {
        DeviceSinkBuilder::from_default_device()
            .map(|b| b.with_error_callback(|err| tracing::warn!("audio output error: {err}")))
            .and_then(|b| b.open_sink_or_fallback())
            .or_else(|_| DeviceSinkBuilder::open_default_sink())
    }));
    match opened {
        Ok(Ok(mut sink)) => {
            sink.log_on_drop(false);
            Some(sink)
        }
        Ok(Err(err)) => {
            tracing::info!("no audio output device, playing silently: {err}");
            None
        }
        Err(_) => {
            tracing::warn!("audio backend panicked while opening the device, playing silently");
            None
        }
    }
}

/// Decoder thread body.
pub(crate) fn run(
    shared: Arc<Shared>,
    audio: Arc<AudioShared>,
    mut track: AudioTrack,
    rx: Receiver<AudioCmd>,
    output_mode: AudioOutput,
) {
    let mut output = Output::Pending(StreamSource::new(audio.clone()));
    let mut generation = 0u64;
    let mut eof = false;
    let mut skip_until: Option<f64> = None;
    let mut scratch: Vec<f32> = Vec::new();

    'main: loop {
        if shared.is_shutdown() {
            break;
        }

        // Commands (coalesce seeks: only the last one matters).
        let mut want_output = false;
        let mut seek = None;
        loop {
            match rx.try_recv() {
                Ok(AudioCmd::Play) => want_output = true,
                Ok(AudioCmd::Seek { generation, to }) => seek = Some((generation, to)),
                Ok(AudioCmd::Wake) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'main,
            }
        }
        if let Some((g, to)) = seek {
            generation = g;
            eof = !track.seek(to);
            skip_until = Some(to.as_secs_f64());
            if eof {
                audio.set_eof(generation);
            }
        }
        if want_output || shared.is_playing() {
            output = match output {
                Output::Pending(source) => match output_mode {
                    AudioOutput::Default => match open_device() {
                        Some(sink) => {
                            sink.mixer().add(source);
                            Output::Device { _sink: sink }
                        }
                        None => Output::Null {
                            source,
                            last: Instant::now(),
                            carry: 0.0,
                        },
                    },
                    AudioOutput::Null => Output::Null {
                        source,
                        last: Instant::now(),
                        carry: 0.0,
                    },
                },
                other => other,
            };
        }

        // Decode ahead.
        let mut decoded = 0;
        while decoded < PACKETS_PER_TURN && !eof && audio.has_room() {
            match track.decode_next(&mut scratch, &mut skip_until, audio.channels) {
                Step::Chunk(chunk) => audio.push(generation, chunk),
                Step::Skip => {}
                Step::Eof => {
                    eof = true;
                    audio.set_eof(generation);
                }
            }
            decoded += 1;
        }

        // Without a device, consume samples in real time.
        if let Output::Null {
            source,
            last,
            carry,
        } = &mut output
        {
            let now = Instant::now();
            let frames = now.duration_since(*last).as_secs_f64() * f64::from(audio.rate) + *carry;
            *last = now;
            let whole = frames.floor();
            *carry = frames - whole;
            let samples = whole as u64 * u64::from(audio.channels);
            for _ in 0..samples {
                if source.next().is_none() {
                    break;
                }
            }
        }

        if shared.kind == MediaKind::Audio && audio.is_drained() && !shared.is_ended() {
            shared.mark_ended();
        }

        let busy = !eof && audio.has_room();
        if !busy {
            match rx.recv_timeout(IDLE_WAIT) {
                Ok(AudioCmd::Play) => {
                    // Re-handled on the next turn via `shared.is_playing()`.
                }
                Ok(AudioCmd::Seek { generation: g, to }) => {
                    generation = g;
                    eof = !track.seek(to);
                    skip_until = Some(to.as_secs_f64());
                    if eof {
                        audio.set_eof(generation);
                    }
                }
                Ok(AudioCmd::Wake) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    audio.close();
    drop(output);
}

enum Step {
    Chunk(Vec<f32>),
    Skip,
    Eof,
}

impl AudioTrack {
    /// Seeks the demuxer; returns `false` when the track cannot continue.
    fn seek(&mut self, to: Duration) -> bool {
        let Some(time) = Time::try_from_secs_f64(to.as_secs_f64()) else {
            return false;
        };
        let target = SeekTo::Time {
            time,
            track_id: Some(self.track_id),
        };
        let result = self.reader.seek(SeekMode::Accurate, target).or_else(|_| {
            self.reader.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
        });
        self.decoder.reset();
        match result {
            Ok(_) => true,
            Err(err) => {
                tracing::warn!("audio seek failed: {err}");
                false
            }
        }
    }

    fn decode_next(
        &mut self,
        scratch: &mut Vec<f32>,
        skip_until: &mut Option<f64>,
        out_channels: u16,
    ) -> Step {
        let packet = match self.reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => return Step::Eof,
            Err(SymError::ResetRequired) => {
                self.decoder.reset();
                return Step::Skip;
            }
            Err(SymError::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Step::Eof;
            }
            Err(err) => {
                tracing::warn!("audio demux error, stopping: {err}");
                return Step::Eof;
            }
        };
        if packet.track_id != self.track_id {
            return Step::Skip;
        }
        let buf = match self.decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(SymError::DecodeError(err)) => {
                tracing::debug!("skipping undecodable audio packet: {err}");
                return Step::Skip;
            }
            Err(SymError::ResetRequired) => {
                self.decoder.reset();
                return Step::Skip;
            }
            Err(err) => {
                tracing::warn!("audio decode error, stopping: {err}");
                return Step::Eof;
            }
        };
        let frames = buf.frames();
        if frames == 0 {
            return Step::Skip;
        }
        let rate = f64::from(buf.spec().rate().max(1));
        let src_channels = buf.spec().channels().count().max(1);

        // After a seek, drop the frames before the requested position.
        let mut start = 0usize;
        if let Some(target) = *skip_until {
            let pts = self
                .time_base
                .and_then(|tb| tb.calc_time(packet.pts))
                .map(|t| t.as_secs_f64());
            match pts {
                Some(pts) => {
                    let skip = ((target - pts) * rate).round();
                    if skip >= frames as f64 {
                        return Step::Skip;
                    }
                    start = if skip > 0.0 { skip as usize } else { 0 };
                    *skip_until = None;
                }
                None => *skip_until = None,
            }
        }
        buf.slice(start..).copy_to_vec_interleaved::<f32>(scratch);

        let out_channels = usize::from(out_channels);
        if src_channels == out_channels {
            return Step::Chunk(std::mem::take(scratch));
        }
        // Channel layout changed mid-stream: map onto the output layout.
        let n = scratch.len() / src_channels;
        let mut chunk = Vec::with_capacity(n * out_channels);
        for frame in scratch.chunks_exact(src_channels) {
            for c in 0..out_channels {
                chunk.push(frame[c.min(src_channels - 1)]);
            }
        }
        Step::Chunk(chunk)
    }
}
