//! The views. The main window (`window.rs`) is skinned: the player, the equalizer and the
//! playlist, built from `components/`. Preferences and About are app chrome windows in the fixed
//! palette of `theme::palette`.

pub mod about;
pub mod components;
pub mod display;
pub mod dock;
pub mod equalizer;
pub mod menus;
pub mod player;
pub mod playlist;
pub mod preferences;
pub mod window;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Window};

use crate::app::{models, persist};
use crate::state::settings::ThemeChoice;

/// The user guide, opened by Help, User guide and F1. Provisional, like the website itself.
pub const USER_GUIDE_URL: &str = "https://nastro.tanis.codes/docs/";

/// Saves everything and quits.
pub fn quit(cx: &mut App) {
    log::info!("quitting");
    persist::save_all(cx);
    cx.quit();
}

/// Opens the user skins folder in the file manager, creating it first.
pub fn open_skins_folder(cx: &mut App) {
    let Some(dir) = crate::state::session::user_skins_dir() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        log::warn!("could not create {}: {error}", dir.display());
        return;
    }
    cx.open_with_system(&dir);
}

/// Applies the chrome theme choice (System, Light, Dark) to every open window.
pub fn apply_chrome_theme(cx: &mut App) {
    for window in cx.windows() {
        let _ = window.update(cx, |_, window, cx| apply_chrome_theme_to(window, cx));
    }
}

/// Applies the chrome theme choice to `window`.
pub fn apply_chrome_theme_to(window: &mut Window, cx: &mut App) {
    match models(cx).shell.read(cx).settings.theme {
        ThemeChoice::System => Theme::sync_system_appearance(Some(window), cx),
        ThemeChoice::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        ThemeChoice::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}

/// Re-applies strings cached outside `render` in the main window after a language change.
pub fn relocalize(cx: &mut App) {
    for window in cx.windows() {
        if let Some(view) = window.downcast::<window::NastroWindow>() {
            let _ = view.update(cx, |view, window, cx| view.relocalize(window, cx));
        }
    }
}
