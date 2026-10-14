//! The play order: what plays next, with shuffle and repeat.
//!
//! Without shuffle the order is the playlist order. With shuffle it is a random permutation of
//! the positions: every track plays once before any repeats, and "previous" walks back along
//! it. The permutation is built lazily, and again when the playlist length changes or after
//! [`PlayOrder::invalidate`].

use fastrand::Rng;

/// Shuffle and repeat settings plus the shuffled order.
#[derive(Debug, Clone)]
pub struct PlayOrder {
    shuffle: bool,
    repeat: bool,
    rng: Rng,
    /// The permutation of `0..len` used while shuffling. Empty until needed.
    order: Vec<usize>,
    /// Set when the permutation must be rebuilt before its next use.
    stale: bool,
}

impl PlayOrder {
    /// A play order with shuffle and repeat off. The same `seed` gives the same shuffles.
    pub fn new(seed: u64) -> Self {
        PlayOrder {
            shuffle: false,
            repeat: false,
            rng: Rng::with_seed(seed),
            order: Vec::new(),
            stale: true,
        }
    }

    /// Whether shuffle is on.
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Turns shuffle on or off. Turning it on builds a new order on next use.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        if self.shuffle != shuffle {
            self.shuffle = shuffle;
            self.invalidate();
        }
    }

    /// Whether the playlist repeats when it ends.
    pub fn repeat(&self) -> bool {
        self.repeat
    }

    /// Turns repeat on or off.
    pub fn set_repeat(&mut self, repeat: bool) {
        self.repeat = repeat;
    }

    /// Call when the playlist changed (tracks added, removed or reordered): the shuffled order
    /// is rebuilt on next use.
    pub fn invalidate(&mut self) {
        self.stale = true;
    }

    /// What plays when a track ends: `None` at the end of the list when repeat is off.
    pub fn next_auto(&mut self, playlist_len: usize, current: Option<usize>) -> Option<usize> {
        self.step_forward(playlist_len, current, self.repeat)
    }

    /// What the Next button plays: like [`next_auto`](Self::next_auto) but it wraps around even
    /// with repeat off. `None` only for an empty playlist.
    pub fn next_manual(&mut self, playlist_len: usize, current: Option<usize>) -> Option<usize> {
        self.step_forward(playlist_len, current, true)
    }

    /// What the Previous button plays. It wraps from the first track to the last, and with
    /// nothing playing it starts at the first track of the order. `None` for an empty playlist.
    pub fn previous(&mut self, playlist_len: usize, current: Option<usize>) -> Option<usize> {
        if playlist_len == 0 {
            return None;
        }
        if !self.shuffle {
            return Some(match current {
                Some(0) => playlist_len - 1,
                Some(c) if c < playlist_len => c - 1,
                Some(_) => playlist_len - 1,
                None => 0,
            });
        }
        self.prepare(playlist_len, current);
        let position = self.position_of(current);
        Some(match position {
            Some(0) => self.order[playlist_len - 1],
            Some(p) => self.order[p - 1],
            None => self.order[0],
        })
    }

    fn step_forward(
        &mut self,
        playlist_len: usize,
        current: Option<usize>,
        wrap: bool,
    ) -> Option<usize> {
        if playlist_len == 0 {
            return None;
        }
        if !self.shuffle {
            return match current {
                None => Some(0),
                Some(c) if c + 1 < playlist_len => Some(c + 1),
                Some(_) => wrap.then_some(0),
            };
        }
        self.prepare(playlist_len, current);
        match self.position_of(current) {
            None => Some(self.order[0]),
            Some(p) if p + 1 < playlist_len => Some(self.order[p + 1]),
            Some(_) if wrap => {
                self.build(playlist_len, None, current);
                Some(self.order[0])
            }
            Some(_) => None,
        }
    }

    /// Builds the permutation when it is missing, stale or of the wrong length. A track that is
    /// already playing goes first, so the rest of the list plays before anything repeats.
    fn prepare(&mut self, playlist_len: usize, current: Option<usize>) {
        if self.stale || self.order.len() != playlist_len {
            self.build(playlist_len, current, None);
            self.stale = false;
        }
    }

    /// A new permutation of `0..len`, with `first` moved to the front when given, and not
    /// starting with `avoid` when there is more than one track.
    fn build(&mut self, len: usize, first: Option<usize>, avoid: Option<usize>) {
        self.order = (0..len).collect();
        self.rng.shuffle(&mut self.order);
        if let Some(index) = first.and_then(|f| self.order.iter().position(|&t| t == f)) {
            self.order.swap(0, index);
        } else if len > 1 && avoid.is_some_and(|a| self.order[0] == a) {
            self.order.swap(0, len - 1);
        }
    }

    fn position_of(&self, current: Option<usize>) -> Option<usize> {
        current.and_then(|c| self.order.iter().position(|&t| t == c))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn walk(order: &mut PlayOrder, len: usize, steps: usize) -> Vec<usize> {
        let mut played = Vec::new();
        let mut current = None;
        for _ in 0..steps {
            current = order.next_auto(len, current);
            played.push(current.unwrap());
        }
        played
    }

    #[test]
    fn sequential_order_stops_at_the_end() {
        let mut order = PlayOrder::new(1);
        assert_eq!(order.next_auto(3, None), Some(0));
        assert_eq!(order.next_auto(3, Some(0)), Some(1));
        assert_eq!(order.next_auto(3, Some(2)), None);
    }

    #[test]
    fn repeat_wraps_automatically() {
        let mut order = PlayOrder::new(1);
        order.set_repeat(true);
        assert!(order.repeat());
        assert_eq!(order.next_auto(3, Some(2)), Some(0));
    }

    #[test]
    fn next_button_wraps_even_without_repeat() {
        let mut order = PlayOrder::new(1);
        assert_eq!(order.next_manual(3, Some(2)), Some(0));
        assert_eq!(order.next_manual(3, None), Some(0));
        assert_eq!(order.next_manual(0, None), None);
    }

    #[test]
    fn previous_wraps() {
        let mut order = PlayOrder::new(1);
        assert_eq!(order.previous(3, Some(2)), Some(1));
        assert_eq!(order.previous(3, Some(0)), Some(2));
        assert_eq!(order.previous(3, None), Some(0));
        assert_eq!(order.previous(0, None), None);
    }

    #[test]
    fn empty_playlist_has_nothing_next() {
        let mut order = PlayOrder::new(1);
        order.set_shuffle(true);
        assert_eq!(order.next_auto(0, None), None);
        assert_eq!(order.previous(0, None), None);
    }

    #[test]
    fn shuffle_plays_every_track_once_before_repeating() {
        let mut order = PlayOrder::new(42);
        order.set_shuffle(true);
        assert!(order.shuffle());
        let mut first_round = walk(&mut order, 10, 10);
        assert_ne!(first_round, (0..10).collect::<Vec<_>>());
        first_round.sort_unstable();
        assert_eq!(first_round, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_ends_without_repeat_and_reshuffles_with_it() {
        let mut order = PlayOrder::new(5);
        order.set_shuffle(true);
        let played = walk(&mut order, 6, 6);
        assert_eq!(order.next_auto(6, played.last().copied()), None);

        order.set_repeat(true);
        let mut second_round = vec![order.next_auto(6, played.last().copied()).unwrap()];
        for _ in 1..6 {
            second_round.push(order.next_auto(6, second_round.last().copied()).unwrap());
        }
        second_round.sort_unstable();
        assert_eq!(second_round, (0..6).collect::<Vec<_>>());
    }

    #[test]
    fn reshuffle_never_repeats_the_last_track_first() {
        for seed in 0..200 {
            let mut order = PlayOrder::new(seed);
            order.set_shuffle(true);
            order.set_repeat(true);
            let played = walk(&mut order, 3, 30);
            assert!(
                played.windows(2).all(|pair| pair[0] != pair[1]),
                "seed {seed}: {played:?}"
            );
        }
    }

    #[test]
    fn single_track_repeats_itself() {
        let mut order = PlayOrder::new(1);
        order.set_shuffle(true);
        order.set_repeat(true);
        assert_eq!(walk(&mut order, 1, 3), [0, 0, 0]);
    }

    #[test]
    fn previous_walks_back_along_the_permutation() {
        let mut order = PlayOrder::new(9);
        order.set_shuffle(true);
        let played = walk(&mut order, 5, 5);
        let mut current = played.last().copied();
        for expected in played.iter().rev().skip(1) {
            current = order.previous(5, current);
            assert_eq!(current, Some(*expected));
        }
        assert_eq!(order.previous(5, current), Some(played[4]));
    }

    #[test]
    fn shuffle_is_deterministic_for_a_seed() {
        let run = |seed| {
            let mut order = PlayOrder::new(seed);
            order.set_shuffle(true);
            walk(&mut order, 8, 8)
        };
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4));
    }

    #[test]
    fn a_playing_track_goes_first_when_the_order_is_built() {
        let mut order = PlayOrder::new(11);
        order.set_shuffle(true);
        let mut seen = vec![4];
        let mut current = Some(4);
        for _ in 0..5 {
            current = order.next_auto(6, current);
            seen.push(current.unwrap());
        }
        seen.sort_unstable();
        assert_eq!(seen, (0..6).collect::<Vec<_>>());
    }

    #[test]
    fn invalidate_and_length_changes_rebuild_the_order() {
        let mut order = PlayOrder::new(2);
        order.set_shuffle(true);
        assert!(order.next_auto(4, None).unwrap() < 4);
        let next = order.next_auto(8, None).unwrap();
        assert!(next < 8);
        order.invalidate();
        let mut round = walk(&mut order, 8, 8);
        round.sort_unstable();
        assert_eq!(round, (0..8).collect::<Vec<_>>());
    }
}
