//! The four built-in skins, embedded in the binary.

use crate::color::Color;
use crate::parse::parse_with;
use crate::skin::{PlaylistColors, Skin, SkinColors, SkinFonts, SkinOrigin};

/// Id of the skin used when nothing else is chosen.
pub const DEFAULT_SKIN_ID: &str = "silver";

pub(crate) const SILVER_TOML: &str = include_str!("../skins/silver.toml");

const BUILTIN: [(&str, &str); 4] = [
    ("silver", SILVER_TOML),
    ("graphite", include_str!("../skins/graphite.toml")),
    ("porcelain", include_str!("../skins/porcelain.toml")),
    ("nebula", include_str!("../skins/nebula.toml")),
];

/// The built-in skins in menu order: Silver, Graphite, Porcelain, Nebula.
pub fn builtin_skins() -> Vec<Skin> {
    BUILTIN
        .iter()
        .filter_map(|(id, text)| load(id, text))
        .collect()
}

/// The built-in skin with this id, if any.
pub fn builtin_skin(id: &str) -> Option<Skin> {
    BUILTIN
        .iter()
        .find(|(name, _)| *name == id)
        .and_then(|(id, text)| load(id, text))
}

/// The default skin (Silver).
pub fn default_skin() -> Skin {
    silver_base()
}

/// Silver, the source of every missing token in a user skin.
pub(crate) fn silver_base() -> Skin {
    let blank = blank();
    match parse_with(DEFAULT_SKIN_ID, SILVER_TOML, &blank) {
        Ok(parsed) => parsed.skin,
        Err(err) => {
            // The file is embedded and covered by tests, so this is unreachable in practice.
            log::error!("embedded skin `silver` is invalid: {err}");
            blank
        }
    }
}

fn load(id: &str, text: &str) -> Option<Skin> {
    match parse_with(id, text, &blank()) {
        Ok(parsed) => Some(parsed.skin),
        Err(err) => {
            log::error!("embedded skin `{id}` is invalid: {err}");
            None
        }
    }
}

/// Placeholder base that the embedded files overwrite completely.
fn blank() -> Skin {
    let c = Color::rgb(0, 0, 0);
    Skin {
        id: DEFAULT_SKIN_ID.to_owned(),
        name: "Silver".to_owned(),
        format: 1,
        fonts: SkinFonts::default(),
        colors: SkinColors {
            frame_hi: c,
            frame_mid: c,
            frame_lo: c,
            frame_brush: 0.0,
            frame_edge: c,
            bevel_hi: c,
            groove: c,
            text: c,
            text_muted: c,
            control_hi: c,
            control_lo: c,
            control_hover_hi: c,
            control_hover_lo: c,
            slot: c,
            field_bg: c,
            focus: c,
            lcd_top: c,
            lcd_bottom: c,
            lcd_edge: c,
            lcd_fg: c,
            lcd_dim: c,
            lcd_glow: c,
            lcd_rule: c,
            vis_bar: c,
            vis_osc: c,
            vis_peak: c,
            accent: c,
            accent_ink: c,
            led_off: c,
        },
        playlist: PlaylistColors {
            bg: c,
            fg: c,
            current: c,
            selected_bg: c,
            index: c,
        },
        origin: SkinOrigin::BuiltIn,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::parse::parse_skin;

    use super::*;

    #[test]
    fn four_skins_in_order() {
        let ids: Vec<_> = builtin_skins().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["silver", "graphite", "porcelain", "nebula"]);
        let names: Vec<_> = builtin_skins().into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Silver", "Graphite", "Porcelain", "Nebula"]);
    }

    #[test]
    fn parse_with_zero_warnings() {
        for (id, text) in BUILTIN {
            let parsed = parse_skin(id, text).unwrap();
            assert!(parsed.warnings.is_empty(), "{id}: {:?}", parsed.warnings);
        }
    }

    #[test]
    fn lookup_and_default() {
        assert_eq!(builtin_skin("nebula").unwrap().name, "Nebula");
        assert!(builtin_skin("nope").is_none());
        assert_eq!(default_skin().id, DEFAULT_SKIN_ID);
        assert_eq!(default_skin().origin, SkinOrigin::BuiltIn);
    }

    #[test]
    fn lcd_rule_is_lcd_fg_at_16_percent() {
        for skin in builtin_skins() {
            let c = skin.colors;
            assert_eq!(
                c.lcd_rule,
                Color {
                    a: 0x29,
                    ..c.lcd_fg
                },
                "{}",
                skin.id
            );
        }
    }

    #[test]
    fn silver_matches_the_handoff_values() {
        let c = default_skin().colors;
        assert_eq!(c.frame_hi.to_hex(), "#f3f5f7");
        assert_eq!(c.bevel_hi.to_hex(), "#ffffffd9");
        assert_eq!(c.lcd_glow.to_hex(), "#78d2ff73");
        assert_eq!(c.frame_brush, 0.05);
        assert_eq!(default_skin().playlist.selected_bg.to_hex(), "#2f6aa5");
    }
}
