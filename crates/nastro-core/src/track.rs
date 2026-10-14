//! A track in a playlist, the tags read from its file and the technical details of its stream.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tags and length read from an audio file. Every field is optional: files without tags are
/// common, and the length is unknown for some streams until they are decoded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackMeta {
    /// The track title tag.
    pub title: Option<String>,
    /// The artist tag (track artist, or album artist when there is no track artist).
    pub artist: Option<String>,
    /// The album tag.
    pub album: Option<String>,
    /// The genre tag, used by the equalizer's automatic preset.
    pub genre: Option<String>,
    /// The playing time.
    pub duration: Option<Duration>,
}

/// One entry of a playlist: a file and what is known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// The audio file.
    pub path: PathBuf,
    /// Tags and length, empty until the file is probed or read from a playlist file.
    pub meta: TrackMeta,
}

impl Track {
    /// A track for `path` with no metadata yet.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Track {
            path: path.into(),
            meta: TrackMeta::default(),
        }
    }

    /// The label the playlist and the marquee show: "Artist - Title", or the title alone, or
    /// the file name without its extension when the file has no title tag.
    pub fn label(&self) -> String {
        let title = self
            .meta
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let artist = self
            .meta
            .artist
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty());
        match (artist, title) {
            (Some(artist), Some(title)) => format!("{artist} - {title}"),
            (None, Some(title)) => title.to_string(),
            _ => file_stem(&self.path),
        }
    }
}

/// The file name of `path` without its extension, or the whole path when it has no file name.
fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Technical details of a decoded stream, shown as badges in the player's display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    /// Short codec name in capitals, as the badge shows it: `FLAC`, `MP3`, `VORBIS`, `AAC`,
    /// `ALAC`, `PCM`, `ADPCM`.
    pub codec: String,
    /// Average bitrate in kbit/s, when known.
    pub bitrate_kbps: Option<u32>,
    /// Sample rate of the source, in Hz.
    pub sample_rate: u32,
    /// Number of channels of the source.
    pub channels: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(artist: Option<&str>, title: Option<&str>) -> Track {
        let mut track = Track::new("/music/01 Intro.flac");
        track.meta.artist = artist.map(str::to_string);
        track.meta.title = title.map(str::to_string);
        track
    }

    #[test]
    fn label_joins_artist_and_title() {
        assert_eq!(
            track(Some("Erik Satie"), Some("Gymnopédie No. 1")).label(),
            "Erik Satie - Gymnopédie No. 1"
        );
    }

    #[test]
    fn label_falls_back_to_title_then_file_stem() {
        assert_eq!(track(None, Some("Clair de lune")).label(), "Clair de lune");
        assert_eq!(track(Some("Ravel"), None).label(), "01 Intro");
        assert_eq!(track(Some(" "), Some("  ")).label(), "01 Intro");
    }
}
