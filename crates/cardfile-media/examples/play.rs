//! Plays an audio or video file and prints the position.
//!
//! ```text
//! cargo run -p cardfile-media --example play -- path/to/file.mp3
//! ```

use std::sync::Arc;
use std::time::Duration;

use cardfile_media::{PlaybackState, Player, media_kind};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: play <audio-or-video-file>")?;
    let data: Arc<[u8]> = std::fs::read(&path)?.into();
    let name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str());

    let Some(kind) = media_kind("", name) else {
        return Err(format!("{path}: not playable inline").into());
    };
    let player = Player::open(data, "", name)?;
    println!(
        "{kind:?} -> {:?}, duration {:?}, video size {:?}",
        player.kind(),
        player.duration(),
        player.video_size()
    );

    player.play();
    let mut last_seq = 0;
    while player.state() != PlaybackState::Ended {
        let frame = player.current_frame().map(|f| (f.seq, f.width, f.height));
        let new_frame = match frame {
            Some((seq, ..)) if seq != last_seq => {
                last_seq = seq;
                " (new frame)"
            }
            _ => "",
        };
        println!("{:>8.2}s{new_frame}", player.position().as_secs_f64());
        std::thread::sleep(Duration::from_millis(250));
    }
    println!("ended at {:.2}s", player.position().as_secs_f64());
    Ok(())
}
