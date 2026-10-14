//! `skin.toml` parsing.

use toml::{Table, Value};

use crate::builtin::silver_base;
use crate::color::Color;
use crate::error::SkinError;
use crate::skin::{Skin, SkinOrigin};

/// A parsed skin and the non-fatal problems found while reading it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedSkin {
    /// The skin, with any missing token filled from Silver.
    pub skin: Skin,
    /// Missing tokens, unknown keys and unknown sections.
    pub warnings: Vec<String>,
}

/// Parses a `skin.toml` text. `id` becomes [`Skin::id`].
///
/// Missing tokens are taken from the Silver skin with a warning, unknown keys only warn. A
/// malformed color, a wrong type, a `format` other than 1, a missing `[skin] name` or a
/// `frame_brush` outside 0..=0.2 is an error.
pub fn parse_skin(id: &str, text: &str) -> Result<ParsedSkin, SkinError> {
    parse_with(id, text, &silver_base())
}

/// Parses `text`, taking missing values from `base`.
pub(crate) fn parse_with(id: &str, text: &str, base: &Skin) -> Result<ParsedSkin, SkinError> {
    let root: Table = text
        .parse()
        .map_err(|e: toml::de::Error| SkinError::Toml(e.to_string()))?;
    let mut warnings = Vec::new();
    for key in root.keys() {
        if !["skin", "font", "color", "playlist"].contains(&key.as_str()) {
            warnings.push(format!("unknown section [{key}]"));
        }
    }
    let mut skin = base.clone();
    skin.id = id.to_owned();
    skin.origin = SkinOrigin::BuiltIn;

    let section_skin = section(&root, "skin")?;
    if let Some(table) = section_skin {
        warn_unknown(table, "skin", &["name", "format"], &mut warnings);
    }
    let name = section_skin.and_then(|t| t.get("name"));
    skin.name = match name {
        Some(value) => string_of("skin.name", value)?,
        None => return Err(SkinError::MissingName),
    };
    if let Some(value) = section_skin.and_then(|t| t.get("format")) {
        let format = value
            .as_integer()
            .ok_or_else(|| wrong_type("skin.format", "an integer"))?;
        if format != 1 {
            return Err(SkinError::UnsupportedFormat(
                u32::try_from(format).unwrap_or(u32::MAX),
            ));
        }
    }
    skin.format = 1;

    let font = section(&root, "font")?;
    if let Some(table) = font {
        warn_unknown(table, "font", &["ui", "mono", "lcd"], &mut warnings);
    }
    for (key, slot) in [
        ("ui", &mut skin.fonts.ui),
        ("mono", &mut skin.fonts.mono),
        ("lcd", &mut skin.fonts.lcd),
    ] {
        match font.and_then(|t| t.get(key)) {
            Some(value) => *slot = string_of(&format!("font.{key}"), value)?,
            None => warnings.push(format!("missing `font.{key}`, using `{slot}`")),
        }
    }

    let color = section(&root, "color")?;
    let mut known: Vec<&str> = vec!["frame_brush"];
    macro_rules! tokens {
        ($($field:ident),* $(,)?) => {$(
            known.push(stringify!($field));
            read_color(color, "color", stringify!($field), &mut skin.colors.$field, &mut warnings)?;
        )*};
    }
    tokens!(
        frame_hi,
        frame_mid,
        frame_lo,
        frame_edge,
        bevel_hi,
        groove,
        text,
        text_muted,
        control_hi,
        control_lo,
        control_hover_hi,
        control_hover_lo,
        slot,
        field_bg,
        focus,
        lcd_top,
        lcd_bottom,
        lcd_edge,
        lcd_fg,
        lcd_dim,
        lcd_glow,
        lcd_rule,
        vis_bar,
        vis_osc,
        vis_peak,
        accent,
        accent_ink,
        led_off,
    );
    match color.and_then(|t| t.get("frame_brush")) {
        Some(value) => {
            let brush = match value {
                Value::Float(f) => *f,
                Value::Integer(i) => *i as f64,
                _ => return Err(wrong_type("color.frame_brush", "a number")),
            };
            if !(0.0..=0.2).contains(&brush) {
                return Err(SkinError::OutOfRange {
                    key: "color.frame_brush".to_owned(),
                });
            }
            skin.colors.frame_brush = brush as f32;
        }
        None => warnings.push("missing `color.frame_brush`, using the Silver value".to_owned()),
    }
    if let Some(table) = color {
        warn_unknown(table, "color", &known, &mut warnings);
    }

    let playlist = section(&root, "playlist")?;
    macro_rules! pl_tokens {
        ($($field:ident),* $(,)?) => {
            let mut pl_known: Vec<&str> = Vec::new();
            $(
                pl_known.push(stringify!($field));
                read_color(
                    playlist, "playlist", stringify!($field), &mut skin.playlist.$field,
                    &mut warnings,
                )?;
            )*
            if let Some(table) = playlist {
                warn_unknown(table, "playlist", &pl_known, &mut warnings);
            }
        };
    }
    pl_tokens!(bg, fg, current, selected_bg, index);

    Ok(ParsedSkin { skin, warnings })
}

fn section<'a>(root: &'a Table, name: &str) -> Result<Option<&'a Table>, SkinError> {
    match root.get(name) {
        None => Ok(None),
        Some(Value::Table(table)) => Ok(Some(table)),
        Some(_) => Err(wrong_type(name, "a table")),
    }
}

fn wrong_type(key: &str, expected: &str) -> SkinError {
    SkinError::Toml(format!("`{key}` must be {expected}"))
}

fn string_of(key: &str, value: &Value) -> Result<String, SkinError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| wrong_type(key, "a string"))
}

fn warn_unknown(table: &Table, section: &str, known: &[&str], warnings: &mut Vec<String>) {
    for key in table.keys() {
        if !known.contains(&key.as_str()) {
            warnings.push(format!("unknown key `{section}.{key}`"));
        }
    }
}

fn read_color(
    table: Option<&Table>,
    section: &str,
    key: &str,
    slot: &mut Color,
    warnings: &mut Vec<String>,
) -> Result<(), SkinError> {
    let full = format!("{section}.{key}");
    match table.and_then(|t| t.get(key)) {
        Some(value) => {
            let text = string_of(&full, value)?;
            *slot = Color::parse(&text).ok_or(SkinError::InvalidColor {
                key: full,
                value: text,
            })?;
        }
        None => warnings.push(format!("missing `{full}`, using the Silver value {slot}")),
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::builtin::SILVER_TOML;

    use super::*;

    fn silver_without(line_start: &str) -> String {
        SILVER_TOML
            .lines()
            .filter(|l| !l.starts_with(line_start))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn missing_token_warns_and_falls_back() {
        let parsed = parse_skin("x", &silver_without("accent = ")).unwrap();
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].contains("color.accent"));
        assert_eq!(parsed.skin.colors.accent, Color::parse("#36b4ff").unwrap());
        assert_eq!(parsed.skin.id, "x");
    }

    #[test]
    fn minimal_skin_loads_with_many_warnings() {
        let parsed = parse_skin("min", "[skin]\nname = \"Min\"\n").unwrap();
        assert_eq!(parsed.skin.name, "Min");
        assert!(parsed.warnings.len() > 30);
    }

    #[test]
    fn unknown_keys_warn() {
        let text = format!("{SILVER_TOML}\n[extra]\na = 1\n").replace("[font]", "[font]\nbig = 1");
        let parsed = parse_skin("x", &text).unwrap();
        assert!(parsed.warnings.iter().any(|w| w.contains("[extra]")));
        assert!(parsed.warnings.iter().any(|w| w.contains("font.big")));
    }

    #[test]
    fn bad_color_names_the_key() {
        let text = SILVER_TOML.replace("#36b4ff", "blue");
        match parse_skin("x", &text) {
            Err(SkinError::InvalidColor { key, value }) => {
                assert_eq!(key, "color.accent");
                assert_eq!(value, "blue");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wrong_type_is_an_error() {
        let text = SILVER_TOML.replace("accent = \"#36b4ff\"", "accent = 5");
        let err = parse_skin("x", &text).unwrap_err();
        assert!(err.to_string().contains("color.accent"), "{err}");
    }

    #[test]
    fn format_and_name() {
        let text = SILVER_TOML.replace("format = 1", "format = 2");
        assert!(matches!(
            parse_skin("x", &text),
            Err(SkinError::UnsupportedFormat(2))
        ));
        assert!(matches!(
            parse_skin("x", &silver_without("name = ")),
            Err(SkinError::MissingName)
        ));
    }

    #[test]
    fn frame_brush_range() {
        let text = SILVER_TOML.replace("frame_brush = 0.05", "frame_brush = 0.3");
        assert!(matches!(
            parse_skin("x", &text),
            Err(SkinError::OutOfRange { .. })
        ));
        let text = SILVER_TOML.replace("frame_brush = 0.05", "frame_brush = 0");
        assert_eq!(parse_skin("x", &text).unwrap().skin.colors.frame_brush, 0.0);
    }

    #[test]
    fn invalid_toml() {
        assert!(matches!(parse_skin("x", "[skin"), Err(SkinError::Toml(_))));
    }
}
