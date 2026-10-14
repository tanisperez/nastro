//! The skin model.

use std::path::PathBuf;

use crate::color::Color;

/// A complete skin: everything the app needs to paint the player, equalizer and playlist.
#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
    /// Stable identifier: the file stem, or the folder name for a skin folder.
    pub id: String,
    /// Display name, from `[skin] name`.
    pub name: String,
    /// The skin format version (1).
    pub format: u32,
    /// Font family names.
    pub fonts: SkinFonts,
    /// The color tokens.
    pub colors: SkinColors,
    /// The playlist colors.
    pub playlist: PlaylistColors,
    /// Where the skin came from.
    pub origin: SkinOrigin,
}

/// Where a skin was loaded from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkinOrigin {
    /// Embedded in the binary.
    BuiltIn,
    /// A file on disk (the `.toml` file or the folder's `skin.toml`).
    File(PathBuf),
}

/// Font family names used by a skin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkinFonts {
    /// Proportional UI font.
    pub ui: String,
    /// Monospace font.
    pub mono: String,
    /// Font of the LCD (clock, marquee, badges).
    pub lcd: String,
}

impl Default for SkinFonts {
    fn default() -> Self {
        Self {
            ui: "Geist".to_owned(),
            mono: "Geist Mono".to_owned(),
            lcd: "Share Tech Mono".to_owned(),
        }
    }
}

/// The color tokens of a skin. Translucent tokens carry their alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinColors {
    /// Metal, top.
    pub frame_hi: Color,
    /// Metal, middle (at 40 %).
    pub frame_mid: Color,
    /// Metal, bottom.
    pub frame_lo: Color,
    /// Brushed texture opacity, 0.0 to 0.2.
    pub frame_brush: f32,
    /// Panel and control border.
    pub frame_edge: Color,
    /// 1 px top highlight.
    pub bevel_hi: Color,
    /// Engraved grooves.
    pub groove: Color,
    /// Text and icons on metal.
    pub text: Color,
    /// Secondary labels, inactive title.
    pub text_muted: Color,
    /// Button, top.
    pub control_hi: Color,
    /// Button, bottom.
    pub control_lo: Color,
    /// Button hover, top.
    pub control_hover_hi: Color,
    /// Button hover, bottom.
    pub control_hover_lo: Color,
    /// Slider track.
    pub slot: Color,
    /// Search field background.
    pub field_bg: Color,
    /// Focus ring.
    pub focus: Color,
    /// LCD, top.
    pub lcd_top: Color,
    /// LCD, bottom.
    pub lcd_bottom: Color,
    /// LCD recessed border.
    pub lcd_edge: Color,
    /// LCD clock, marquee and badges.
    pub lcd_fg: Color,
    /// LCD badge borders, EQ curve when off.
    pub lcd_dim: Color,
    /// LCD clock glow.
    pub lcd_glow: Color,
    /// Rule above the marquee (`lcd_fg` at 16 %).
    pub lcd_rule: Color,
    /// Spectrum bars.
    pub vis_bar: Color,
    /// Oscilloscope.
    pub vis_osc: Color,
    /// Spectrum peaks.
    pub vis_peak: Color,
    /// LED on, progress, volume fill.
    pub accent: Color,
    /// Active icon on metal.
    pub accent_ink: Color,
    /// LED off.
    pub led_off: Color,
}

impl SkinColors {
    /// Disabled text: `text` at 45 % alpha.
    pub fn text_disabled(&self) -> Color {
        self.text.with_alpha(0.45)
    }

    /// Playlist row hover: white at 7 % alpha, painted over the row.
    pub fn playlist_hover(&self) -> Color {
        Color::rgb(255, 255, 255).with_alpha(0.07)
    }

    /// Pressed control gradient `(top, bottom)`: the control gradient inverted.
    pub fn control_pressed(&self) -> (Color, Color) {
        (self.control_lo, self.control_hi)
    }
}

/// The playlist colors, the same fields as `pledit.txt` in a classic skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaylistColors {
    /// Background (`NormalBG`).
    pub bg: Color,
    /// Track title (`Normal`).
    pub fg: Color,
    /// Playing track and active tab (`Current`).
    pub current: Color,
    /// Selected row background (`SelectedBG`).
    pub selected_bg: Color,
    /// Index and duration.
    pub index: Color,
}
