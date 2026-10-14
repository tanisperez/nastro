//! Turning what the user opens or drops (files, folders, playlists) into playlist tracks. This
//! reads the filesystem, so the app runs it on the background executor, never on the UI thread.

use std::fs;
use std::path::{Path, PathBuf};

use nastro_core::{Track, parse_m3u};

/// How deep folders are walked, a guard against symlink loops and absurd trees.
const MAX_DEPTH: usize = 16;

/// Whether `path` is a playlist file Nastro reads (`.m3u` or `.m3u8`).
pub fn is_playlist_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("m3u") || extension.eq_ignore_ascii_case("m3u8")
        })
}

/// The tracks for `paths`, in order: a supported audio file is one track, a folder adds every
/// supported file inside it (recursively, sorted by path, playlists inside folders ignored),
/// and a playlist file adds its entries. Anything else is skipped.
pub fn collect_tracks(paths: &[PathBuf]) -> Vec<Track> {
    let mut tracks = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut files = Vec::new();
            walk(path, 0, &mut files);
            files.sort();
            tracks.extend(files.into_iter().map(Track::new));
        } else if is_playlist_path(path) {
            tracks.extend(read_playlist(path));
        } else if nastro_audio::is_supported_path(path) {
            tracks.push(Track::new(path.clone()));
        }
    }
    tracks
}

/// The entries of a playlist file, empty when it cannot be read.
pub fn read_playlist(path: &Path) -> Vec<Track> {
    match fs::read(path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let base = path.parent().unwrap_or_else(|| Path::new(""));
            parse_m3u(&text, base).tracks
        }
        Err(error) => {
            log::warn!("could not read the playlist {}: {error}", path.display());
            Vec::new()
        }
    }
}

/// Adds every supported audio file under `dir` to `out`.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if hidden {
            continue;
        }
        if kind.is_dir() || (kind.is_symlink() && path.is_dir()) {
            walk(&path, depth + 1, out);
        } else if nastro_audio::is_supported_path(&path) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|error| panic!("{error}"));
        }
        fs::write(path, b"").unwrap_or_else(|error| panic!("{error}"));
    }

    #[test]
    fn folders_expand_sorted_and_skip_other_files() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let root = dir.path();
        touch(&root.join("b/02.flac"));
        touch(&root.join("b/01.mp3"));
        touch(&root.join("a.ogg"));
        touch(&root.join("cover.jpg"));
        touch(&root.join(".hidden/x.mp3"));
        let tracks = collect_tracks(&[root.to_path_buf()]);
        let names: Vec<_> = tracks
            .iter()
            .map(|track| {
                track
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&track.path)
                    .to_path_buf()
            })
            .collect();
        assert_eq!(
            names,
            vec![
                PathBuf::from("a.ogg"),
                PathBuf::from("b/01.mp3"),
                PathBuf::from("b/02.flac")
            ]
        );
    }

    #[test]
    fn playlists_add_their_entries_in_order() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let list = dir.path().join("mix.m3u8");
        fs::write(
            &list,
            "#EXTM3U\n#EXTINF:61,Artist - Song\nsong.mp3\nother.flac\n",
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let tracks = collect_tracks(&[list]);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].path, dir.path().join("song.mp3"));
        assert_eq!(tracks[0].meta.title.as_deref(), Some("Song"));
    }

    #[test]
    fn single_files_keep_the_given_order() {
        let tracks = collect_tracks(&[PathBuf::from("/x/2.mp3"), PathBuf::from("/x/1.mp3")]);
        assert_eq!(tracks[0].path, PathBuf::from("/x/2.mp3"));
        assert!(is_playlist_path(Path::new("a.M3U")));
        assert!(!is_playlist_path(Path::new("a.mp3")));
    }
}
