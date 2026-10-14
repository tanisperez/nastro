//! A playlist: tracks, a multi-selection, the track loaded in the player, sorting and search.
//!
//! The `current` track and the selection are tracked by position, so every operation that
//! moves or removes tracks remaps them: `current` keeps pointing at the same track, or becomes
//! `None` when that track is removed.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use crate::text::fold;
use crate::track::{Track, TrackMeta};

/// What [`Playlist::sort`] orders by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    /// The title tag.
    Title,
    /// The artist tag.
    Artist,
    /// The album tag.
    Album,
    /// The file name, with its extension.
    FileName,
    /// The whole path.
    Path,
    /// The playing time.
    Duration,
}

/// An ordered list of tracks with a selection.
#[derive(Debug, Clone, Default)]
pub struct Playlist {
    name: String,
    tracks: Vec<Track>,
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
    current: Option<usize>,
    generation: u64,
}

impl Playlist {
    /// An empty playlist called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Playlist {
            name: name.into(),
            ..Playlist::default()
        }
    }

    /// The playlist name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renames the playlist.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.bump();
    }

    /// A counter that grows on every change to the name or to the tracks (adding, removing,
    /// reordering, tags). Selection and `current` changes do not touch it: it is the key for
    /// caching data derived from the contents, such as filter results and labels.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Number of tracks.
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Whether there are no tracks.
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// All the tracks, in order.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// The track at `index`, if any.
    pub fn track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// Appends `tracks` at the end.
    pub fn add(&mut self, tracks: Vec<Track>) {
        let end = self.tracks.len();
        self.insert(end, tracks);
    }

    /// Inserts `tracks` before position `index` (the end when it is past it).
    pub fn insert(&mut self, index: usize, tracks: Vec<Track>) {
        if tracks.is_empty() {
            return;
        }
        let index = index.min(self.tracks.len());
        let count = tracks.len();
        self.tracks.splice(index..index, tracks);
        self.remap(|i| Some(if i >= index { i + count } else { i }));
        self.bump();
    }

    /// Removes the tracks at `indices` (duplicates and out of range values are ignored).
    pub fn remove(&mut self, indices: &[usize]) {
        let doomed: BTreeSet<usize> = indices
            .iter()
            .copied()
            .filter(|i| *i < self.len())
            .collect();
        if doomed.is_empty() {
            return;
        }
        // New position of each survivor: its old position minus the removed tracks before it.
        let new_position = |i: usize| (!doomed.contains(&i)).then(|| i - doomed.range(..i).count());
        self.remap(new_position);
        let mut position = 0;
        self.tracks.retain(|_| {
            let keep = !doomed.contains(&position);
            position += 1;
            keep
        });
        self.bump();
    }

    /// Removes the selected tracks and returns how many there were.
    pub fn remove_selected(&mut self) -> usize {
        let selected = self.selected_indices();
        self.remove(&selected);
        selected.len()
    }

    /// Removes every track that is not selected. Does nothing without a selection.
    pub fn crop_to_selection(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let others: Vec<usize> = (0..self.len())
            .filter(|i| !self.selected.contains(i))
            .collect();
        self.remove(&others);
    }

    /// Removes every track.
    pub fn clear(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        self.tracks.clear();
        self.selected.clear();
        self.anchor = None;
        self.current = None;
        self.bump();
    }

    /// The track loaded in the player.
    pub fn current(&self) -> Option<usize> {
        self.current
    }

    /// Sets the loaded track. An out of range index becomes `None`.
    pub fn set_current(&mut self, current: Option<usize>) {
        self.current = current.filter(|i| *i < self.len());
    }

    /// Selects only `index` and makes it the anchor (a plain click).
    pub fn select_only(&mut self, index: usize) {
        if index < self.len() {
            self.selected.clear();
            self.selected.insert(index);
            self.anchor = Some(index);
        }
    }

    /// Toggles `index` in the selection and makes it the anchor (ctrl-click).
    pub fn toggle_selected(&mut self, index: usize) {
        if index < self.len() {
            if !self.selected.remove(&index) {
                self.selected.insert(index);
            }
            self.anchor = Some(index);
        }
    }

    /// Selects everything between the anchor and `index`, replacing the selection (shift-click).
    /// Without an anchor it behaves like [`select_only`](Self::select_only). The anchor stays.
    pub fn select_range_to(&mut self, index: usize) {
        if index >= self.len() {
            return;
        }
        let anchor = self.anchor.unwrap_or(index).min(self.len() - 1);
        self.selected = (anchor.min(index)..=anchor.max(index)).collect();
        self.anchor = Some(anchor);
    }

    /// Selects every track.
    pub fn select_all(&mut self) {
        self.selected = (0..self.len()).collect();
    }

    /// Clears the selection and the anchor.
    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Selects what is not selected and the other way round.
    pub fn invert_selection(&mut self) {
        self.selected = (0..self.len())
            .filter(|i| !self.selected.contains(i))
            .collect();
    }

    /// Whether the track at `index` is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// The selected positions, ascending.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// Moves the selected tracks by `delta` positions (negative is up). The selected tracks keep
    /// their order and the gaps between them, and the whole block stops at the ends of the list.
    pub fn move_selected(&mut self, delta: isize) {
        let (Some(&first), Some(&last)) = (self.selected.first(), self.selected.last()) else {
            return;
        };
        let min_delta = -(first as isize);
        let max_delta = (self.len() - 1 - last) as isize;
        let delta = delta.clamp(min_delta, max_delta);
        if delta == 0 {
            return;
        }
        // `order[new] = Some(old)` for the moved block, the rest fills the free slots in order.
        let mut order: Vec<Option<usize>> = vec![None; self.len()];
        for &old in &self.selected {
            order[old.saturating_add_signed(delta)] = Some(old);
        }
        let mut others = (0..self.len()).filter(|i| !self.selected.contains(i));
        let order = order
            .into_iter()
            .filter_map(|slot| slot.or_else(|| others.next()))
            .collect();
        self.permute(order);
    }

    /// Sorts by `key`, keeping equal tracks in their order. Text keys ignore case and accents,
    /// and tracks without the tag go last.
    pub fn sort(&mut self, key: SortKey) {
        let text = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(fold)
        };
        let file_name = |track: &Track| track.path.file_name().map(|n| fold(&n.to_string_lossy()));
        let order = match key {
            SortKey::Title => sorted_order(self.tracks.iter().map(|t| text(&t.meta.title))),
            SortKey::Artist => sorted_order(self.tracks.iter().map(|t| text(&t.meta.artist))),
            SortKey::Album => sorted_order(self.tracks.iter().map(|t| text(&t.meta.album))),
            SortKey::FileName => sorted_order(self.tracks.iter().map(file_name)),
            SortKey::Path => sorted_order(
                self.tracks
                    .iter()
                    .map(|t| Some(fold(&t.path.to_string_lossy()))),
            ),
            SortKey::Duration => sorted_order(self.tracks.iter().map(|t| t.meta.duration)),
        };
        self.permute(order);
    }

    /// Reverses the order.
    pub fn reverse(&mut self) {
        self.permute((0..self.len()).rev().collect());
    }

    /// Shuffles the tracks. The same `seed` gives the same order for the same length.
    pub fn randomize(&mut self, seed: u64) {
        let mut order: Vec<usize> = (0..self.len()).collect();
        fastrand::Rng::with_seed(seed).shuffle(&mut order);
        self.permute(order);
    }

    /// Applies the tags in `meta` to every track with this `path`: the fields that are `Some`
    /// replace the track's, the others are kept (a file without tags does not erase the title
    /// read from a playlist file). Returns whether anything changed.
    pub fn update_meta(&mut self, path: &Path, meta: &TrackMeta) -> bool {
        let mut changed = false;
        for track in self.tracks.iter_mut().filter(|t| t.path == path) {
            let before = track.meta.clone();
            let target = &mut track.meta;
            merge(&mut target.title, &meta.title);
            merge(&mut target.artist, &meta.artist);
            merge(&mut target.album, &meta.album);
            merge(&mut target.genre, &meta.genre);
            merge(&mut target.duration, &meta.duration);
            changed |= *target != before;
        }
        if changed {
            self.bump();
        }
        changed
    }

    /// The summed playing time and how many tracks have an unknown duration.
    pub fn total_duration(&self) -> (Duration, usize) {
        let mut total = Duration::ZERO;
        let mut unknown = 0;
        for track in &self.tracks {
            match track.meta.duration {
                Some(duration) => total += duration,
                None => unknown += 1,
            }
        }
        (total, unknown)
    }

    /// The positions whose label or file name contains every word of `query`, ignoring case and
    /// accents. An empty query matches everything.
    pub fn filter(&self, query: &str) -> Vec<usize> {
        let query = fold(query);
        let words: Vec<&str> = query.split_whitespace().collect();
        (0..self.len())
            .filter(|&i| {
                let track = &self.tracks[i];
                let file_name = track.path.file_name().unwrap_or_default().to_string_lossy();
                let haystack = fold(&format!("{} {}", track.label(), file_name));
                words.iter().all(|word| haystack.contains(word))
            })
            .collect()
    }

    fn bump(&mut self) {
        self.generation += 1;
    }

    /// Reorders the tracks so that `order[new] = old`, and remaps the positions.
    fn permute(&mut self, order: Vec<usize>) {
        let mut new_position = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            new_position[old] = new;
        }
        let mut old_tracks: Vec<Option<Track>> = std::mem::take(&mut self.tracks)
            .into_iter()
            .map(Some)
            .collect();
        self.tracks = order
            .iter()
            .filter_map(|&old| old_tracks[old].take())
            .collect();
        self.remap(|i| new_position.get(i).copied());
        self.bump();
    }

    /// Moves the selection, the anchor and `current` to new positions; `None` drops them.
    fn remap(&mut self, new_position: impl Fn(usize) -> Option<usize>) {
        self.selected = self
            .selected
            .iter()
            .filter_map(|&i| new_position(i))
            .collect();
        self.anchor = self.anchor.and_then(&new_position);
        self.current = self.current.and_then(&new_position);
    }
}

/// Replaces `target` by `new` when `new` has a value.
fn merge<T: Clone>(target: &mut Option<T>, new: &Option<T>) {
    if new.is_some() {
        target.clone_from(new);
    }
}

/// The positions of `keys` in ascending key order, stable, with the missing keys last.
fn sorted_order<K: Ord>(keys: impl Iterator<Item = Option<K>>) -> Vec<usize> {
    let keys: Vec<Option<K>> = keys.collect();
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| match (&keys[a], &keys[b]) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    order
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn titled(path: &str, title: Option<&str>, secs: Option<u64>) -> Track {
        let mut track = Track::new(path);
        track.meta.title = title.map(str::to_string);
        track.meta.duration = secs.map(Duration::from_secs);
        track
    }

    fn playlist(n: usize) -> Playlist {
        let mut list = Playlist::new("test");
        list.add((0..n).map(|i| Track::new(format!("/m/{i}.mp3"))).collect());
        list
    }

    fn names(list: &Playlist) -> Vec<String> {
        list.tracks()
            .iter()
            .map(|t| t.path.file_stem().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn add_and_insert_keep_current_and_selection() {
        let mut list = playlist(3);
        list.set_current(Some(1));
        list.select_only(2);
        list.insert(1, vec![Track::new("/m/x.mp3"), Track::new("/m/y.mp3")]);
        assert_eq!(names(&list), ["0", "x", "y", "1", "2"]);
        assert_eq!(list.current(), Some(3));
        assert_eq!(list.selected_indices(), [4]);
        list.add(vec![Track::new("/m/z.mp3")]);
        assert_eq!(list.len(), 6);
        assert_eq!(list.current(), Some(3));
    }

    #[test]
    fn insert_past_the_end_appends() {
        let mut list = playlist(2);
        list.insert(99, vec![Track::new("/m/z.mp3")]);
        assert_eq!(names(&list), ["0", "1", "z"]);
    }

    #[test]
    fn generation_grows_on_mutations_only() {
        let mut list = playlist(3);
        let start = list.generation();
        list.select_only(1);
        list.set_current(Some(0));
        assert_eq!(list.generation(), start);
        list.reverse();
        assert!(list.generation() > start);
        let before = list.generation();
        list.set_name("other");
        assert!(list.generation() > before);
        let before = list.generation();
        list.add(Vec::new());
        list.remove(&[10]);
        assert_eq!(list.generation(), before);
    }

    #[test]
    fn remove_remaps_current_and_selection() {
        let mut list = playlist(6);
        list.set_current(Some(4));
        list.select_only(1);
        list.toggle_selected(5);
        list.remove(&[0, 2, 2, 40]);
        assert_eq!(names(&list), ["1", "3", "4", "5"]);
        assert_eq!(list.current(), Some(2));
        assert_eq!(list.selected_indices(), [0, 3]);
    }

    #[test]
    fn removing_the_current_track_clears_it() {
        let mut list = playlist(3);
        list.set_current(Some(1));
        list.remove(&[1]);
        assert_eq!(list.current(), None);
    }

    #[test]
    fn remove_selected_and_crop() {
        let mut list = playlist(5);
        list.select_only(1);
        list.toggle_selected(3);
        list.set_current(Some(3));
        let mut cropped = list.clone();
        cropped.crop_to_selection();
        assert_eq!(names(&cropped), ["1", "3"]);
        assert_eq!(cropped.current(), Some(1));
        assert_eq!(cropped.selected_indices(), [0, 1]);
        assert_eq!(list.remove_selected(), 2);
        assert_eq!(names(&list), ["0", "2", "4"]);
        assert_eq!(list.current(), None);
        assert!(list.selected_indices().is_empty());
        assert_eq!(list.remove_selected(), 0);
    }

    #[test]
    fn crop_without_selection_keeps_everything() {
        let mut list = playlist(3);
        list.crop_to_selection();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn clear_empties_everything() {
        let mut list = playlist(3);
        list.set_current(Some(2));
        list.select_all();
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.current(), None);
        assert!(list.selected_indices().is_empty());
    }

    #[test]
    fn set_current_ignores_out_of_range() {
        let mut list = playlist(2);
        list.set_current(Some(1));
        assert_eq!(list.current(), Some(1));
        list.set_current(Some(2));
        assert_eq!(list.current(), None);
    }

    #[test]
    fn click_ctrl_click_and_shift_click() {
        let mut list = playlist(8);
        list.select_only(2);
        list.select_range_to(5);
        assert_eq!(list.selected_indices(), [2, 3, 4, 5]);
        list.select_range_to(0);
        assert_eq!(list.selected_indices(), [0, 1, 2]);
        list.toggle_selected(6);
        assert_eq!(list.selected_indices(), [0, 1, 2, 6]);
        list.select_range_to(7);
        assert_eq!(list.selected_indices(), [6, 7]);
        list.toggle_selected(7);
        assert!(!list.is_selected(7));
        list.select_only(99);
        assert!(!list.is_selected(99));
    }

    #[test]
    fn shift_click_without_anchor_selects_one() {
        let mut list = playlist(4);
        list.select_range_to(2);
        assert_eq!(list.selected_indices(), [2]);
    }

    #[test]
    fn select_all_invert_and_clear() {
        let mut list = playlist(4);
        list.select_all();
        assert_eq!(list.selected_indices(), [0, 1, 2, 3]);
        list.select_only(1);
        list.invert_selection();
        assert_eq!(list.selected_indices(), [0, 2, 3]);
        list.clear_selection();
        assert!(list.selected_indices().is_empty());
        list.select_range_to(3);
        assert_eq!(list.selected_indices(), [3]);
    }

    #[test]
    fn move_selected_keeps_the_block_and_current() {
        let mut list = playlist(6);
        list.set_current(Some(4));
        list.select_only(1);
        list.toggle_selected(2);
        list.move_selected(2);
        assert_eq!(names(&list), ["0", "3", "4", "1", "2", "5"]);
        assert_eq!(list.selected_indices(), [3, 4]);
        assert_eq!(list.current(), Some(2));
        list.move_selected(-1);
        assert_eq!(names(&list), ["0", "3", "1", "2", "4", "5"]);
        assert_eq!(list.current(), Some(4));
    }

    #[test]
    fn move_selected_clamps_at_the_ends() {
        let mut list = playlist(5);
        list.select_only(1);
        list.toggle_selected(2);
        list.move_selected(-10);
        assert_eq!(names(&list), ["1", "2", "0", "3", "4"]);
        list.move_selected(10);
        assert_eq!(names(&list), ["0", "3", "4", "1", "2"]);
        assert_eq!(list.selected_indices(), [3, 4]);
    }

    #[test]
    fn move_selected_keeps_gaps_between_selected_tracks() {
        let mut list = playlist(6);
        list.select_only(0);
        list.toggle_selected(2);
        list.move_selected(1);
        assert_eq!(names(&list), ["1", "0", "3", "2", "4", "5"]);
        assert_eq!(list.selected_indices(), [1, 3]);
    }

    #[test]
    fn sort_by_title_ignores_case_and_accents_and_puts_missing_last() {
        let mut list = Playlist::new("t");
        list.add(vec![
            titled("/m/a.mp3", None, None),
            titled("/m/b.mp3", Some("zebra"), None),
            titled("/m/c.mp3", Some("Écoute"), None),
            titled("/m/d.mp3", Some("apple"), None),
            titled("/m/e.mp3", Some("  "), None),
        ]);
        list.set_current(Some(2));
        list.sort(SortKey::Title);
        assert_eq!(names(&list), ["d", "c", "b", "a", "e"]);
        assert_eq!(list.current(), Some(1));
    }

    #[test]
    fn sort_is_stable() {
        let mut list = Playlist::new("t");
        let mut a = titled("/m/a.mp3", None, None);
        let mut b = titled("/m/b.mp3", None, None);
        let mut c = titled("/m/c.mp3", None, None);
        a.meta.artist = Some("X".into());
        b.meta.artist = Some("x".into());
        c.meta.artist = Some("A".into());
        list.add(vec![a, b, c]);
        list.sort(SortKey::Artist);
        assert_eq!(names(&list), ["c", "a", "b"]);
    }

    #[test]
    fn sort_by_album_file_name_path_and_duration() {
        let mut list = Playlist::new("t");
        let mut a = titled("/z/a.mp3", None, Some(30));
        a.meta.album = Some("Beta".into());
        let mut b = titled("/a/b.mp3", None, Some(10));
        b.meta.album = Some("alpha".into());
        let c = titled("/m/C.mp3", None, None);
        list.add(vec![a, b, c]);
        list.sort(SortKey::Album);
        assert_eq!(names(&list), ["b", "a", "C"]);
        list.sort(SortKey::FileName);
        assert_eq!(names(&list), ["a", "b", "C"]);
        list.sort(SortKey::Path);
        assert_eq!(names(&list), ["b", "C", "a"]);
        list.sort(SortKey::Duration);
        assert_eq!(names(&list), ["b", "a", "C"]);
    }

    #[test]
    fn reverse_and_randomize_keep_the_same_tracks() {
        let mut list = playlist(5);
        list.set_current(Some(0));
        list.reverse();
        assert_eq!(names(&list), ["4", "3", "2", "1", "0"]);
        assert_eq!(list.current(), Some(4));

        let mut a = playlist(20);
        let mut b = playlist(20);
        a.randomize(7);
        b.randomize(7);
        assert_eq!(names(&a), names(&b));
        assert_ne!(names(&a), names(&playlist(20)));
        let mut sorted = names(&a);
        sorted.sort_by_key(|n| n.parse::<usize>().unwrap());
        assert_eq!(sorted, names(&playlist(20)));
    }

    #[test]
    fn update_meta_applies_to_every_track_with_the_path() {
        let mut list = playlist(3);
        list.add(vec![Track::new("/m/1.mp3")]);
        let meta = TrackMeta {
            title: Some("One".into()),
            duration: Some(Duration::from_secs(9)),
            ..TrackMeta::default()
        };
        let before = list.generation();
        assert!(list.update_meta(Path::new("/m/1.mp3"), &meta));
        assert!(list.generation() > before);
        assert_eq!(list.track(1).unwrap().meta, meta);
        assert_eq!(list.track(3).unwrap().meta, meta);
        assert!(list.track(0).unwrap().meta.title.is_none());
        let before = list.generation();
        assert!(!list.update_meta(Path::new("/m/1.mp3"), &meta));
        assert!(!list.update_meta(Path::new("/nope.mp3"), &meta));
        assert_eq!(list.generation(), before);
    }

    #[test]
    fn update_meta_keeps_fields_the_probe_did_not_find() {
        let mut list = Playlist::new("t");
        list.add(vec![titled("/m/a.mp3", Some("From m3u"), None)]);
        let probe = TrackMeta {
            duration: Some(Duration::from_secs(5)),
            ..TrackMeta::default()
        };
        assert!(list.update_meta(Path::new("/m/a.mp3"), &probe));
        let meta = &list.track(0).unwrap().meta;
        assert_eq!(meta.title.as_deref(), Some("From m3u"));
        assert_eq!(meta.duration, Some(Duration::from_secs(5)));
    }

    #[test]
    fn total_duration_counts_unknown_tracks() {
        let mut list = Playlist::new("t");
        list.add(vec![
            titled("/m/a.mp3", None, Some(60)),
            titled("/m/b.mp3", None, None),
            titled("/m/c.mp3", None, Some(30)),
        ]);
        assert_eq!(list.total_duration(), (Duration::from_secs(90), 1));
        assert_eq!(Playlist::new("e").total_duration(), (Duration::ZERO, 0));
    }

    #[test]
    fn filter_matches_every_word_ignoring_case_and_accents() {
        let mut list = Playlist::new("t");
        let mut satie = titled("/m/01.flac", Some("Gymnopédie No. 1"), None);
        satie.meta.artist = Some("Erik Satie".into());
        let mut dvorak = titled("/m/02.flac", Some("Humoresque"), None);
        dvorak.meta.artist = Some("Dvořák".into());
        list.add(vec![satie, dvorak, Track::new("/m/Dvorak live.flac")]);
        assert_eq!(list.filter("gymnopedie"), [0]);
        assert_eq!(list.filter("SATIE  no"), [0]);
        assert_eq!(list.filter("dvorak"), [1, 2]);
        assert_eq!(list.filter("dvorak humor"), [1]);
        assert_eq!(list.filter("02.flac"), [1]);
        assert!(list.filter("nothing here").is_empty());
        assert_eq!(list.filter("   "), [0, 1, 2]);
        assert_eq!(list.filter(""), [0, 1, 2]);
    }
}
