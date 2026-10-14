//! Reading and writing playlists as text in the M3U / M3U8 format (UTF-8), with the extended
//! directives `#EXTM3U`, `#PLAYLIST:` and `#EXTINF:`. No IO: callers read and write the files.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::track::Track;

/// A playlist read from M3U text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedPlaylist {
    /// The `#PLAYLIST:` name, when the file has one.
    pub name: Option<String>,
    /// The entries that could be turned into a local path, in file order.
    pub tracks: Vec<Track>,
}

/// What an `#EXTINF` line said about the entry that follows it.
#[derive(Default)]
struct ExtInf {
    duration: Option<Duration>,
    artist: Option<String>,
    title: Option<String>,
}

/// Parses M3U text. Relative paths are resolved against `base_dir`, `file://` URIs are decoded
/// into paths, other URLs (such as `http://`) are skipped, and Windows paths (`C:\Music\a.mp3`)
/// are kept as written. Unknown `#` lines and blank lines are ignored.
pub fn parse_m3u(text: &str, base_dir: &Path) -> ParsedPlaylist {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut parsed = ParsedPlaylist::default();
    let mut info = ExtInf::default();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(name) = line.strip_prefix("#PLAYLIST:") {
            let name = name.trim();
            parsed.name = (!name.is_empty()).then(|| name.to_string());
        } else if let Some(rest) = line.strip_prefix("#EXTINF:") {
            info = parse_extinf(rest);
        } else if line.starts_with('#') {
            // `#EXTM3U` and any directive we do not know.
        } else {
            let entry = std::mem::take(&mut info);
            if let Some(path) = entry_path(line, base_dir) {
                let mut track = Track::new(path);
                track.meta.duration = entry.duration;
                track.meta.artist = entry.artist;
                track.meta.title = entry.title;
                parsed.tracks.push(track);
            }
        }
    }
    parsed
}

/// Writes extended M3U text: `#EXTM3U`, `#PLAYLIST:` when `name` is given, then an `#EXTINF`
/// line (seconds, or -1 when unknown, and the track label) and the path of each track, with
/// `\n` line endings. The label has the artist only when the track has a title tag.
pub fn write_m3u(name: Option<&str>, tracks: &[Track]) -> String {
    let mut out = String::from("#EXTM3U\n");
    if let Some(name) = name {
        out.push_str(&format!("#PLAYLIST:{}\n", one_line(name)));
    }
    for track in tracks {
        let seconds = track
            .meta
            .duration
            .map_or(-1, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        out.push_str(&format!("#EXTINF:{seconds},{}\n", one_line(&track.label())));
        out.push_str(&track.path.to_string_lossy());
        out.push('\n');
    }
    out
}

/// Replaces line breaks, which would corrupt the format, with spaces.
fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Parses what follows `#EXTINF:`, "<seconds>,<Artist> - <Title>".
fn parse_extinf(rest: &str) -> ExtInf {
    let (seconds, text) = rest.split_once(',').unwrap_or((rest, ""));
    let duration = seconds
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|s| *s >= 0.0)
        .and_then(|s| Duration::try_from_secs_f64(s).ok());
    let text = text.trim();
    let (artist, title) = match text.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => (
            Some(artist.trim().to_string()),
            Some(title.trim().to_string()),
        ),
        _ => (None, (!text.is_empty()).then(|| text.to_string())),
    };
    ExtInf {
        duration,
        artist,
        title,
    }
}

/// The local path an entry line stands for, or `None` for URLs we do not play.
fn entry_path(line: &str, base_dir: &Path) -> Option<PathBuf> {
    if let Some(uri) = line.strip_prefix("file://") {
        return Some(PathBuf::from(file_uri_path(uri)));
    }
    if line.contains("://") {
        return None;
    }
    if is_windows_path(line) {
        return Some(PathBuf::from(line));
    }
    let path = Path::new(line);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    })
}

/// The path part of a `file://` URI (what follows the scheme): the optional host is dropped,
/// percent escapes are decoded, and the slash before a Windows drive letter is removed.
fn file_uri_path(uri: &str) -> String {
    let path = uri.strip_prefix("localhost").unwrap_or(uri);
    let decoded = percent_decode(path);
    let bytes = decoded.as_bytes();
    let drive =
        bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    if drive {
        decoded[1..].to_string()
    } else {
        decoded
    }
}

/// Whether `line` starts with a drive letter ("C:\" or "C:/") or is a UNC path ("\\server").
fn is_windows_path(line: &str) -> bool {
    let bytes = line.as_bytes();
    let drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    drive || line.starts_with("\\\\")
}

/// Decodes `%XX` escapes as UTF-8, leaving malformed escapes untouched.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| hex_byte(bytes[i + 1], bytes[i + 2]))
            .flatten();
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_byte(high: u8, low: u8) -> Option<u8> {
    let digit = |b: u8| char::from(b).to_digit(16);
    Some((digit(high)? * 16 + digit(low)?) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> &'static Path {
        Path::new("/home/me/lists")
    }

    #[test]
    fn parses_extended_entries() {
        let text = "#EXTM3U\n#PLAYLIST:Evening\n#EXTINF:215,Erik Satie - Gymnopédie No. 1\n\
                    /music/satie.flac\n#EXTINF:-1,Just a title\n/music/other.mp3\n";
        let parsed = parse_m3u(text, base());
        assert_eq!(parsed.name.as_deref(), Some("Evening"));
        assert_eq!(parsed.tracks.len(), 2);
        let first = &parsed.tracks[0];
        assert_eq!(first.path, PathBuf::from("/music/satie.flac"));
        assert_eq!(first.meta.artist.as_deref(), Some("Erik Satie"));
        assert_eq!(first.meta.title.as_deref(), Some("Gymnopédie No. 1"));
        assert_eq!(first.meta.duration, Some(Duration::from_secs(215)));
        let second = &parsed.tracks[1];
        assert_eq!(second.meta.artist, None);
        assert_eq!(second.meta.title.as_deref(), Some("Just a title"));
        assert_eq!(second.meta.duration, None);
    }

    #[test]
    fn splits_the_label_on_the_first_separator_only() {
        let parsed = parse_m3u("#EXTINF:1,A - B - C\n/a.mp3\n", base());
        assert_eq!(parsed.tracks[0].meta.artist.as_deref(), Some("A"));
        assert_eq!(parsed.tracks[0].meta.title.as_deref(), Some("B - C"));
    }

    #[test]
    fn handles_bom_crlf_blank_lines_and_unknown_directives() {
        let text = "\u{feff}#EXTM3U\r\n\r\n#EXTALB:Something\r\n#EXTINF:10,T\r\n\r\n/a.mp3\r\n";
        let parsed = parse_m3u(text, base());
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].meta.title.as_deref(), Some("T"));
    }

    #[test]
    fn plain_m3u_without_header_works() {
        let parsed = parse_m3u("/a.mp3\n/b.mp3\n", base());
        assert_eq!(parsed.name, None);
        assert_eq!(parsed.tracks.len(), 2);
        assert_eq!(parsed.tracks[0].meta, Default::default());
    }

    #[test]
    fn extinf_applies_to_the_next_entry_only() {
        let parsed = parse_m3u("#EXTINF:5,T\n/a.mp3\n/b.mp3\n", base());
        assert_eq!(parsed.tracks[0].meta.title.as_deref(), Some("T"));
        assert_eq!(parsed.tracks[1].meta.title, None);
    }

    #[test]
    fn relative_paths_resolve_against_the_base_directory() {
        let parsed = parse_m3u("a.mp3\n../up/b.mp3\n/abs/c.mp3\n", base());
        assert_eq!(parsed.tracks[0].path, PathBuf::from("/home/me/lists/a.mp3"));
        assert_eq!(
            parsed.tracks[1].path,
            PathBuf::from("/home/me/lists/../up/b.mp3")
        );
        assert_eq!(parsed.tracks[2].path, PathBuf::from("/abs/c.mp3"));
    }

    #[test]
    fn file_uris_are_decoded_and_http_is_skipped() {
        let text = "file:///music/My%20Song%20%C3%A9.mp3\nhttp://radio.example/stream\n\
                    file://localhost/tmp/a.mp3\nfile:///C:/Users/me/a%20b.mp3\n";
        let parsed = parse_m3u(text, base());
        let paths: Vec<_> = parsed.tracks.iter().map(|t| t.path.clone()).collect();
        assert_eq!(
            paths,
            [
                PathBuf::from("/music/My Song é.mp3"),
                PathBuf::from("/tmp/a.mp3"),
                PathBuf::from("C:/Users/me/a b.mp3"),
            ]
        );
    }

    #[test]
    fn skipped_urls_do_not_leak_their_extinf() {
        let parsed = parse_m3u("#EXTINF:1,Radio\nhttp://x/y\n/a.mp3\n", base());
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].meta.title, None);
    }

    #[test]
    fn windows_paths_are_kept_as_is() {
        let parsed = parse_m3u("C:\\Music\\a.mp3\n\\\\server\\share\\b.mp3\n", base());
        assert_eq!(parsed.tracks[0].path, PathBuf::from("C:\\Music\\a.mp3"));
        assert_eq!(
            parsed.tracks[1].path,
            PathBuf::from("\\\\server\\share\\b.mp3")
        );
    }

    #[test]
    fn bad_durations_are_unknown() {
        let parsed = parse_m3u(
            "#EXTINF:abc,T\n/a.mp3\n#EXTINF:,U\n/b.mp3\n#EXTINF:12.7,V\n/c.mp3\n",
            base(),
        );
        assert_eq!(parsed.tracks[0].meta.duration, None);
        assert_eq!(parsed.tracks[1].meta.duration, None);
        assert_eq!(
            parsed.tracks[2].meta.duration,
            Some(Duration::from_secs_f64(12.7))
        );
    }

    #[test]
    fn writes_extended_m3u() {
        let mut tagged = Track::new("/music/a.flac");
        tagged.meta.artist = Some("Erik Satie".into());
        tagged.meta.title = Some("Gymnopédie".into());
        tagged.meta.duration = Some(Duration::from_millis(215_900));
        let untitled = Track::new("/music/01 Intro.mp3");
        let text = write_m3u(Some("Evening"), &[tagged, untitled]);
        assert_eq!(
            text,
            "#EXTM3U\n#PLAYLIST:Evening\n#EXTINF:215,Erik Satie - Gymnopédie\n/music/a.flac\n\
             #EXTINF:-1,01 Intro\n/music/01 Intro.mp3\n"
        );
        assert_eq!(write_m3u(None, &[]), "#EXTM3U\n");
    }

    #[test]
    fn round_trips_names_tags_and_paths() {
        let mut a = Track::new("/music/a.flac");
        a.meta.artist = Some("Erik Satie".into());
        a.meta.title = Some("Gymnopédie - No. 1".into());
        a.meta.duration = Some(Duration::from_secs(215));
        let mut b = Track::new("/music/b c.mp3");
        b.meta.title = Some("Solo".into());
        let tracks = vec![a, b];
        let parsed = parse_m3u(&write_m3u(Some("Mix"), &tracks), base());
        assert_eq!(parsed.name.as_deref(), Some("Mix"));
        assert_eq!(parsed.tracks, tracks);
    }

    #[test]
    fn round_trip_without_tags_keeps_paths_and_durations() {
        let mut a = Track::new("/music/01 Intro.mp3");
        a.meta.duration = Some(Duration::from_secs(60));
        let parsed = parse_m3u(&write_m3u(None, std::slice::from_ref(&a)), base());
        assert_eq!(parsed.name, None);
        assert_eq!(parsed.tracks[0].path, a.path);
        assert_eq!(parsed.tracks[0].meta.duration, a.meta.duration);
        assert_eq!(parsed.tracks[0].label(), a.label());
    }
}
