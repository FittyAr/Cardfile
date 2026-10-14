//! Classification of attachments into inline-playable media kinds.

/// What kind of inline player an attachment needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// Audio only: MP3, WAV, OGG/Vorbis, FLAC, AAC/M4A.
    Audio,
    /// Video: MP4 container with H.264 video (and optionally an AAC track).
    Video,
}

/// Classifies an attachment by MIME type and/or file name.
///
/// The MIME type wins when it is recognised; otherwise (empty, generic such as
/// `application/octet-stream`, or simply unknown) the file extension decides.
/// Matching is case-insensitive and MIME parameters (`; codecs=...`) are ignored.
///
/// Returns `None` when the attachment cannot be played inline; the UI should then
/// offer to open it in the system player. `Some` is a strong hint, not a
/// guarantee: [`Player::open`](crate::Player::open) still returns
/// [`MediaError::Unsupported`](crate::MediaError::Unsupported) for, e.g., an
/// MP4 that carries HEVC instead of H.264.
pub fn media_kind(mime: &str, file_name: Option<&str>) -> Option<MediaKind> {
    kind_from_mime(mime).or_else(|| file_name.and_then(kind_from_file_name))
}

fn kind_from_mime(mime: &str) -> Option<MediaKind> {
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" | "audio/x-mp3"
        | "audio/wav" | "audio/wave" | "audio/x-wav" | "audio/vnd.wave" | "audio/x-pn-wav"
        | "audio/ogg" | "audio/vorbis" | "audio/x-vorbis+ogg" | "audio/flac" | "audio/x-flac"
        | "audio/aac" | "audio/x-aac" | "audio/aacp" | "audio/mp4" | "audio/m4a"
        | "audio/x-m4a" => Some(MediaKind::Audio),
        "video/mp4" | "video/x-m4v" | "video/m4v" => Some(MediaKind::Video),
        _ => None,
    }
}

fn kind_from_file_name(name: &str) -> Option<MediaKind> {
    let (_, ext) = name.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "mp3" | "wav" | "wave" | "ogg" | "oga" | "flac" | "aac" | "m4a" => Some(MediaKind::Audio),
        "mp4" | "m4v" => Some(MediaKind::Video),
        _ => None,
    }
}

/// The lowercase extension of `file_name`, used as a probe hint.
pub(crate) fn extension(file_name: Option<&str>) -> Option<String> {
    file_name
        .and_then(|n| n.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_mime_types() {
        for mime in [
            "audio/mpeg",
            "audio/wav",
            "audio/x-wav",
            "audio/ogg",
            "audio/flac",
            "audio/aac",
            "audio/mp4",
            "audio/x-m4a",
            "AUDIO/MPEG",
            "audio/ogg; codecs=vorbis",
        ] {
            assert_eq!(media_kind(mime, None), Some(MediaKind::Audio), "{mime}");
        }
    }

    #[test]
    fn video_mime_types() {
        assert_eq!(media_kind("video/mp4", None), Some(MediaKind::Video));
        assert_eq!(
            media_kind("video/mp4; codecs=\"avc1.42E01E\"", None),
            Some(MediaKind::Video)
        );
        assert_eq!(media_kind("video/x-m4v", None), Some(MediaKind::Video));
    }

    #[test]
    fn extensions_when_mime_is_generic_or_unknown() {
        let cases = [
            ("song.MP3", Some(MediaKind::Audio)),
            ("a.wav", Some(MediaKind::Audio)),
            ("a.ogg", Some(MediaKind::Audio)),
            ("a.oga", Some(MediaKind::Audio)),
            ("a.flac", Some(MediaKind::Audio)),
            ("a.aac", Some(MediaKind::Audio)),
            ("a.m4a", Some(MediaKind::Audio)),
            ("clip.mp4", Some(MediaKind::Video)),
            ("clip.M4V", Some(MediaKind::Video)),
            ("clip.webm", None),
            ("clip.mkv", None),
            ("noext", None),
        ];
        for (name, want) in cases {
            assert_eq!(
                media_kind("application/octet-stream", Some(name)),
                want,
                "{name}"
            );
            assert_eq!(media_kind("", Some(name)), want, "{name}");
        }
    }

    #[test]
    fn mime_wins_over_extension() {
        assert_eq!(
            media_kind("video/mp4", Some("x.mp3")),
            Some(MediaKind::Video)
        );
        assert_eq!(
            media_kind("audio/mp4", Some("x.mp4")),
            Some(MediaKind::Audio)
        );
    }

    #[test]
    fn not_playable() {
        assert_eq!(media_kind("video/webm", None), None);
        assert_eq!(media_kind("video/quicktime", None), None);
        assert_eq!(media_kind("image/png", Some("a.png")), None);
        assert_eq!(media_kind("application/pdf", None), None);
        assert_eq!(media_kind("", None), None);
    }

    #[test]
    fn extension_helper() {
        assert_eq!(extension(Some("A.Mp3")).as_deref(), Some("mp3"));
        assert_eq!(extension(Some("none")), None);
        assert_eq!(extension(None), None);
    }
}
