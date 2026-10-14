//! The sizes the layout works with, in logical pixels at 1x.

/// Panel sizes and docking distances. `Default` is the classic 1x set; the app multiplies by
/// its scale with [`Metrics::scaled`] before calling the layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Space between two docked panels.
    pub gap: f32,
    /// How close an edge must be to another to snap to it.
    pub snap_distance: f32,
    /// Height of a shaded (collapsed) panel.
    pub shade_height: f32,
    /// Size of the player, width and height.
    pub player_size: (f32, f32),
    /// Size of the equalizer, width and height.
    pub equalizer_size: (f32, f32),
    /// Default size of the playlist, width and height.
    pub playlist_size: (f32, f32),
    /// The narrowest the playlist can be.
    pub playlist_min_width: f32,
    /// Height of one playlist row.
    pub playlist_row_height: f32,
    /// Height of everything in the playlist panel except the rows (title bar, search, footer).
    pub playlist_chrome_height: f32,
    /// The fewest rows the playlist can show.
    pub playlist_min_rows: u32,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            gap: 6.0,
            snap_distance: 12.0,
            shade_height: 24.0,
            player_size: (520.0, 274.0),
            equalizer_size: (520.0, 207.0),
            playlist_size: (600.0, 487.0),
            playlist_min_width: 520.0,
            playlist_row_height: 22.0,
            playlist_chrome_height: 106.0,
            playlist_min_rows: 4,
        }
    }
}

impl Metrics {
    /// Every length multiplied by `factor`. The minimum row count does not change.
    pub fn scaled(&self, factor: f32) -> Metrics {
        let pair = |(w, h): (f32, f32)| (w * factor, h * factor);
        Metrics {
            gap: self.gap * factor,
            snap_distance: self.snap_distance * factor,
            shade_height: self.shade_height * factor,
            player_size: pair(self.player_size),
            equalizer_size: pair(self.equalizer_size),
            playlist_size: pair(self.playlist_size),
            playlist_min_width: self.playlist_min_width * factor,
            playlist_row_height: self.playlist_row_height * factor,
            playlist_chrome_height: self.playlist_chrome_height * factor,
            playlist_min_rows: self.playlist_min_rows,
        }
    }

    /// The playlist height that shows exactly `rows` rows.
    pub fn playlist_height_for_rows(&self, rows: u32) -> f32 {
        self.playlist_chrome_height + rows as f32 * self.playlist_row_height
    }

    /// `height` snapped to the nearest whole number of rows, with at least the minimum rows.
    pub fn snap_playlist_height(&self, height: f32) -> f32 {
        let rows = ((height - self.playlist_chrome_height) / self.playlist_row_height).round();
        let rows = if rows.is_finite() && rows > 0.0 {
            rows as u32
        } else {
            0
        };
        self.playlist_height_for_rows(rows.max(self.playlist_min_rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let m = Metrics::default();
        assert_eq!((m.gap, m.snap_distance, m.shade_height), (6.0, 12.0, 24.0));
        assert_eq!(m.player_size, (520.0, 274.0));
        assert_eq!(m.equalizer_size, (520.0, 207.0));
        assert_eq!(m.playlist_size, (600.0, 487.0));
        assert_eq!((m.playlist_min_width, m.playlist_row_height), (520.0, 22.0));
        assert_eq!((m.playlist_chrome_height, m.playlist_min_rows), (106.0, 4));
    }

    #[test]
    fn scaled_multiplies_lengths_only() {
        let m = Metrics::default().scaled(2.0);
        assert_eq!(m.gap, 12.0);
        assert_eq!(m.player_size, (1040.0, 548.0));
        assert_eq!(m.playlist_chrome_height, 212.0);
        assert_eq!(m.playlist_min_rows, 4);
        assert_eq!(Metrics::default().scaled(1.0), Metrics::default());
    }

    #[test]
    fn snapping_goes_to_the_nearest_row_with_a_minimum() {
        let m = Metrics::default();
        assert_eq!(m.snap_playlist_height(487.0), 106.0 + 17.0 * 22.0);
        assert_eq!(
            m.snap_playlist_height(106.0 + 5.0 * 22.0 + 12.0),
            106.0 + 6.0 * 22.0
        );
        assert_eq!(m.snap_playlist_height(0.0), 106.0 + 4.0 * 22.0);
        assert_eq!(m.snap_playlist_height(f32::NAN), 106.0 + 4.0 * 22.0);
        assert_eq!(m.playlist_height_for_rows(13), 392.0);
    }
}
