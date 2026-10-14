//! The asset source of the app: the skin icons of `assets/icons/` under the `nastro/` prefix,
//! and gpui-kit's curated icon set for the stock widgets the chrome borrows (menu checks,
//! chevrons, select arrows). Without gpui-kit's set, those widgets render their icons as empty
//! space (`docs/gpui-notes.md`, "Icons").

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

/// A skin icon. Its path is what `svg().path(..)` takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinIcon {
    Previous,
    Play,
    Pause,
    Stop,
    Next,
    Open,
    Volume,
    Search,
    Plus,
    Chevron,
    Minimize,
    Shade,
    Close,
    Logo,
    StatePlay,
    StateStop,
    Grip,
}

impl SkinIcon {
    /// The asset path of the icon.
    pub fn path(self) -> &'static str {
        match self {
            SkinIcon::Previous => "nastro/previous.svg",
            SkinIcon::Play => "nastro/play.svg",
            SkinIcon::Pause => "nastro/pause.svg",
            SkinIcon::Stop => "nastro/stop.svg",
            SkinIcon::Next => "nastro/next.svg",
            SkinIcon::Open => "nastro/open.svg",
            SkinIcon::Volume => "nastro/volume.svg",
            SkinIcon::Search => "nastro/search.svg",
            SkinIcon::Plus => "nastro/plus.svg",
            SkinIcon::Chevron => "nastro/chevron.svg",
            SkinIcon::Minimize => "nastro/minimize.svg",
            SkinIcon::Shade => "nastro/shade.svg",
            SkinIcon::Close => "nastro/close.svg",
            SkinIcon::Logo => "nastro/logo.svg",
            SkinIcon::StatePlay => "nastro/state-play.svg",
            SkinIcon::StateStop => "nastro/state-stop.svg",
            SkinIcon::Grip => "nastro/grip.svg",
        }
    }
}

/// The app icon, for the About window.
pub const APP_ICON: &str = "nastro/app-icon.png";

/// Every skin icon, by file name.
const ICONS: &[(&str, &[u8])] = &[
    (
        "previous.svg",
        include_bytes!("../assets/icons/previous.svg"),
    ),
    ("play.svg", include_bytes!("../assets/icons/play.svg")),
    ("pause.svg", include_bytes!("../assets/icons/pause.svg")),
    ("stop.svg", include_bytes!("../assets/icons/stop.svg")),
    ("next.svg", include_bytes!("../assets/icons/next.svg")),
    ("open.svg", include_bytes!("../assets/icons/open.svg")),
    ("volume.svg", include_bytes!("../assets/icons/volume.svg")),
    ("search.svg", include_bytes!("../assets/icons/search.svg")),
    ("plus.svg", include_bytes!("../assets/icons/plus.svg")),
    ("chevron.svg", include_bytes!("../assets/icons/chevron.svg")),
    (
        "minimize.svg",
        include_bytes!("../assets/icons/minimize.svg"),
    ),
    ("shade.svg", include_bytes!("../assets/icons/shade.svg")),
    ("close.svg", include_bytes!("../assets/icons/close.svg")),
    ("logo.svg", include_bytes!("../assets/icons/logo.svg")),
    (
        "state-play.svg",
        include_bytes!("../assets/icons/state-play.svg"),
    ),
    (
        "state-stop.svg",
        include_bytes!("../assets/icons/state-stop.svg"),
    ),
    ("grip.svg", include_bytes!("../assets/icons/grip.svg")),
    (
        "app-icon.png",
        include_bytes!("../../../packaging/icons/png/nastro-256.png"),
    ),
];

/// The app's asset source.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path.strip_prefix("nastro/") {
            return Ok(ICONS
                .iter()
                .find(|(file, _)| *file == name)
                .map(|(_, bytes)| Cow::Borrowed(*bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut found = gpui_kit::assets::Assets.list(path)?;
        found.extend(
            ICONS
                .iter()
                .map(|(file, _)| format!("nastro/{file}"))
                .filter(|full| full.starts_with(path))
                .map(SharedString::from),
        );
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_path_loads() {
        let icons = [
            SkinIcon::Previous,
            SkinIcon::Play,
            SkinIcon::Pause,
            SkinIcon::Stop,
            SkinIcon::Next,
            SkinIcon::Open,
            SkinIcon::Volume,
            SkinIcon::Search,
            SkinIcon::Plus,
            SkinIcon::Chevron,
            SkinIcon::Minimize,
            SkinIcon::Shade,
            SkinIcon::Close,
            SkinIcon::Logo,
            SkinIcon::StatePlay,
            SkinIcon::StateStop,
            SkinIcon::Grip,
        ];
        for icon in icons {
            let loaded = Assets.load(icon.path()).ok().flatten();
            assert!(loaded.is_some(), "{}", icon.path());
        }
    }
}
