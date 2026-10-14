//! The app chrome palette: menus, Preferences and About. It is Postino's design system, the same
//! tokens for light and dark, so both apps look like siblings outside their main window. The
//! skin (player, equalizer, playlist) never reads this: its colors come from `theme::skin`.
//!
//! Every hex literal is written once, in [`LIGHT`] and [`DARK`]. [`Palette`] (read by views) and
//! the gpui-kit theme JSON (built by `theme::mod` through [`theme_colors`]) both derive from them.

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, BoxShadow, Hsla, Rgba, px};

/// One shadow layer, CSS values: offset y, blur, spread, and the color as 8-bit RGB plus alpha.
type ShadowLayer = (f32, f32, f32, (u8, u8, u8, f32));

/// The raw values of one mode.
struct Tokens {
    bg: &'static str,
    surface: &'static str,
    raised: &'static str,
    overlay: &'static str,
    border: &'static str,
    border_strong: &'static str,
    fg: &'static str,
    fg_muted: &'static str,
    fg_subtle: &'static str,
    accent: &'static str,
    accent_fg: &'static str,
    accent_text: &'static str,
    accent_subtle: &'static str,
    hover: &'static str,
    warning: &'static str,
    danger: &'static str,
    /// The two shadow layers of menus and windows, CSS values: offset y, blur, spread, color.
    shadow: [ShadowLayer; 2],
}

const LIGHT: Tokens = Tokens {
    bg: "#fcfcfd",
    surface: "#f4f5f7",
    raised: "#ffffff",
    overlay: "#ffffff",
    border: "#e3e5ea",
    border_strong: "#cdd0d8",
    fg: "#1b1c22",
    fg_muted: "#5c606e",
    fg_subtle: "#8b8e9a",
    accent: "#4f57d8",
    accent_fg: "#ffffff",
    accent_text: "#3f47c4",
    accent_subtle: "#e8e9fd",
    hover: "#eceef2",
    warning: "#9a6500",
    danger: "#cf3535",
    shadow: [
        (12.0, 32.0, 0.0, (20, 22, 40, 0.14)),
        (0.0, 0.0, 1.0, (20, 22, 40, 0.06)),
    ],
};

const DARK: Tokens = Tokens {
    bg: "#15161a",
    surface: "#1b1c21",
    raised: "#23252c",
    overlay: "#262830",
    border: "#2c2e36",
    border_strong: "#3b3e48",
    fg: "#e9eaee",
    fg_muted: "#a0a3ae",
    fg_subtle: "#6f7280",
    accent: "#5b63e6",
    accent_fg: "#ffffff",
    accent_text: "#a3a9ff",
    accent_subtle: "#272b4d",
    hover: "#25272e",
    warning: "#e8b75a",
    danger: "#f27171",
    shadow: [
        (12.0, 32.0, 0.0, (0, 0, 0, 0.45)),
        (0.0, 0.0, 1.0, (255, 255, 255, 0.04)),
    ],
};

/// One [`Hsla`] per chrome token. Views read chrome colors only from here, through
/// [`PaletteExt::palette`]. The token set is Postino's whole chrome design system, kept complete
/// even where Nastro's few chrome views do not use a token yet.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Palette {
    /// Window content background.
    pub bg: Hsla,
    /// Title bar and side navigation background.
    pub surface: Hsla,
    /// Inputs and cards.
    pub raised: Hsla,
    /// Menus and popovers.
    pub overlay: Hsla,
    /// Default separator and border.
    pub border: Hsla,
    /// Control borders.
    pub border_strong: Hsla,
    /// Primary text.
    pub fg: Hsla,
    /// Secondary text.
    pub fg_muted: Hsla,
    /// Hints, placeholders, icons.
    pub fg_subtle: Hsla,
    /// Primary button, selected item, focus.
    pub accent: Hsla,
    /// Text on `accent`.
    pub accent_fg: Hsla,
    /// Links and the selected item's label.
    pub accent_text: Hsla,
    /// Selected item background.
    pub accent_subtle: Hsla,
    /// Hover background.
    pub hover: Hsla,
    /// Warnings.
    pub warning: Hsla,
    /// Errors.
    pub danger: Hsla,
    /// The elevation shadow of menus and popovers.
    pub shadow: Vec<BoxShadow>,
}

impl Palette {
    /// The light chrome palette.
    pub fn light() -> Self {
        Self::from_tokens(&LIGHT)
    }

    /// The dark chrome palette.
    pub fn dark() -> Self {
        Self::from_tokens(&DARK)
    }

    fn from_tokens(tokens: &Tokens) -> Self {
        Palette {
            bg: hex(tokens.bg),
            surface: hex(tokens.surface),
            raised: hex(tokens.raised),
            overlay: hex(tokens.overlay),
            border: hex(tokens.border),
            border_strong: hex(tokens.border_strong),
            fg: hex(tokens.fg),
            fg_muted: hex(tokens.fg_muted),
            fg_subtle: hex(tokens.fg_subtle),
            accent: hex(tokens.accent),
            accent_fg: hex(tokens.accent_fg),
            accent_text: hex(tokens.accent_text),
            accent_subtle: hex(tokens.accent_subtle),
            hover: hex(tokens.hover),
            warning: hex(tokens.warning),
            danger: hex(tokens.danger),
            shadow: tokens.shadow.iter().map(shadow_layer).collect(),
        }
    }
}

/// Gives `App` (and `Context<T>` through deref) access to the chrome palette of the active mode.
pub trait PaletteExt {
    /// The light or dark [`Palette`], matching `cx.theme().is_dark()`.
    fn palette(&self) -> Palette;
}

impl PaletteExt for App {
    fn palette(&self) -> Palette {
        if self.theme().is_dark() {
            Palette::dark()
        } else {
            Palette::light()
        }
    }
}

/// The `"colors"` JSON of one mode's gpui-kit `ThemeConfig`, for the stock widgets the chrome
/// borrows (popup menus, buttons, selects, inputs, the scrollbar). The mapping follows Postino's
/// (`docs/gpui-notes.md`, "Mapping a token to a theme field").
pub(super) fn theme_colors(dark: bool) -> serde_json::Value {
    let tokens = if dark { &DARK } else { &LIGHT };
    serde_json::json!({
        "background": tokens.bg,
        "sidebar.background": tokens.surface,
        "title_bar.background": tokens.surface,
        "tab_bar.background": tokens.surface,
        "secondary.background": tokens.raised,
        "popover.background": tokens.overlay,
        "overlay": "#00000000",
        "border": tokens.border,
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": tokens.border_strong,
        "scrollbar.thumb.hover.background": tokens.fg_subtle,
        "input.border": tokens.border_strong,
        "foreground": tokens.fg,
        "muted.foreground": tokens.fg_muted,
        "primary.background": tokens.accent,
        "primary.foreground": tokens.accent_fg,
        "link": tokens.accent_text,
        "accent.background": tokens.accent_subtle,
        "accent.foreground": tokens.fg,
        "selection.background": format!("{}4d", tokens.accent),
        "list.active.background": tokens.accent_subtle,
        "list.hover.background": tokens.hover,
        "button.hover.background": tokens.hover,
        "table.hover.background": tokens.hover,
        "warning.background": tokens.warning,
        "danger.background": tokens.danger,
        "button.primary.background": tokens.accent,
        "button.primary.foreground": tokens.accent_fg,
        "button.secondary.background": tokens.raised,
        "button.secondary.foreground": tokens.fg,
        "button.secondary.hover.background": tokens.hover,
        "switch.background": tokens.border_strong,
        "switch.thumb.background": tokens.accent_fg,
    })
}

/// A CSS shadow layer as a gpui [`BoxShadow`]. gpui's `blur_radius` is the gaussian's standard
/// deviation, half of a CSS blur, so the blur is halved.
fn shadow_layer(layer: &ShadowLayer) -> BoxShadow {
    let (offset_y, blur, spread, (r, g, b, a)) = *layer;
    BoxShadow::new(px(0.0), px(offset_y), rgba(r, g, b, a))
        .blur_radius(px(blur / 2.0))
        .spread_radius(px(spread))
}

/// Parses a `#rrggbb` or `#rrggbbaa` literal of this file. A malformed literal (impossible for the
/// constants above) falls back to opaque black instead of panicking.
fn hex(value: &str) -> Hsla {
    let digits = value.trim_start_matches('#');
    let parsed = u32::from_str_radix(digits, 16).unwrap_or(0);
    let (rgb, alpha) = if digits.len() == 8 {
        (parsed >> 8, (parsed & 0xff) as f32 / 255.0)
    } else {
        (parsed, 1.0)
    };
    rgba(
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
        alpha,
    )
}

/// An [`Hsla`] from 8-bit channels and a 0..1 alpha.
fn rgba(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn to_hex(color: Hsla) -> String {
        let rgba = Rgba::from(color);
        format!(
            "#{:02x}{:02x}{:02x}",
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8,
        )
    }

    #[test]
    fn palettes_match_the_tokens() {
        assert_eq!(to_hex(Palette::light().bg), "#fcfcfd");
        assert_eq!(to_hex(Palette::light().accent), "#4f57d8");
        assert_eq!(to_hex(Palette::dark().bg), "#15161a");
        assert_eq!(to_hex(Palette::dark().accent), "#5b63e6");
    }

    #[test]
    fn theme_colors_use_the_same_tokens() {
        assert_eq!(theme_colors(false)["background"], "#fcfcfd");
        assert_eq!(theme_colors(true)["popover.background"], "#262830");
    }
}
