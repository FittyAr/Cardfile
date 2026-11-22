//! Classification of attachments into inline-playable media kinds.

/// What kind of inline player an attachment needs.
///
/// Only audio is played inline today; the enum is kept (rather than a `bool`)
/// so callers stay unchanged if another kind is ever added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// Patent-free audio: MP3, WAV (PCM), OGG/Vorbis, FLAC.
    Audio,
}

/// Classifies an attachment by MIME type and/or file name.
///
/// The MIME type wins when it is recognised; otherwise (empty, generic such as
/// `application/octet-stream`, or simply unknown) the file extension decides.
/// Matching is case-insensitive and MIME parameters (`; codecs=...`) are ignored.
///
/// Returns `None` when the attachment cannot be played inline — every video,
/// AAC/M4A, Opus and any other format besides MP3, WAV, OGG/Vorbis and FLAC; the
/// UI should then open it in the system player. `Some` is a strong hint, not a
/// guarantee: [`Player::open`](crate::Player::open) still returns
/// [`MediaError::Unsupported`](crate::MediaError::Unsupported) for, e.g., an
/// OGG file that carries Opus instead of Vorbis.
pub fn media_kind(mime: &str, file_name: Option<&str>) -> Option<MediaKind> {
    match kind_from_mime(mime) {
        MimeVerdict::Playable(kind) => Some(kind),
        MimeVerdict::NotPlayable => None,
        MimeVerdict::Unknown => file_name.and_then(kind_from_file_name),
    }
}

/// What the MIME type alone says.
enum MimeVerdict {
    Playable(MediaKind),
    /// A recognised media type that is never played inline (all video, AAC…):
    /// a misleading extension must not override it.
    NotPlayable,
    /// Empty, generic or unknown: let the extension decide.
    Unknown,
}

fn kind_from_mime(mime: &str) -> MimeVerdict {
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match essence.as_str() {
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" | "audio/x-mp3"
        | "audio/wav" | "audio/wave" | "audio/x-wav" | "audio/vnd.wave" | "audio/x-pn-wav"
        | "audio/ogg" | "audio/vorbis" | "audio/x-vorbis+ogg" | "audio/flac" | "audio/x-flac" => {
            MimeVerdict::Playable(MediaKind::Audio)
        }
        "audio/aac" | "audio/x-aac" | "audio/aacp" | "audio/mp4" | "audio/m4a" | "audio/x-m4a"
        | "audio/opus" | "audio/webm" => MimeVerdict::NotPlayable,
        video if video.starts_with("video/") => MimeVerdict::NotPlayable,
        _ => MimeVerdict::Unknown,
    }
}

fn kind_from_file_name(name: &str) -> Option<MediaKind> {
    let (_, ext) = name.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "mp3" | "wav" | "wave" | "ogg" | "oga" | "flac" => Some(MediaKind::Audio),
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
            "audio/x-flac",
            "AUDIO/MPEG",
            "audio/ogg; codecs=vorbis",
        ] {
            assert_eq!(media_kind(mime, None), Some(MediaKind::Audio), "{mime}");
        }
    }

    #[test]
    fn patent_encumbered_and_video_mime_types_are_not_playable() {
        for mime in [
            "audio/aac",
            "audio/mp4",
            "audio/x-m4a",
            "audio/opus",
            "video/mp4",
            "video/mp4; codecs=\"avc1.42E01E\"",
            "video/x-m4v",
            "video/webm",
            "video/quicktime",
        ] {
            assert_eq!(media_kind(mime, None), None, "{mime}");
        }
    }

    #[test]
    fn extensions_when_mime_is_generic_or_unknown() {
        let cases = [
            ("song.MP3", Some(MediaKind::Audio)),
            ("a.wav", Some(MediaKind::Audio)),
            ("a.wave", Some(MediaKind::Audio)),
            ("a.ogg", Some(MediaKind::Audio)),
            ("a.oga", Some(MediaKind::Audio)),
            ("a.flac", Some(MediaKind::Audio)),
            ("a.aac", None),
            ("a.m4a", None),
            ("a.opus", None),
            ("clip.mp4", None),
            ("clip.M4V", None),
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
        assert_eq!(media_kind("video/mp4", Some("x.mp3")), None);
        assert_eq!(media_kind("audio/aac", Some("x.mp3")), None);
        assert_eq!(
            media_kind("audio/mpeg", Some("x.mp4")),
            Some(MediaKind::Audio)
        );
    }

    #[test]
    fn not_playable() {
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
