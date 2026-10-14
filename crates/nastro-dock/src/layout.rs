//! The layout: where each panel is, and the operations that keep docked panels together.

use crate::metrics::Metrics;
use crate::panel::{LayoutPreset, PanelId, PanelState};
use crate::rect::Rect;
use crate::snap::snap;

/// Two edges closer than this count as touching, to absorb floating point error.
const TOUCH_TOLERANCE: f32 = 0.5;

/// Playlist rows of preset A. The column is player + equalizer + playlist, about 902 px tall
/// at 1x. The playlist gets the largest whole number of rows that fits in the 409 px left
/// (floor of (409 - 106) / 22 = 13), so the total is 885.
const COLUMN_PLAYLIST_ROWS: u32 = 13;

/// Playlist rows of preset B. The player row is 274 px and the whole is about 602 px tall at
/// 1x, which leaves 322 px for the playlist: floor of (322 - 106) / 22 = 9 rows, a total of 584.
const L_SHAPE_PLAYLIST_ROWS: u32 = 9;

/// The position and state of the three panels.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    panels: [PanelState; 3],
}

impl Layout {
    /// A layout from the state of each panel, in the order of [`PanelId::ALL`].
    pub fn from_states(panels: [PanelState; 3]) -> Self {
        Layout { panels }
    }

    /// The state of `id`.
    pub fn state(&self, id: PanelId) -> &PanelState {
        &self.panels[id.index()]
    }

    /// Every panel with its state, in the order of [`PanelId::ALL`].
    pub fn states(&self) -> impl Iterator<Item = (PanelId, &PanelState)> {
        PanelId::ALL.into_iter().map(|id| (id, self.state(id)))
    }

    /// A preset with every panel visible and unshaded, the top-left corner at (0, 0) and a gap
    /// between docked panels.
    ///
    /// Sizes at 1x (the totals are the bounds):
    /// - A, a column: 520 x 885. The playlist is as wide as the player and has 13 rows
    ///   (392 px), see [`COLUMN_PLAYLIST_ROWS`].
    /// - B, an L shape: 1046 x 584. The equalizer is stretched to the player's height
    ///   (520 x 274) and the playlist below spans both with 9 rows (304 px).
    /// - C, the default: 1126 x 487. The playlist is 600 x 487, as tall as the player and the
    ///   equalizer together with the gap.
    /// - D, a strip: 1652 x 274. The equalizer is stretched to 274 px and the playlist is 600
    ///   wide and as tall as the player.
    pub fn preset(preset: LayoutPreset, m: &Metrics) -> Layout {
        let (player_w, player_h) = m.player_size;
        let (eq_w, eq_h) = m.equalizer_size;
        let (list_w, _) = m.playlist_size;
        let player = Rect::new(0.0, 0.0, player_w, player_h);
        let (equalizer, playlist) = match preset {
            LayoutPreset::A => {
                let eq = Rect::new(0.0, player.bottom() + m.gap, eq_w, eq_h);
                let list_h = m.playlist_height_for_rows(COLUMN_PLAYLIST_ROWS);
                (eq, Rect::new(0.0, eq.bottom() + m.gap, player_w, list_h))
            }
            LayoutPreset::B => {
                let eq = Rect::new(player.right() + m.gap, 0.0, eq_w, player_h);
                let list_h = m.playlist_height_for_rows(L_SHAPE_PLAYLIST_ROWS);
                let list_w = eq.right();
                (eq, Rect::new(0.0, player.bottom() + m.gap, list_w, list_h))
            }
            LayoutPreset::C => {
                let eq = Rect::new(0.0, player.bottom() + m.gap, eq_w, eq_h);
                let column_h = eq.bottom();
                (eq, Rect::new(player.right() + m.gap, 0.0, list_w, column_h))
            }
            LayoutPreset::D => {
                let eq = Rect::new(player.right() + m.gap, 0.0, eq_w, player_h);
                (eq, Rect::new(eq.right() + m.gap, 0.0, list_w, player_h))
            }
        };
        let state = |rect| PanelState {
            rect,
            visible: true,
            shaded: false,
        };
        Layout::from_states([state(player), state(equalizer), state(playlist)])
    }

    /// Where `id` is drawn: `None` when hidden, and as tall as the shade bar when shaded.
    pub fn displayed_rect(&self, id: PanelId, m: &Metrics) -> Option<Rect> {
        let state = self.state(id);
        state.visible.then(|| {
            let mut rect = state.rect;
            if state.shaded {
                rect.h = m.shade_height;
            }
            rect
        })
    }

    /// The window size: from (0, 0) to the bottom-right corner of the displayed panels, or
    /// zero size when nothing is displayed.
    pub fn bounds(&self, m: &Metrics) -> Rect {
        let (right, bottom) = PanelId::ALL
            .into_iter()
            .filter_map(|id| self.displayed_rect(id, m))
            .fold((0.0_f32, 0.0_f32), |(r, b), rect| {
                (r.max(rect.right()), b.max(rect.bottom()))
            });
        Rect::new(0.0, 0.0, right, bottom)
    }

    /// Shows or hides `id`. No other panel moves.
    pub fn set_visible(&mut self, id: PanelId, visible: bool) {
        self.panels[id.index()].visible = visible;
    }

    /// Shades or unshades `id`. The panels docked below it, and those docked below them, move
    /// up or down by the change of height. Panels beside it stay.
    pub fn set_shaded(&mut self, id: PanelId, shaded: bool, m: &Metrics) {
        let new_height = if shaded {
            m.shade_height
        } else {
            self.state(id).rect.h
        };
        let moves = self
            .displayed_rect(id, m)
            .map(|old| self.followers(id, (0.0, new_height - old.h), false, m));
        self.panels[id.index()].shaded = shaded;
        if let Some(moves) = moves {
            self.apply_moves(&moves);
        }
    }

    /// Resizes the playlist. The width is at least the minimum, the height is snapped to the
    /// chrome plus a whole number of rows (at least the minimum rows). The panels docked to its
    /// right move by the change of width, those docked below move by the change of height, and
    /// so on transitively.
    pub fn resize_playlist(&mut self, width: f32, height: f32, m: &Metrics) {
        let id = PanelId::Playlist;
        let width = width.max(m.playlist_min_width);
        let height = m.snap_playlist_height(height);
        let shaded = self.state(id).shaded;
        let new_displayed_height = if shaded { m.shade_height } else { height };
        let moves = self.displayed_rect(id, m).map(|old| {
            let change = (width - old.w, new_displayed_height - old.h);
            self.followers(id, change, true, m)
        });
        let rect = &mut self.panels[id.index()].rect;
        rect.w = width;
        rect.h = height;
        if let Some(moves) = moves {
            self.apply_moves(&moves);
        }
    }

    /// Moves `id` towards `(x, y)`, snapping it to the other displayed panels, and returns where
    /// it ends up. See [`Layout::snapped_rect`]. Only `id` moves, its docked group does not.
    pub fn move_panel(&mut self, id: PanelId, x: f32, y: f32, m: &Metrics) -> Rect {
        let rect = self.snapped_rect(id, x, y, m);
        let state = &mut self.panels[id.index()];
        state.rect.x = rect.x;
        state.rect.y = rect.y;
        rect
    }

    /// Where `id` would end up if dragged to `(x, y)`, without moving it. The edges snap
    /// magnetically to the other displayed panels (docking with the gap, and alignment, per axis
    /// within the snap distance), then x and y are clamped to be at least 0. The size is the
    /// displayed size (the shade height when shaded).
    pub fn snapped_rect(&self, id: PanelId, x: f32, y: f32, m: &Metrics) -> Rect {
        let size = self.displayed_rect(id, m).unwrap_or(self.state(id).rect);
        let others: Vec<Rect> = PanelId::ALL
            .into_iter()
            .filter(|other| *other != id)
            .filter_map(|other| self.displayed_rect(other, m))
            .collect();
        let (x, y) = snap(Rect::new(x, y, size.w, size.h), &others, m);
        Rect::new(x.max(0.0), y.max(0.0), size.w, size.h)
    }

    /// The panels connected to `id` through docked edges, `id` included, in the order of
    /// [`PanelId::ALL`]. Two panels are docked when they are one gap apart (within 0.5 px) and
    /// overlap on the other axis. A hidden panel is docked to nothing.
    pub fn docked_group(&self, id: PanelId, m: &Metrics) -> Vec<PanelId> {
        let rects = self.displayed_rects(m);
        let mut group = vec![id];
        let mut next = 0;
        while next < group.len() {
            let Some(from) = rects[group[next].index()] else {
                next += 1;
                continue;
            };
            next += 1;
            for other in PanelId::ALL {
                let docked = rects[other.index()].is_some_and(|to| {
                    docked_below(&from, &to, m)
                        || docked_below(&to, &from, m)
                        || docked_right(&from, &to, m)
                        || docked_right(&to, &from, m)
                });
                if docked && !group.contains(&other) {
                    group.push(other);
                }
            }
        }
        group.sort();
        group
    }

    /// Moves every panel so the displayed ones start at (0, 0). Hidden panels move with them but
    /// never go below 0.
    pub fn compact(&mut self, m: &Metrics) {
        let displayed: Vec<Rect> = PanelId::ALL
            .into_iter()
            .filter_map(|id| self.displayed_rect(id, m))
            .collect();
        let Some(first) = displayed.first() else {
            return;
        };
        let left = displayed.iter().fold(first.x, |min, r| min.min(r.x));
        let top = displayed.iter().fold(first.y, |min, r| min.min(r.y));
        for panel in &mut self.panels {
            let rect = panel.rect.translated(-left, -top);
            panel.rect.x = rect.x.max(0.0);
            panel.rect.y = rect.y.max(0.0);
        }
    }

    fn displayed_rects(&self, m: &Metrics) -> [Option<Rect>; 3] {
        PanelId::ALL.map(|id| self.displayed_rect(id, m))
    }

    /// The panels that follow `source` when its displayed size changes by `change` (width,
    /// height), with how far each moves. Panels docked below it follow by the height change;
    /// with `include_right`, panels docked to its right follow by the width change. Panels
    /// docked to those follow in turn, carrying the same offset. Call it before applying the
    /// change: docking is judged on the current geometry.
    fn followers(
        &self,
        source: PanelId,
        change: (f32, f32),
        include_right: bool,
        m: &Metrics,
    ) -> Vec<(PanelId, f32, f32)> {
        let rects = self.displayed_rects(m);
        let mut found = vec![(source, 0.0, 0.0)];
        let mut next = 0;
        while next < found.len() {
            let (id, dx, dy) = found[next];
            next += 1;
            let Some(from) = rects[id.index()] else {
                continue;
            };
            let (grow_w, grow_h) = if id == source { change } else { (0.0, 0.0) };
            for other in PanelId::ALL {
                let Some(to) = rects[other.index()] else {
                    continue;
                };
                if found.iter().any(|(known, _, _)| *known == other) {
                    continue;
                }
                if docked_below(&from, &to, m) {
                    found.push((other, dx, dy + grow_h));
                } else if include_right && docked_right(&from, &to, m) {
                    found.push((other, dx + grow_w, dy));
                }
            }
        }
        found.remove(0);
        found
    }

    fn apply_moves(&mut self, moves: &[(PanelId, f32, f32)]) {
        for &(id, dx, dy) in moves {
            let rect = &mut self.panels[id.index()].rect;
            *rect = rect.translated(dx, dy);
        }
    }
}

/// Whether `lower` is docked under `upper`: one gap below it and overlapping horizontally.
fn docked_below(upper: &Rect, lower: &Rect, m: &Metrics) -> bool {
    (lower.y - (upper.bottom() + m.gap)).abs() <= TOUCH_TOLERANCE
        && lower.x < upper.right()
        && upper.x < lower.right()
}

/// Whether `right` is docked to the right of `left`: one gap away and overlapping vertically.
fn docked_right(left: &Rect, right: &Rect, m: &Metrics) -> bool {
    (right.x - (left.right() + m.gap)).abs() <= TOUCH_TOLERANCE
        && right.y < left.bottom()
        && left.y < right.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;
    use PanelId::{Equalizer, Player, Playlist};

    fn m() -> Metrics {
        Metrics::default()
    }

    fn preset(preset: LayoutPreset) -> Layout {
        Layout::preset(preset, &m())
    }

    fn rect(layout: &Layout, id: PanelId) -> Rect {
        layout.state(id).rect
    }

    #[test]
    fn preset_a_is_a_column_with_13_playlist_rows() {
        let layout = preset(LayoutPreset::A);
        assert_eq!(rect(&layout, Player), Rect::new(0.0, 0.0, 520.0, 274.0));
        assert_eq!(
            rect(&layout, Equalizer),
            Rect::new(0.0, 280.0, 520.0, 207.0)
        );
        assert_eq!(rect(&layout, Playlist), Rect::new(0.0, 493.0, 520.0, 392.0));
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 520.0, 885.0));
    }

    #[test]
    fn preset_b_is_an_l_shape_with_9_playlist_rows() {
        let layout = preset(LayoutPreset::B);
        assert_eq!(rect(&layout, Player), Rect::new(0.0, 0.0, 520.0, 274.0));
        assert_eq!(
            rect(&layout, Equalizer),
            Rect::new(526.0, 0.0, 520.0, 274.0)
        );
        assert_eq!(
            rect(&layout, Playlist),
            Rect::new(0.0, 280.0, 1046.0, 304.0)
        );
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 1046.0, 584.0));
    }

    #[test]
    fn preset_c_has_the_playlist_beside_the_column() {
        let layout = preset(LayoutPreset::C);
        assert_eq!(rect(&layout, Player), Rect::new(0.0, 0.0, 520.0, 274.0));
        assert_eq!(
            rect(&layout, Equalizer),
            Rect::new(0.0, 280.0, 520.0, 207.0)
        );
        assert_eq!(rect(&layout, Playlist), Rect::new(526.0, 0.0, 600.0, 487.0));
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 1126.0, 487.0));
    }

    #[test]
    fn preset_d_is_a_strip() {
        let layout = preset(LayoutPreset::D);
        assert_eq!(
            rect(&layout, Equalizer),
            Rect::new(526.0, 0.0, 520.0, 274.0)
        );
        assert_eq!(
            rect(&layout, Playlist),
            Rect::new(1052.0, 0.0, 600.0, 274.0)
        );
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 1652.0, 274.0));
    }

    #[test]
    fn presets_are_visible_unshaded_and_fully_docked() {
        for p in LayoutPreset::ALL {
            let layout = preset(p);
            assert!(layout.states().all(|(_, s)| s.visible && !s.shaded));
            assert_eq!(layout.docked_group(Player, &m()), PanelId::ALL, "{p:?}");
        }
    }

    #[test]
    fn presets_follow_the_scale() {
        let scaled = m().scaled(2.0);
        let layout = Layout::preset(LayoutPreset::C, &scaled);
        assert_eq!(layout.bounds(&scaled), Rect::new(0.0, 0.0, 2252.0, 974.0));
    }

    #[test]
    fn displayed_rect_hides_and_shades() {
        let mut layout = preset(LayoutPreset::C);
        assert_eq!(
            layout.displayed_rect(Equalizer, &m()),
            Some(rect(&layout, Equalizer))
        );
        layout.set_shaded(Equalizer, true, &m());
        assert_eq!(
            layout.displayed_rect(Equalizer, &m()),
            Some(Rect::new(0.0, 280.0, 520.0, 24.0))
        );
        assert_eq!(rect(&layout, Equalizer).h, 207.0);
        layout.set_visible(Equalizer, false);
        assert_eq!(layout.displayed_rect(Equalizer, &m()), None);
    }

    #[test]
    fn bounds_ignore_hidden_panels_and_can_be_empty() {
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 520.0, 487.0));
        layout.set_visible(Equalizer, false);
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 520.0, 274.0));
        layout.set_visible(Player, false);
        assert_eq!(layout.bounds(&m()), Rect::default());
    }

    #[test]
    fn set_visible_moves_nothing() {
        let mut layout = preset(LayoutPreset::A);
        let before = layout.clone();
        layout.set_visible(Equalizer, false);
        layout.set_visible(Equalizer, true);
        assert_eq!(layout, before);
        layout.set_visible(Equalizer, false);
        assert_eq!(rect(&layout, Playlist), rect(&before, Playlist));
    }

    #[test]
    fn shading_moves_the_panels_docked_below_and_not_the_ones_beside() {
        // Layout C: the equalizer is below the player, the playlist is beside both.
        let mut layout = preset(LayoutPreset::C);
        layout.set_shaded(Player, true, &m());
        assert_eq!(rect(&layout, Equalizer).y, 30.0);
        assert_eq!(rect(&layout, Playlist), Rect::new(526.0, 0.0, 600.0, 487.0));
        layout.set_shaded(Player, false, &m());
        assert_eq!(layout, preset(LayoutPreset::C));
    }

    #[test]
    fn shading_moves_panels_transitively() {
        let mut layout = preset(LayoutPreset::A);
        layout.set_shaded(Player, true, &m());
        assert_eq!(rect(&layout, Equalizer).y, 30.0);
        assert_eq!(rect(&layout, Playlist).y, 493.0 - 250.0);
        layout.set_shaded(Equalizer, true, &m());
        assert_eq!(rect(&layout, Playlist).y, 493.0 - 250.0 - 183.0);
        assert_eq!(layout.bounds(&m()).h, 30.0 + 24.0 + 6.0 + 392.0);
        layout.set_shaded(Player, false, &m());
        layout.set_shaded(Equalizer, false, &m());
        assert_eq!(layout, preset(LayoutPreset::A));
    }

    #[test]
    fn shading_leaves_undocked_panels_alone() {
        let mut layout = preset(LayoutPreset::A);
        layout.move_panel(Playlist, 700.0, 700.0, &m());
        layout.set_shaded(Player, true, &m());
        assert_eq!(rect(&layout, Playlist).y, 700.0);
        assert_eq!(rect(&layout, Equalizer).y, 30.0);
    }

    #[test]
    fn shading_a_hidden_panel_moves_nothing() {
        let mut layout = preset(LayoutPreset::A);
        layout.set_visible(Player, false);
        layout.set_shaded(Player, true, &m());
        assert_eq!(rect(&layout, Equalizer).y, 280.0);
    }

    #[test]
    fn resizing_the_playlist_snaps_to_rows_and_minimum_width() {
        let mut layout = preset(LayoutPreset::C);
        layout.resize_playlist(700.0, 106.0 + 10.0 * 22.0 + 5.0, &m());
        assert_eq!(rect(&layout, Playlist), Rect::new(526.0, 0.0, 700.0, 326.0));
        layout.resize_playlist(100.0, 0.0, &m());
        assert_eq!(rect(&layout, Playlist), Rect::new(526.0, 0.0, 520.0, 194.0));
    }

    fn state_at(x: f32, y: f32, w: f32, h: f32) -> PanelState {
        PanelState {
            rect: Rect::new(x, y, w, h),
            visible: true,
            shaded: false,
        }
    }

    #[test]
    fn resizing_the_playlist_moves_the_panels_docked_to_its_right() {
        // Player, then the playlist, then the equalizer, side by side.
        let mut layout = Layout::from_states([
            state_at(0.0, 0.0, 520.0, 274.0),
            state_at(1052.0, 0.0, 520.0, 274.0),
            state_at(526.0, 0.0, 520.0, 274.0),
        ]);
        layout.resize_playlist(620.0, 274.0, &m());
        assert_eq!(rect(&layout, Playlist).w, 620.0);
        assert_eq!(rect(&layout, Equalizer).x, 1152.0);
        assert_eq!(rect(&layout, Player).x, 0.0);
    }

    #[test]
    fn resizing_the_playlist_moves_the_panels_docked_below_it() {
        // The playlist on top, the equalizer below it and the player below that.
        let mut layout = Layout::from_states([
            state_at(0.0, 611.0, 520.0, 274.0),
            state_at(0.0, 398.0, 520.0, 207.0),
            state_at(0.0, 0.0, 520.0, 392.0),
        ]);
        layout.resize_playlist(520.0, 392.0 + 44.0, &m());
        assert_eq!(rect(&layout, Playlist).h, 436.0);
        assert_eq!(rect(&layout, Equalizer).y, 442.0);
        assert_eq!(rect(&layout, Player).y, 655.0);
        layout.resize_playlist(520.0, 392.0, &m());
        assert_eq!(rect(&layout, Equalizer).y, 398.0);
        assert_eq!(rect(&layout, Player).y, 611.0);
    }

    #[test]
    fn resizing_a_shaded_playlist_changes_width_but_not_what_is_below() {
        let mut layout = Layout::from_states([
            state_at(0.0, 611.0, 520.0, 274.0),
            state_at(0.0, 30.0, 520.0, 207.0),
            state_at(0.0, 0.0, 520.0, 392.0),
        ]);
        layout.set_shaded(Playlist, true, &m());
        layout.resize_playlist(520.0, 500.0, &m());
        assert_eq!(rect(&layout, Playlist).h, 106.0 + 18.0 * 22.0);
        assert_eq!(rect(&layout, Equalizer).y, 30.0);
    }

    #[test]
    fn resizing_the_last_playlist_moves_nothing_else() {
        let mut layout = preset(LayoutPreset::C);
        layout.resize_playlist(900.0, 600.0, &m());
        assert_eq!(rect(&layout, Player), Rect::new(0.0, 0.0, 520.0, 274.0));
        assert_eq!(
            rect(&layout, Equalizer),
            Rect::new(0.0, 280.0, 520.0, 207.0)
        );
    }

    #[test]
    fn move_panel_snaps_and_applies() {
        let mut layout = preset(LayoutPreset::C);
        let moved = layout.move_panel(Playlist, 1000.0, 400.0, &m());
        assert_eq!(moved, Rect::new(1000.0, 400.0, 600.0, 487.0));
        assert_eq!(rect(&layout, Playlist), moved);
        // Close to the right of the player and top aligned with it: the equalizer is the
        // panel below the player, so use the player's right edge and top.
        let moved = layout.move_panel(Playlist, 530.0, 3.0, &m());
        assert_eq!((moved.x, moved.y), (526.0, 0.0));
    }

    #[test]
    fn move_panel_docks_in_each_direction() {
        let metrics = m();
        // Only the player (0,0 520x274) and the equalizer (520x207) are displayed.
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        let below = layout.snapped_rect(Equalizer, 0.0, 285.0, &metrics);
        assert_eq!((below.x, below.y), (0.0, 280.0));
        let right = layout.snapped_rect(Equalizer, 530.0, 0.0, &metrics);
        assert_eq!((right.x, right.y), (526.0, 0.0));
        // Dragged to the left of the player (which is at x = 0) the clamp wins.
        let left = layout.snapped_rect(Equalizer, -540.0, 0.0, &metrics);
        assert_eq!((left.x, left.y), (0.0, 0.0));
    }

    #[test]
    fn move_panel_docks_to_the_left_and_above_other_panels() {
        let metrics = m();
        // The player sits at 600,300; the equalizer (520x207) is dragged around it.
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        layout.panels[Player.index()].rect = Rect::new(600.0, 300.0, 520.0, 274.0);
        let left = layout.snapped_rect(Equalizer, 70.0, 300.0, &metrics);
        assert_eq!((left.x, left.y), (74.0, 300.0));
        let above = layout.snapped_rect(Equalizer, 600.0, 90.0, &metrics);
        assert_eq!((above.x, above.y), (600.0, 87.0));
    }

    #[test]
    fn move_panel_does_not_snap_beyond_the_distance() {
        let metrics = m();
        let layout = preset(LayoutPreset::C);
        let far = layout.snapped_rect(Equalizer, 500.0, 600.0, &metrics);
        assert_eq!((far.x, far.y), (500.0, 600.0));
    }

    #[test]
    fn move_panel_clamps_at_zero() {
        let mut layout = preset(LayoutPreset::C);
        let moved = layout.move_panel(Playlist, -50.0, -20.0, &m());
        assert_eq!((moved.x, moved.y), (0.0, 0.0));
        assert!(layout.state(Playlist).rect.x >= 0.0);
    }

    #[test]
    fn move_panel_aligns_corners() {
        // Equalizer dragged near the lower right corner of the player: docks right, aligns top.
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        let moved = layout.move_panel(Equalizer, 531.0, 4.0, &m());
        assert_eq!((moved.x, moved.y), (526.0, 0.0));
        // Docked below the player and aligned with its right edge (same width: left edges).
        let moved = layout.move_panel(Equalizer, 4.0, 276.0, &m());
        assert_eq!((moved.x, moved.y), (0.0, 280.0));
    }

    #[test]
    fn move_panel_uses_the_shaded_height() {
        let metrics = m();
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        layout.set_shaded(Equalizer, true, &metrics);
        let moved = layout.move_panel(Equalizer, 600.0, 600.0, &metrics);
        assert_eq!(moved.h, 24.0);
        // The bottom of the shaded bar docks 6 px above a panel placed 100 px lower.
        layout.panels[Player.index()].rect = Rect::new(0.0, 700.0, 520.0, 274.0);
        let moved = layout.move_panel(Equalizer, 0.0, 672.0, &metrics);
        assert_eq!(moved.y, 700.0 - 6.0 - 24.0);
    }

    #[test]
    fn docked_group_follows_docked_edges_transitively() {
        let layout = preset(LayoutPreset::A);
        assert_eq!(layout.docked_group(Playlist, &m()), PanelId::ALL);
        assert_eq!(layout.docked_group(Player, &m()), PanelId::ALL);
    }

    #[test]
    fn docked_group_excludes_detached_and_hidden_panels() {
        let mut layout = preset(LayoutPreset::C);
        layout.move_panel(Playlist, 800.0, 100.0, &m());
        assert_eq!(layout.docked_group(Player, &m()), [Player, Equalizer]);
        assert_eq!(layout.docked_group(Playlist, &m()), [Playlist]);
        layout.set_visible(Equalizer, false);
        assert_eq!(layout.docked_group(Player, &m()), [Player]);
        assert_eq!(layout.docked_group(Equalizer, &m()), [Equalizer]);
    }

    #[test]
    fn docked_group_needs_the_exact_gap_and_overlap() {
        let metrics = m();
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Playlist, false);
        let equalizer = Equalizer.index();
        // Two px too far: no longer docked.
        layout.panels[equalizer].rect = Rect::new(0.0, 282.0, 520.0, 207.0);
        assert_eq!(layout.docked_group(Player, &metrics), [Player]);
        // One gap away horizontally but below the player's bottom: no overlap, not docked.
        layout.panels[equalizer].rect = Rect::new(526.0, 280.0, 10.0, 10.0);
        assert_eq!(layout.docked_group(Player, &metrics), [Player]);
        // Within the 0.5 px tolerance.
        layout.panels[equalizer].rect = Rect::new(0.0, 280.4, 520.0, 207.0);
        assert_eq!(layout.docked_group(Player, &metrics), [Player, Equalizer]);
    }

    #[test]
    fn compact_moves_the_displayed_panels_to_the_origin() {
        let mut layout = preset(LayoutPreset::C);
        layout.set_visible(Player, false);
        layout.set_visible(Equalizer, false);
        layout.compact(&m());
        assert_eq!(rect(&layout, Playlist), Rect::new(0.0, 0.0, 600.0, 487.0));
        assert_eq!(layout.bounds(&m()), Rect::new(0.0, 0.0, 600.0, 487.0));
        assert!(
            layout
                .states()
                .all(|(_, s)| s.rect.x >= 0.0 && s.rect.y >= 0.0)
        );
    }

    #[test]
    fn compact_keeps_relative_positions_and_does_nothing_when_tidy() {
        let mut layout = preset(LayoutPreset::C);
        let before = layout.clone();
        layout.compact(&m());
        assert_eq!(layout, before);
        for id in PanelId::ALL {
            layout.move_panel(
                id,
                rect(&before, id).x + 100.0,
                rect(&before, id).y + 40.0,
                &m(),
            );
        }
        layout.compact(&m());
        assert_eq!(layout, before);
    }

    #[test]
    fn compact_with_nothing_displayed_is_a_no_op() {
        let mut layout = preset(LayoutPreset::C);
        for id in PanelId::ALL {
            layout.set_visible(id, false);
        }
        let before = layout.clone();
        layout.compact(&m());
        assert_eq!(layout, before);
    }
}
