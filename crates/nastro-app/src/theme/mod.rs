//! Nastro's visual foundations: the bundled fonts, the app chrome palette (menus, Preferences,
//! About) with the gpui-kit theme family built from it, the skin metrics, and the active skin.
//!
//! [`install`] does the startup work: register the fonts, load the "Nastro Light" and "Nastro
//! Dark" chrome themes into gpui-kit's `ThemeRegistry` (after `gpui_kit::init`, see
//! `docs/gpui-notes.md`, "Themes") and make the chosen skin active.

pub mod metrics;
pub mod palette;
pub mod skin;

pub use palette::PaletteExt;
pub use skin::{SkinExt, SkinTheme};

use std::borrow::Cow;

use gpui_kit::App;
use gpui_kit::component::{Theme, ThemeRegistry};

/// Name of the light chrome theme in gpui-kit's registry.
const LIGHT_THEME_NAME: &str = "Nastro Light";
/// Name of the dark chrome theme in gpui-kit's registry.
const DARK_THEME_NAME: &str = "Nastro Dark";
/// The chrome's font, the same as Postino's.
pub const CHROME_FONT: &str = "Geist";
/// The chrome's monospace font.
pub const CHROME_MONO_FONT: &str = "Geist Mono";
/// Base size of the chrome's text.
const CHROME_FONT_SIZE: f32 = 13.0;
/// General radius of gpui-kit's widgets in the chrome.
const GENERAL_RADIUS: u64 = 8;
/// Radius of large chrome elements.
const LARGE_RADIUS: u64 = 14;

/// Registers the fonts, loads the chrome theme family and makes `skin` the active skin. Call once,
/// after `gpui_kit::init`, before opening any window.
pub fn install(cx: &mut App, skin: SkinTheme) {
    register_fonts(cx);

    let json = theme_family_json().to_string();
    // Built here from a fixed shape covered by the tests below, so a failure would be a bug in
    // this module, not a runtime condition: one of AGENTS.md's "truly impossible state" cases.
    #[allow(clippy::expect_used)]
    ThemeRegistry::global_mut(cx)
        .load_themes_from_str(&json)
        .expect("theme_family_json always builds a valid ThemeSet");
    let registry = ThemeRegistry::global(cx);
    let light = registry.themes().get(LIGHT_THEME_NAME).cloned();
    let dark = registry.themes().get(DARK_THEME_NAME).cloned();
    let theme = Theme::global_mut(cx);
    if let Some(light) = light {
        theme.light_theme = light;
    }
    if let Some(dark) = dark {
        theme.dark_theme = dark;
    }

    skin::set_active(cx, skin);
}

/// Registers the bundled Geist, Geist Mono and Share Tech Mono fonts. A failure only means the
/// OS fallback fonts are used, which is not worth aborting over.
fn register_fonts(cx: &App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../../assets/fonts/Geist-Regular.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Geist-Medium.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Geist-SemiBold.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/Geist-Bold.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/GeistMono-Regular.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/GeistMono-Medium.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/GeistMono-SemiBold.ttf").as_slice()),
        Cow::Borrowed(include_bytes!("../../assets/fonts/ShareTechMono-Regular.ttf").as_slice()),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        log::warn!("could not register the bundled fonts: {error}");
    }
}

/// The chrome theme family JSON, both modes, built from [`palette`]'s tokens.
fn theme_family_json() -> serde_json::Value {
    serde_json::json!({
        "name": "Nastro",
        "themes": [
            theme_config_json(LIGHT_THEME_NAME, "light", false),
            theme_config_json(DARK_THEME_NAME, "dark", true),
        ],
    })
}

/// One mode's `ThemeConfig`.
fn theme_config_json(name: &str, mode: &str, dark: bool) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "mode": mode,
        "font.family": CHROME_FONT,
        "font.size": CHROME_FONT_SIZE,
        "mono_font.family": CHROME_MONO_FONT,
        "mono_font.size": CHROME_FONT_SIZE - 1.0,
        "radius": GENERAL_RADIUS,
        "radius.lg": LARGE_RADIUS,
        "colors": palette::theme_colors(dark),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use gpui_kit::component::ThemeSet;

    #[test]
    fn family_json_parses_as_a_theme_set_with_both_modes() {
        let json = theme_family_json().to_string();
        let set: ThemeSet = serde_json::from_str(&json).expect("a valid ThemeSet");
        assert_eq!(set.themes.len(), 2);
        assert_eq!(set.themes[0].name.as_ref(), LIGHT_THEME_NAME);
        assert!(set.themes[1].mode.is_dark());
    }
}
