//! The three panels, their state and the layout presets.

use crate::rect::Rect;

/// One of the three panels docked in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PanelId {
    /// The player with the display and the transport buttons.
    Player,
    /// The 10-band equalizer.
    Equalizer,
    /// The playlist.
    Playlist,
}

impl PanelId {
    /// Every panel, in a fixed order.
    pub const ALL: [PanelId; 3] = [PanelId::Player, PanelId::Equalizer, PanelId::Playlist];

    /// Position in [`PanelId::ALL`].
    pub(crate) fn index(self) -> usize {
        match self {
            PanelId::Player => 0,
            PanelId::Equalizer => 1,
            PanelId::Playlist => 2,
        }
    }
}

/// Where a panel is and how it is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelState {
    /// Position and full, unshaded size, in window coordinates.
    pub rect: Rect,
    /// Whether the panel is shown at all.
    pub visible: bool,
    /// Whether the panel is collapsed to its bar.
    pub shaded: bool,
}

/// The arrangements the user can pick, `C` by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LayoutPreset {
    /// A column: player, equalizer, playlist.
    A,
    /// An L shape: player and equalizer side by side, the playlist below both.
    B,
    /// The player over the equalizer, the playlist to their right.
    #[default]
    C,
    /// A strip: the three panels side by side.
    D,
}

impl LayoutPreset {
    /// Every preset.
    pub const ALL: [LayoutPreset; 4] = [
        LayoutPreset::A,
        LayoutPreset::B,
        LayoutPreset::C,
        LayoutPreset::D,
    ];

    /// The lowercase letter stored in the settings file.
    pub fn as_str(self) -> &'static str {
        match self {
            LayoutPreset::A => "a",
            LayoutPreset::B => "b",
            LayoutPreset::C => "c",
            LayoutPreset::D => "d",
        }
    }

    /// The preset for "a" to "d", in any case.
    pub fn parse(text: &str) -> Option<LayoutPreset> {
        LayoutPreset::ALL
            .into_iter()
            .find(|preset| preset.as_str().eq_ignore_ascii_case(text.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_is_c() {
        assert_eq!(LayoutPreset::default(), LayoutPreset::C);
    }

    #[test]
    fn presets_round_trip_through_text() {
        for preset in LayoutPreset::ALL {
            assert_eq!(LayoutPreset::parse(preset.as_str()), Some(preset));
        }
        assert_eq!(LayoutPreset::parse("B"), Some(LayoutPreset::B));
        assert_eq!(LayoutPreset::parse(" d "), Some(LayoutPreset::D));
        assert_eq!(LayoutPreset::parse("e"), None);
        assert_eq!(LayoutPreset::parse(""), None);
        assert_eq!(LayoutPreset::parse("ab"), None);
    }

    #[test]
    fn panel_indexes_match_the_order_of_all() {
        for (i, id) in PanelId::ALL.into_iter().enumerate() {
            assert_eq!(id.index(), i);
        }
    }
}
