//! End-to-end playback tests with fixtures generated in memory.
//!
//! Every player uses `AudioOutput::Null` (except the device fallback test), so
//! the tests behave the same with or without a sound card.

// The fixture builders below are not `#[test]` fns but should panic just the same.
#![allow(clippy::expect_used)]

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cardfile_media::{
    AudioOutput, MediaError, MediaKind, PlaybackState, Player, PlayerOptions, media_kind,
};

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

/// Video, AAC/M4A and Opus are never played inline, even when the bytes would
/// decode: the UI must hand them to the system player.
#[test]
fn video_and_patent_encumbered_types_are_unsupported() {
    for (mime, name) in [
        ("video/mp4", Some("clip.mp4")),
        ("", Some("clip.mp4")),
        ("video/webm", Some("clip.webm")),
        ("audio/mp4", Some("song.m4a")),
        ("", Some("song.m4a")),
        ("audio/aac", Some("song.aac")),
        ("audio/aac", Some("song.wav")),
        ("audio/opus", Some("song.opus")),
    ] {
        assert_eq!(media_kind(mime, name), None, "{mime} {name:?}");
        let res = Player::open_with(wav(0.1, 8000), mime, name, null());
        assert!(
            matches!(res, Err(MediaError::Unsupported(_))),
            "{mime} {name:?} was accepted"
        );
    }
}

/// Plays `player` (if it opened) until it ends; it must end, never hang or crash.
fn plays_out(res: Result<Player, MediaError>, what: &str) {
    let Ok(player) = res else { return };
    player.play();
    assert!(
        wait_for(|| player.state() == PlaybackState::Ended),
        "{what}: never ended"
    );
}

/// Truncated and corrupted files end playback (or fail to open); they never
/// panic or hang the decoder thread.
#[test]
fn truncated_and_garbage_audio_ends_cleanly() {
    let good = wav(0.3, 8000);
    // Header plus half the samples.
    let truncated: Arc<[u8]> = good[..good.len() / 2].into();
    plays_out(
        Player::open_with(truncated, "audio/wav", Some("cut.wav"), null()),
        "truncated wav",
    );
    // Valid header, garbage payload (xorshift noise).
    let mut garbage = good.to_vec();
    let mut x = 0x2545_f491_u32;
    for b in &mut garbage[44..] {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *b = x as u8;
    }
    plays_out(
        Player::open_with(
            garbage.clone().into(),
            "audio/wav",
            Some("noise.wav"),
            null(),
        ),
        "garbage wav",
    );
    // Pure noise claimed as each supported format.
    let noise: Arc<[u8]> = garbage[44..].into();
    for (mime, name) in [
        ("audio/mpeg", "a.mp3"),
        ("audio/ogg", "a.ogg"),
        ("audio/flac", "a.flac"),
        ("audio/wav", "a.wav"),
    ] {
        plays_out(
            Player::open_with(noise.clone(), mime, Some(name), null()),
            name,
        );
    }
    // Every header byte flipped in turn: still no panic.
    for i in 0..44 {
        let mut bytes = good.to_vec();
        bytes[i] ^= 0xff;
        plays_out(
            Player::open_with(bytes.into(), "audio/wav", Some("flip.wav"), null()),
            "flipped header",
        );
    }
}

/// `play` from `Ended` must never come back as `Ended` because the worker saw the
/// previous run's drained output in the middle of the restart.
#[test]
fn restart_after_end_is_not_reported_ended() {
    let player = Player::open_with(wav(0.05, 8000), "audio/wav", None, null()).expect("open");
    for _ in 0..20 {
        player.play();
        assert!(wait_for(|| player.state() == PlaybackState::Ended));
        player.play();
        let state = player.state();
        assert!(
            state == PlaybackState::Playing
                || (state == PlaybackState::Ended && player.position() > Duration::ZERO),
            "restart reported {state:?} at {:?}",
            player.position()
        );
        assert!(!player.is_failed());
    }
}
