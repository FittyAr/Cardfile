//! End-to-end playback tests with fixtures generated in memory.
//!
//! Every player uses `AudioOutput::Null` (except the device fallback test), so
//! the tests behave the same with or without a sound card.

// The fixture builders below are not `#[test]` fns but should panic just the same.
#![allow(clippy::expect_used)]

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cardfile_media::{AudioOutput, MediaKind, PlaybackState, Player, PlayerOptions, VideoFrame};

const TIMEOUT: Duration = Duration::from_secs(10);

fn null() -> PlayerOptions {
    PlayerOptions::default().audio_output(AudioOutput::Null)
}

/// Polls `cond` until it holds or [`TIMEOUT`] elapses.
fn wait_for(mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < TIMEOUT {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cond()
}

/// A mono 16-bit WAV with a quiet 440 Hz tone.
fn wav(seconds: f32, rate: u32) -> Arc<[u8]> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut out, spec).expect("wav writer");
    let n = (seconds * rate as f32) as u32;
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let s = (t * 440.0 * std::f32::consts::TAU).sin() * 0.05;
        writer
            .write_sample((s * f32::from(i16::MAX)) as i16)
            .expect("sample");
    }
    writer.finalize().expect("finalize");
    out.into_inner().into()
}

fn approx(a: Duration, b: Duration, tolerance: Duration) -> bool {
    a.abs_diff(b) <= tolerance
}

#[test]
fn wav_opens_with_duration() {
    let player =
        Player::open_with(wav(0.5, 8000), "audio/wav", Some("a.wav"), null()).expect("open wav");
    assert_eq!(player.kind(), MediaKind::Audio);
    assert_eq!(player.state(), PlaybackState::Paused);
    assert_eq!(player.position(), Duration::ZERO);
    let duration = player.duration().expect("duration");
    assert!(approx(
        duration,
        Duration::from_millis(500),
        Duration::from_millis(5)
    ));
    assert_eq!(player.video_size(), None);
    assert!(player.current_frame().is_none());
    assert!(player.poster().is_none());
}

#[test]
fn wav_plays_to_the_end_and_restarts() {
    let player = Player::open_with(wav(0.3, 8000), "", Some("tone.wav"), null()).expect("open");
    player.play();
    assert_eq!(player.state(), PlaybackState::Playing);
    assert!(
        wait_for(|| player.state() == PlaybackState::Ended),
        "never ended"
    );
    let duration = player.duration().expect("duration");
    assert!(
        approx(player.position(), duration, Duration::from_millis(30)),
        "position {:?} at end, duration {duration:?}",
        player.position()
    );

    // `play` from Ended restarts at zero.
    player.play();
    assert_eq!(player.state(), PlaybackState::Playing);
    assert!(player.position() < Duration::from_millis(150));
    assert!(wait_for(|| player.state() == PlaybackState::Ended));
}

#[test]
fn pause_freezes_the_position() {
    let player = Player::open_with(wav(3.0, 8000), "audio/wav", None, null()).expect("open");
    player.play();
    assert!(wait_for(|| player.position() > Duration::from_millis(100)));
    player.pause();
    assert_eq!(player.state(), PlaybackState::Paused);
    let frozen = player.position();
    std::thread::sleep(Duration::from_millis(150));
    // The output may finish the frame it was on, nothing more.
    assert!(approx(player.position(), frozen, Duration::from_millis(5)));
    player.toggle();
    assert_eq!(player.state(), PlaybackState::Playing);
    assert!(wait_for(
        || player.position() > frozen + Duration::from_millis(50)
    ));
}

#[test]
fn seek_moves_the_clock_and_keeps_playing() {
    let player = Player::open_with(wav(2.0, 8000), "audio/x-wav", None, null()).expect("open");
    player.seek(Duration::from_millis(1500));
    assert_eq!(player.position(), Duration::from_millis(1500));
    assert_eq!(player.state(), PlaybackState::Paused);
    player.play();
    assert!(wait_for(|| player.state() == PlaybackState::Ended));
    // Seeking past the end clamps.
    player.seek(Duration::from_secs(60));
    assert!(player.position() <= player.duration().expect("duration"));
    assert_eq!(player.state(), PlaybackState::Paused);
}

#[test]
fn volume_is_clamped() {
    let player = Player::open_with(wav(0.1, 8000), "audio/wav", None, null()).expect("open");
    assert_eq!(player.volume(), 1.0);
    player.set_volume(0.25);
    assert_eq!(player.volume(), 0.25);
    player.set_volume(7.0);
    assert_eq!(player.volume(), 1.0);
    player.set_volume(-1.0);
    assert_eq!(player.volume(), 0.0);
    player.set_volume(f32::NAN);
    assert_eq!(player.volume(), 0.0);
}

/// Opens the real default device: must not panic or fail on machines without
/// one (CI). With a device it plays a short, very quiet tone.
#[test]
fn default_output_never_fails() {
    let player = Player::open(wav(0.2, 8000), "audio/wav", Some("a.wav")).expect("open");
    player.set_volume(0.0);
    player.play();
    assert!(wait_for(|| player.state() == PlaybackState::Ended));
}

#[test]
fn dropping_a_playing_player_is_clean() {
    let player = Player::open_with(wav(5.0, 8000), "audio/wav", None, null()).expect("open");
    let clone = player.clone();
    player.play();
    drop(player);
    assert_eq!(clone.state(), PlaybackState::Playing);
    drop(clone);
}

// --- Video ------------------------------------------------------------------

const W: usize = 64;
const H: usize = 48;
const FPS: u32 = 10;
const FRAMES: usize = 10;

/// RGB colour of frame `i`.
fn color(i: usize) -> [u8; 3] {
    let v = (i * 255 / (FRAMES - 1)) as u8;
    [v, 255 - v, 128]
}

/// Splits an Annex B NAL (as returned by the encoder) from its start code.
fn strip_start_code(nal: &[u8]) -> &[u8] {
    if nal.starts_with(&[0, 0, 0, 1]) {
        &nal[4..]
    } else if nal.starts_with(&[0, 0, 1]) {
        &nal[3..]
    } else {
        nal
    }
}

/// Encodes `FRAMES` solid-colour frames with OpenH264 and muxes them into an
/// MP4 (key frames at 0 and 5).
fn h264_mp4() -> Arc<[u8]> {
    use openh264::OpenH264API;
    use openh264::encoder::{BitRate, Encoder, EncoderConfig, FrameRate, RateControlMode};
    use openh264::formats::{RgbSliceU8, YUVBuffer};

    let config = EncoderConfig::new()
        .bitrate(BitRate::from_bps(2_000_000))
        .max_frame_rate(FrameRate::from_hz(FPS as f32))
        .rate_control_mode(RateControlMode::Off)
        .skip_frames(false);
    let mut encoder =
        Encoder::with_api_config(OpenH264API::from_source(), config).expect("encoder");

    let mut sps = Vec::new();
    let mut pps = Vec::new();
    let mut samples = Vec::new();
    for i in 0..FRAMES {
        if i == 5 {
            encoder.force_intra_frame();
        }
        let rgb: Vec<u8> = std::iter::repeat_n(color(i), W * H).flatten().collect();
        let yuv = YUVBuffer::from_rgb8_source(RgbSliceU8::new(&rgb, (W, H)));
        let stream = encoder.encode(&yuv).expect("encode");
        let mut avcc = Vec::new();
        let mut is_sync = false;
        for l in 0..stream.num_layers() {
            let layer = stream.layer(l).expect("layer");
            for n in 0..layer.nal_count() {
                let nal = strip_start_code(layer.nal_unit(n).expect("nal"));
                match nal[0] & 0x1f {
                    7 => sps = nal.to_vec(),
                    8 => pps = nal.to_vec(),
                    t => {
                        if t == 5 {
                            is_sync = true;
                        }
                        avcc.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                        avcc.extend_from_slice(nal);
                    }
                }
            }
        }
        assert!(!avcc.is_empty(), "frame {i} was skipped by the encoder");
        samples.push((avcc, is_sync));
    }

    let mut out = Cursor::new(Vec::new());
    let config = mp4::Mp4Config {
        major_brand: "isom".parse().expect("brand"),
        minor_version: 512,
        compatible_brands: vec![
            "isom".parse().expect("brand"),
            "avc1".parse().expect("brand"),
            "mp41".parse().expect("brand"),
        ],
        timescale: 1000,
    };
    let mut writer = mp4::Mp4Writer::write_start(&mut out, &config).expect("mp4 start");
    writer
        .add_track(&mp4::TrackConfig {
            track_type: mp4::TrackType::Video,
            timescale: 1000,
            language: "und".to_owned(),
            media_conf: mp4::MediaConfig::AvcConfig(mp4::AvcConfig {
                width: W as u16,
                height: H as u16,
                seq_param_set: sps,
                pic_param_set: pps,
            }),
        })
        .expect("track");
    let frame_ms = 1000 / FPS;
    for (i, (avcc, is_sync)) in samples.into_iter().enumerate() {
        writer
            .write_sample(
                1,
                &mp4::Mp4Sample {
                    start_time: u64::from(frame_ms) * i as u64,
                    duration: frame_ms,
                    rendering_offset: 0,
                    is_sync,
                    bytes: bytes::Bytes::from(avcc),
                },
            )
            .expect("sample");
    }
    writer.write_end().expect("mp4 end");
    out.into_inner().into()
}

/// Average BGRA of the frame, as RGB.
fn average_rgb(frame: &VideoFrame) -> [u8; 3] {
    let mut sum = [0u64; 3];
    for px in frame.bgra.as_chunks::<4>().0 {
        sum[0] += u64::from(px[2]);
        sum[1] += u64::from(px[1]);
        sum[2] += u64::from(px[0]);
    }
    let n = (frame.width * frame.height) as u64;
    sum.map(|s| (s / n) as u8)
}

fn close_to(rgb: [u8; 3], want: [u8; 3]) -> bool {
    rgb.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 24)
}

#[test]
fn mp4_poster_and_metadata() {
    let player =
        Player::open_with(h264_mp4(), "video/mp4", Some("clip.mp4"), null()).expect("open mp4");
    assert_eq!(player.kind(), MediaKind::Video);
    assert_eq!(player.video_size(), Some((W as u32, H as u32)));
    let duration = player.duration().expect("duration");
    assert!(approx(
        duration,
        Duration::from_secs(1),
        Duration::from_millis(1)
    ));

    assert!(wait_for(|| player.poster().is_some()), "no poster");
    let poster = player.poster().expect("poster");
    assert_eq!((poster.width, poster.height), (W as u32, H as u32));
    assert_eq!(poster.bgra.len(), W * H * 4);
    assert!(poster.bgra.as_chunks::<4>().0.iter().all(|px| px[3] == 255));
    let rgb = average_rgb(&poster);
    assert!(
        close_to(rgb, color(0)),
        "poster colour {rgb:?}, want {:?}",
        color(0)
    );
    // Before playing, the current frame is the poster.
    assert_eq!(player.current_frame().map(|f| f.seq), Some(poster.seq));
}

#[test]
fn mp4_plays_in_order_and_ends() {
    let player = Player::open_with(h264_mp4(), "", Some("clip.mp4"), null()).expect("open");
    assert!(wait_for(|| player.poster().is_some()));
    player.play();
    let start = Instant::now();
    let mut seqs = Vec::new();
    while player.state() != PlaybackState::Ended && start.elapsed() < TIMEOUT {
        if let Some(f) = player.current_frame()
            && seqs.last() != Some(&f.seq)
        {
            seqs.push(f.seq);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(player.state(), PlaybackState::Ended);
    // Wall clock: about one second of playback.
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(850),
        "ended too early: {elapsed:?}"
    );
    assert!(
        seqs.windows(2).all(|w| w[0] < w[1]),
        "seq not increasing: {seqs:?}"
    );
    assert!(seqs.len() >= FRAMES / 2, "only saw {} frames", seqs.len());
    let last = player.current_frame().expect("last frame");
    let rgb = average_rgb(&last);
    assert!(close_to(rgb, color(FRAMES - 1)), "last colour {rgb:?}");
}

#[test]
fn mp4_seek_while_paused_shows_the_target_frame() {
    let player = Player::open_with(h264_mp4(), "video/mp4", None, null()).expect("open");
    assert!(wait_for(|| player.poster().is_some()));
    // 0.75 s is frame 7: decoded from the key frame at 5.
    player.seek(Duration::from_millis(750));
    assert_eq!(player.position(), Duration::from_millis(750));
    assert!(
        wait_for(|| player
            .current_frame()
            .is_some_and(|f| close_to(average_rgb(&f), color(7)))),
        "frame 7 never shown; got {:?}",
        player.current_frame().map(|f| average_rgb(&f))
    );
    assert_eq!(player.state(), PlaybackState::Paused);
    // Back to a frame before the second key frame.
    player.seek(Duration::from_millis(200));
    assert!(wait_for(|| player
        .current_frame()
        .is_some_and(|f| close_to(average_rgb(&f), color(2)))));
}

#[test]
fn mp4_without_h264_is_unsupported_or_malformed() {
    // A WAV labelled as MP4 must fail cleanly so the UI can fall back.
    let res = Player::open_with(wav(0.1, 8000), "video/mp4", Some("x.mp4"), null());
    assert!(res.is_err());
}
