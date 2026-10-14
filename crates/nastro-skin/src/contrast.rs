//! Contrast validation of a skin against the design rules.

use crate::color::{Color, contrast_ratio};
use crate::skin::Skin;

/// A color pair below its minimum contrast.
#[derive(Clone, Debug, PartialEq)]
pub struct ContrastIssue {
    /// Human readable pair, like `text on frame_hi`.
    pub pair: &'static str,
    /// The measured WCAG ratio.
    pub ratio: f32,
    /// The required minimum.
    pub minimum: f32,
}

/// Checks the pairs the design rules require: text on metal and on controls and the LCD and
/// playlist text at 4.5:1, icons and focus at 3:1. Translucent colors are composited over their
/// background first. Returns the pairs that fail, empty for a conforming skin.
pub fn contrast_issues(skin: &Skin) -> Vec<ContrastIssue> {
    let c = &skin.colors;
    let p = &skin.playlist;
    let pairs: [(&'static str, Color, Color, f32); 15] = [
        ("text on frame_hi", c.text, c.frame_hi, 4.5),
        ("text on frame_mid", c.text, c.frame_mid, 4.5),
        ("text on frame_lo", c.text, c.frame_lo, 4.5),
        ("text on control_hi", c.text, c.control_hi, 4.5),
        ("text on control_lo", c.text, c.control_lo, 4.5),
        ("text_muted on frame_mid", c.text_muted, c.frame_mid, 4.5),
        ("accent_ink on control_hi", c.accent_ink, c.control_hi, 3.0),
        ("accent_ink on control_lo", c.accent_ink, c.control_lo, 3.0),
        ("focus on frame_mid", c.focus, c.frame_mid, 3.0),
        ("lcd_fg on lcd_top", c.lcd_fg, c.lcd_top, 4.5),
        ("lcd_fg on lcd_bottom", c.lcd_fg, c.lcd_bottom, 4.5),
        ("playlist fg on bg", p.fg, p.bg, 4.5),
        ("playlist current on bg", p.current, p.bg, 3.0),
        (
            "playlist current on selected_bg",
            p.current,
            p.selected_bg,
            3.0,
        ),
        ("playlist fg on selected_bg", p.fg, p.selected_bg, 4.5),
    ];
    pairs
        .into_iter()
        .filter_map(|(pair, fg, bg, minimum)| {
            let bg = bg.over(Color::rgb(255, 255, 255));
            let ratio = contrast_ratio(fg.over(bg), bg);
            (ratio < minimum).then_some(ContrastIssue {
                pair,
                ratio,
                minimum,
            })
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::builtin::{builtin_skins, default_skin};

    use super::*;

    /// Pairs that miss the rule in a built-in skin as designed, kept until the designer decides:
    /// (skin id, pair, measured ratio).
    const KNOWN_EXCEPTIONS: &[(&str, &str, f32)] = &[
        ("silver", "playlist fg on selected_bg", 4.186),
        ("graphite", "text on control_hi", 4.437),
        ("graphite", "text_muted on frame_mid", 4.390),
    ];

    #[test]
    fn builtins_have_no_unlisted_issues() {
        for skin in builtin_skins() {
            for issue in contrast_issues(&skin) {
                let known = KNOWN_EXCEPTIONS.iter().any(|(id, pair, ratio)| {
                    *id == skin.id && *pair == issue.pair && (issue.ratio - ratio).abs() < 0.01
                });
                assert!(
                    known,
                    "{}: {} is {:.2}, needs {}",
                    skin.id, issue.pair, issue.ratio, issue.minimum
                );
            }
        }
    }

    #[test]
    fn a_bad_skin_is_flagged() {
        let mut skin = default_skin();
        skin.colors.text = skin.colors.frame_hi;
        let issues = contrast_issues(&skin);
        assert!(
            issues
                .iter()
                .any(|i| i.pair == "text on frame_hi" && i.ratio < 1.01)
        );
    }
}
