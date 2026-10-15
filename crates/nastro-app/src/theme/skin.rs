//! The active skin as gpui colors, plus the skin scale. Converted once per skin change from
//! `nastro_skin::Skin`, stored as a gpui global and read by every skinned view through
//! [`SkinExt::skin`]. Views never write a skin color as a literal.

use std::rc::Rc;

use gpui_kit::{
    App, Background, BoxShadow, Edges, Global, Hsla, Pixels, Rgba, SharedString, linear_color_stop,
    linear_gradient, point, px,
};

use nastro_skin::{Color, Skin};

/// One [`Hsla`] per skin token, named like the `skin.toml` keys (`docs/skin-format.md`). Every
/// token of the format is converted, including the few gpui cannot draw yet (`lcd_glow`, a text
/// glow) or the views do not use yet (`focus`, `text_disabled`), so a view can start using one
/// without touching the conversion.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SkinColors {
    pub frame_hi: Hsla,
    pub frame_mid: Hsla,
    pub frame_lo: Hsla,
    /// Opacity of the brushed metal stripes.
    pub frame_brush: f32,
    pub frame_edge: Hsla,
    pub bevel_hi: Hsla,
    pub groove: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_disabled: Hsla,
    pub control_hi: Hsla,
    pub control_lo: Hsla,
    pub control_hover_hi: Hsla,
    pub control_hover_lo: Hsla,
    pub slot: Hsla,
    pub field_bg: Hsla,
    pub focus: Hsla,
    pub lcd_top: Hsla,
    pub lcd_bottom: Hsla,
    pub lcd_edge: Hsla,
    pub lcd_fg: Hsla,
    pub lcd_dim: Hsla,
    pub lcd_glow: Hsla,
    pub lcd_rule: Hsla,
    pub vis_bar: Hsla,
    pub vis_osc: Hsla,
    pub vis_peak: Hsla,
    pub accent: Hsla,
    pub accent_ink: Hsla,
    pub led_off: Hsla,
    pub pl_bg: Hsla,
    pub pl_fg: Hsla,
    pub pl_current: Hsla,
    pub pl_selected_bg: Hsla,
    pub pl_index: Hsla,
    pub pl_hover: Hsla,
}

/// The active skin: its colors, fonts and scale. Shared as an `Rc`, so a view clones it once per
/// render for the cost of a counter.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SkinTheme {
    /// The skin's id, as stored in the settings.
    pub id: SharedString,
    /// The skin's display name.
    pub name: SharedString,
    /// Every color token.
    pub colors: SkinColors,
    /// The UI font family (labels, buttons, playlist).
    pub font_ui: SharedString,
    /// The numbers font family (marquee, durations, equalizer scale).
    pub font_mono: SharedString,
    /// The display font family (clock, badges).
    pub font_lcd: SharedString,
    /// The skin scale factor: 1, 1.5 or 2.
    pub scale: f32,
}

impl SkinTheme {
    /// Builds the theme of `skin` drawn at `scale`.
    pub fn new(skin: &Skin, scale: f32) -> Self {
        let c = &skin.colors;
        let p = &skin.playlist;
        SkinTheme {
            id: skin.id.clone().into(),
            name: skin.name.clone().into(),
            colors: SkinColors {
                frame_hi: hsla(c.frame_hi),
                frame_mid: hsla(c.frame_mid),
                frame_lo: hsla(c.frame_lo),
                frame_brush: c.frame_brush,
                frame_edge: hsla(c.frame_edge),
                bevel_hi: hsla(c.bevel_hi),
                groove: hsla(c.groove),
                text: hsla(c.text),
                text_muted: hsla(c.text_muted),
                text_disabled: hsla(c.text_disabled()),
                control_hi: hsla(c.control_hi),
                control_lo: hsla(c.control_lo),
                control_hover_hi: hsla(c.control_hover_hi),
                control_hover_lo: hsla(c.control_hover_lo),
                slot: hsla(c.slot),
                field_bg: hsla(c.field_bg),
                focus: hsla(c.focus),
                lcd_top: hsla(c.lcd_top),
                lcd_bottom: hsla(c.lcd_bottom),
                lcd_edge: hsla(c.lcd_edge),
                lcd_fg: hsla(c.lcd_fg),
                lcd_dim: hsla(c.lcd_dim),
                lcd_glow: hsla(c.lcd_glow),
                lcd_rule: hsla(c.lcd_rule),
                vis_bar: hsla(c.vis_bar),
                vis_osc: hsla(c.vis_osc),
                vis_peak: hsla(c.vis_peak),
                accent: hsla(c.accent),
                accent_ink: hsla(c.accent_ink),
                led_off: hsla(c.led_off),
                pl_bg: hsla(p.bg),
                pl_fg: hsla(p.fg),
                pl_current: hsla(p.current),
                pl_selected_bg: hsla(p.selected_bg),
                pl_index: hsla(p.index),
                pl_hover: hsla(c.playlist_hover()),
            },
            font_ui: skin.fonts.ui.clone().into(),
            font_mono: skin.fonts.mono.clone().into(),
            font_lcd: skin.fonts.lcd.clone().into(),
            scale,
        }
    }

    /// A design size scaled by the skin scale. Every skin metric goes through here.
    pub fn px(&self, size: f32) -> Pixels {
        px(size * self.scale)
    }

    /// The raised control gradient: `control_hi` to `control_lo`, the hover pair on hover, and
    /// inverted while pressed.
    pub fn control_bg(&self, hover: bool, pressed: bool) -> Background {
        let c = &self.colors;
        let (hi, lo) = if hover {
            (c.control_hover_hi, c.control_hover_lo)
        } else {
            (c.control_hi, c.control_lo)
        };
        let (top, bottom) = if pressed { (lo, hi) } else { (hi, lo) };
        vertical_gradient(top, bottom)
    }

    /// The display's gradient, `lcd_top` to `lcd_bottom`.
    pub fn lcd_bg(&self) -> Background {
        vertical_gradient(self.colors.lcd_top, self.colors.lcd_bottom)
    }

    /// The shadow of a raised control: the 1 px `bevel_hi` highlight inside its top edge and a
    /// soft drop shadow.
    pub fn control_shadow(&self, pressed: bool) -> Vec<BoxShadow> {
        if pressed {
            return vec![self.inset(0.0, 2.0, 4.0, black(0.35))];
        }
        vec![
            self.inset(0.0, 1.0, 0.0, self.colors.bevel_hi),
            self.drop(1.0, 2.0, black(0.25)),
        ]
    }

    /// The shadow of a flat control (window buttons, LED toggles, text buttons): only the bevel.
    pub fn bevel_shadow(&self, pressed: bool) -> Vec<BoxShadow> {
        if pressed {
            return vec![self.inset(0.0, 2.0, 4.0, black(0.35))];
        }
        vec![self.inset(0.0, 1.0, 0.0, self.colors.bevel_hi)]
    }

    /// The recessed shadow of the display and the equalizer curve.
    pub fn lcd_shadow(&self, blur: f32) -> Vec<BoxShadow> {
        vec![
            self.inset(0.0, 2.0, blur, black(0.6)),
            self.drop(1.0, 0.0, self.colors.bevel_hi),
        ]
    }

    /// The recessed shadow of the track list.
    pub fn list_shadow(&self) -> Vec<BoxShadow> {
        vec![
            self.inset(0.0, 2.0, 6.0, black(0.35)),
            self.drop(1.0, 0.0, self.colors.bevel_hi),
        ]
    }

    /// The recessed shadow of a slider track.
    pub fn slot_shadow(&self) -> Vec<BoxShadow> {
        vec![
            self.inset(0.0, 1.0, 2.0, black(0.5)),
            self.drop(1.0, 0.0, self.colors.bevel_hi),
        ]
    }

    /// The shadow of a whole panel: the bevel inside its top edge and the window drop shadow.
    pub fn panel_shadow(&self) -> Vec<BoxShadow> {
        vec![
            self.inset(0.0, 1.0, 0.0, self.colors.bevel_hi),
            self.drop(PANEL_DROP_Y, PANEL_DROP_BLUR, black(0.35)),
        ]
    }

    /// How far the panel drop shadow reaches past each side of a panel. gpui draws a shadow up
    /// to three standard deviations (one and a half CSS blurs) around its offset box.
    pub fn panel_shadow_reach(&self) -> Edges<Pixels> {
        let reach = 1.5 * PANEL_DROP_BLUR;
        Edges {
            top: self.px(reach - PANEL_DROP_Y),
            right: self.px(reach),
            bottom: self.px(reach + PANEL_DROP_Y),
            left: self.px(reach),
        }
    }

    /// The glow of a lit LED.
    pub fn led_glow(&self) -> Vec<BoxShadow> {
        vec![BoxShadow::new(px(0.0), px(0.0), self.colors.accent).blur_radius(self.px(3.0))]
    }

    /// An inset shadow from CSS values (the blur is halved: gpui's blur is the gaussian's
    /// standard deviation, `docs/gpui-notes.md`).
    fn inset(&self, x: f32, y: f32, blur: f32, color: Hsla) -> BoxShadow {
        let mut shadow =
            BoxShadow::new(self.px(x), self.px(y), color).blur_radius(self.px(blur / 2.0));
        shadow.inset = true;
        shadow
    }

    /// A drop shadow from CSS values, without horizontal offset.
    fn drop(&self, y: f32, blur: f32, color: Hsla) -> BoxShadow {
        BoxShadow {
            color,
            offset: point(px(0.0), self.px(y)),
            blur_radius: self.px(blur / 2.0),
            spread_radius: px(0.0),
            inset: false,
        }
    }
}

/// The active [`SkinTheme`], a gpui global.
pub struct ActiveSkin(pub Rc<SkinTheme>);

impl Global for ActiveSkin {}

/// Gives `App` (and `Context<T>` through deref) access to the active skin.
pub trait SkinExt {
    /// The active skin. Clone the `Rc` to keep it across `cx` borrows.
    fn skin(&self) -> Rc<SkinTheme>;
}

impl SkinExt for App {
    fn skin(&self) -> Rc<SkinTheme> {
        match self.try_global::<ActiveSkin>() {
            Some(active) => active.0.clone(),
            // `theme::install` sets the global before any window opens, so this only happens in
            // code that runs without a window; the default skin keeps it working.
            None => Rc::new(SkinTheme::new(&nastro_skin::default_skin(), 1.0)),
        }
    }
}

/// Makes `theme` the active skin. The caller refreshes the windows.
pub fn set_active(cx: &mut App, theme: SkinTheme) {
    cx.set_global(ActiveSkin(Rc::new(theme)));
}

/// The panel drop shadow, CSS values: offset y and blur. It has to end inside the window margin
/// (`WINDOW_MARGIN`): the window edge would cut it in a straight line around the whole group.
const PANEL_DROP_Y: f32 = 3.0;
const PANEL_DROP_BLUR: f32 = 8.0;

/// A top to bottom gradient between two colors.
pub fn vertical_gradient(top: Hsla, bottom: Hsla) -> Background {
    linear_gradient(
        180.0,
        linear_color_stop(top, 0.0),
        linear_color_stop(bottom, 1.0),
    )
}

/// Black at `alpha`, for the shadows of the design, which are the same in every skin.
pub fn black(alpha: f32) -> Hsla {
    Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: alpha,
    }
}

/// White at `alpha`, for the brushed metal stripes.
pub fn white(alpha: f32) -> Hsla {
    Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: alpha,
    }
}

/// A skin [`Color`] as gpui's [`Hsla`].
pub fn hsla(color: Color) -> Hsla {
    Rgba {
        r: f32::from(color.r) / 255.0,
        g: f32::from(color.g) / 255.0,
        b: f32::from(color.b) / 255.0,
        a: f32::from(color.a) / 255.0,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_multiplies_every_metric() {
        let theme = SkinTheme::new(&nastro_skin::default_skin(), 1.5);
        assert_eq!(theme.px(24.0), px(36.0));
    }

    #[test]
    fn panel_shadow_ends_inside_the_window_margin() {
        let theme = SkinTheme::new(&nastro_skin::default_skin(), 2.0);
        let reach = theme.panel_shadow_reach();
        let margin = theme.px(crate::theme::metrics::WINDOW_MARGIN);
        for side in [reach.top, reach.right, reach.bottom, reach.left] {
            assert!(side <= margin, "{side:?} past the {margin:?} margin");
        }
    }

    #[test]
    fn colors_keep_their_alpha() {
        let color = hsla(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 0x80,
        });
        assert!((color.a - 0.502).abs() < 0.01);
    }
}
