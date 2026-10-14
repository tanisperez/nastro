//! An RGBA color and the WCAG contrast math.

use std::fmt;

use serde::{Deserialize, Deserializer};

/// An 8 bit per channel sRGB color with alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha, 255 is opaque.
    pub a: u8,
}

impl Color {
    /// An opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`, case-insensitive. `None` if malformed.
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if !digits.is_ascii() {
            return None;
        }
        let nibble = |i: usize| u8::from_str_radix(digits.get(i..=i)?, 16).ok();
        let byte = |i: usize| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok();
        match digits.len() {
            3 => Some(Self::rgb(nibble(0)? * 17, nibble(1)? * 17, nibble(2)? * 17)),
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }

    /// Lowercase hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// The same color with alpha set to `alpha` (0.0 to 1.0, clamped).
    pub fn with_alpha(self, alpha: f32) -> Self {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self { a, ..self }
    }

    /// Composites this color over `background`, giving an opaque color.
    pub fn over(self, background: Color) -> Color {
        let alpha = f32::from(self.a) / 255.0;
        let mix = |fg: u8, bg: u8| {
            (f32::from(fg) * alpha + f32::from(bg) * (1.0 - alpha))
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Color::rgb(
            mix(self.r, background.r),
            mix(self.g, background.g),
            mix(self.b, background.b),
        )
    }

    /// WCAG 2.x relative luminance of the color channels (alpha is ignored).
    pub fn relative_luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = f64::from(c) / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        (0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)) as f32
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Color::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid color `{text}`")))
    }
}

/// WCAG 2.x contrast ratio between two colors, from 1.0 to 21.0. Alpha is ignored, so composite
/// translucent colors with [`Color::over`] first.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (
        f64::from(a.relative_luminance()),
        f64::from(b.relative_luminance()),
    );
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    ((hi + 0.05) / (lo + 0.05)) as f32
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn parses_all_forms() {
        assert_eq!(Color::parse("#fa0"), Some(Color::rgb(0xff, 0xaa, 0x00)));
        assert_eq!(Color::parse("#A6E6FF"), Some(Color::rgb(0xa6, 0xe6, 0xff)));
        assert_eq!(
            Color::parse("#78d2ff73"),
            Some(Color {
                r: 0x78,
                g: 0xd2,
                b: 0xff,
                a: 0x73
            })
        );
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "", "#", "fff", "#ff", "#ffff", "#fffff", "#ggg", "#1234567", "#é12", "#+12",
        ] {
            assert_eq!(Color::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(Color::parse("#fa0").unwrap().to_hex(), "#ffaa00");
        assert_eq!(Color::parse("#78d2ff73").unwrap().to_hex(), "#78d2ff73");
    }

    #[test]
    fn alpha_and_compositing() {
        let white = Color::rgb(255, 255, 255);
        assert_eq!(white.with_alpha(0.45).a, 115);
        assert_eq!(white.with_alpha(2.0).a, 255);
        let black = Color::rgb(0, 0, 0);
        assert_eq!(white.with_alpha(0.5).over(black), Color::rgb(128, 128, 128));
        assert_eq!(white.over(black), white);
    }

    #[test]
    fn wcag_extremes() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 0.001);
        // #777 on white is the textbook 4.48:1.
        let grey = Color::parse("#777777").unwrap();
        assert!((contrast_ratio(grey, white) - 4.48).abs() < 0.01);
    }

    #[test]
    fn deserializes_from_string() {
        #[derive(Deserialize)]
        struct W {
            c: Color,
        }
        let w: W = toml::from_str("c = \"#fff\"").unwrap();
        assert_eq!(w.c, Color::rgb(255, 255, 255));
        assert!(toml::from_str::<W>("c = \"nope\"").is_err());
    }
}
