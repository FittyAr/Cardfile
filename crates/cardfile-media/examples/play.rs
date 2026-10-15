//! Plays an audio file (MP3, WAV, OGG/Vorbis or FLAC) and prints the position.
//!
//! ```text
//! cargo run -p cardfile-media --example play -- path/to/file.mp3
//! ```

use std::sync::Arc;
use std::time::Duration;

use cardfile_media::{PlaybackState, Player, media_kind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: play <audio-file>")?;
    let data: Arc<[u8]> = std::fs::read(&path)?.into();
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str());

    let Some(kind) = media_kind("", name) else {
        return Err(format!("{path}: not playable inline (open it in the system player)").into());
    };
    let player = Player::open(data, "", name)?;
    println!("{kind:?}, duration {:?}", player.duration());

    player.play();
    while player.state() != PlaybackState::Ended {
        println!("{:>8.2}s", player.position().as_secs_f64());
        std::thread::sleep(Duration::from_millis(250));
    }
    println!("ended at {:.2}s", player.position().as_secs_f64());
    Ok(())
}
