//! The shell: the settings, the skins on offer, and where the panels are. A gpui entity; the
//! root view observes it to resize the window and to set its input region whenever the panels
//! move, appear, disappear or change size.

use gpui_kit::{Bounds, Context, Pixels, Size, point, px, size};
use nastro_dock::{Layout, LayoutPreset, Metrics, PanelId};
use nastro_skin::Skin;

use crate::state::locale;
use crate::state::session;
use crate::state::settings::{
    LogLevel, Settings, SkinScale, ThemeChoice, TimeDisplay, VisualizerMode, save_settings,
};
use crate::theme::metrics::{PLAYLIST_CHROME_HEIGHT, WINDOW_MARGIN};
use crate::theme::{SkinTheme, skin};

/// The window height a menu needs, in logical pixels: the equalizer presets menu, the longest,
/// plus room for its shadow. Menus are app chrome, so it does not follow the skin scale.
const MENU_ROOM: f32 = 600.0;

/// The shell entity.
pub struct Shell {
    /// The user settings, saved on every change.
    pub settings: Settings,
    /// Where the panels are, in 1x units.
    pub layout: Layout,
    /// Every skin on offer: the built-ins first, then the user's own.
    pub skins: Vec<Skin>,
    /// Whether a menu is open. The whole window then takes input, since a menu can extend past
    /// the panels into the window's transparent gaps.
    pub menu_open: bool,
}

/// The docking metrics at 1x, with the playlist's chrome height of this design.
pub fn dock_metrics() -> Metrics {
    Metrics {
        playlist_chrome_height: PLAYLIST_CHROME_HEIGHT,
        ..Metrics::default()
    }
}

/// The skins on offer: the four built-ins, then the user's valid skins. Problems with user skins
/// are logged.
pub fn load_skins() -> Vec<Skin> {
    let mut skins = nastro_skin::builtin_skins();
    if let Some(dir) = session::user_skins_dir() {
        for result in nastro_skin::load_user_skins(&dir) {
            match result {
                Ok(parsed) => {
                    for warning in &parsed.warnings {
                        log::warn!("skin {}: {warning}", parsed.skin.id);
                    }
                    skins.push(parsed.skin);
                }
                Err((path, error)) => log::warn!("skin {} not loaded: {error}", path.display()),
            }
        }
    }
    skins
}

/// The skin with `id` among `skins`, or the default skin.
pub fn find_skin(skins: &[Skin], id: &str) -> Skin {
    skins
        .iter()
        .find(|skin| skin.id == id)
        .cloned()
        .unwrap_or_else(|| {
            log::warn!("unknown skin {id:?}, using the default one");
            nastro_skin::default_skin()
        })
}

impl Shell {
    /// The shell of the saved settings and layout.
    pub fn new(settings: Settings, layout: Option<Layout>, skins: Vec<Skin>) -> Self {
        let layout = layout.unwrap_or_else(|| session::default_layout(&dock_metrics()));
        Shell {
            settings,
            layout,
            skins,
            menu_open: false,
        }
    }

    /// The skin scale factor.
    pub fn scale(&self) -> f32 {
        self.settings.skin_scale.factor()
    }

    /// The room around the panels inside the window, scaled.
    pub fn margin(&self) -> f32 {
        WINDOW_MARGIN * self.scale()
    }

    /// Where panel `id` is drawn in the window, `None` when hidden.
    pub fn panel_bounds(&self, id: PanelId) -> Option<Bounds<Pixels>> {
        let rect = self.layout.displayed_rect(id, &dock_metrics())?;
        let scale = self.scale();
        let margin = self.margin();
        Some(Bounds {
            origin: point(px(rect.x * scale + margin), px(rect.y * scale + margin)),
            size: size(px(rect.w * scale), px(rect.h * scale)),
        })
    }

    /// The window size that holds every displayed panel plus the margin.
    pub fn window_size(&self) -> Size<Pixels> {
        let bounds = self.layout.bounds(&dock_metrics());
        let scale = self.scale();
        let margin = self.margin();
        // Even with every panel hidden (impossible from the UI: the player cannot be hidden),
        // keep a size the platform accepts.
        let width = (bounds.right() * scale).max(1.0) + 2.0 * margin;
        let mut height = (bounds.bottom() * scale).max(1.0) + 2.0 * margin;
        // A menu is drawn inside the window: while one is open the window grows downwards, with
        // the extra room transparent, so the longest menu fits under a short layout.
        if self.menu_open {
            height = height.max(MENU_ROOM);
        }
        size(px(width.round()), px(height.round()))
    }

    /// Whether panel `id` is shown.
    pub fn is_visible(&self, id: PanelId) -> bool {
        self.layout.state(id).visible
    }

    /// Whether panel `id` is collapsed to its bar.
    pub fn is_shaded(&self, id: PanelId) -> bool {
        self.layout.state(id).shaded
    }

    /// The full height of the equalizer in 1x units, which grows in row layouts.
    pub fn equalizer_height(&self) -> f32 {
        self.layout.state(PanelId::Equalizer).rect.h
    }

    /// Shows or hides panel `id`. The player is always shown.
    pub fn toggle_visible(&mut self, id: PanelId, cx: &mut Context<Self>) {
        if id == PanelId::Player {
            return;
        }
        let visible = !self.layout.state(id).visible;
        self.layout.set_visible(id, visible);
        self.layout_changed(cx);
    }

    /// Collapses or expands panel `id`.
    pub fn toggle_shade(&mut self, id: PanelId, cx: &mut Context<Self>) {
        let shaded = !self.layout.state(id).shaded;
        self.layout.set_shaded(id, shaded, &dock_metrics());
        self.layout_changed(cx);
    }

    /// Arranges the panels with `preset`.
    pub fn apply_preset(&mut self, preset: LayoutPreset, cx: &mut Context<Self>) {
        self.layout = Layout::preset(preset, &dock_metrics());
        self.layout_changed(cx);
    }

    /// Moves panel `id` to `(x, y)` in 1x units, snapping it to the others.
    pub fn move_panel(&mut self, id: PanelId, x: f32, y: f32, cx: &mut Context<Self>) {
        let before = self.layout.state(id).rect;
        let after = self.layout.move_panel(id, x, y, &dock_metrics());
        if before != after {
            self.layout_changed(cx);
        }
    }

    /// Resizes the playlist to `width` x `height` in 1x units.
    pub fn resize_playlist(&mut self, width: f32, height: f32, cx: &mut Context<Self>) {
        let before = self.layout.state(PanelId::Playlist).rect;
        self.layout.resize_playlist(width, height, &dock_metrics());
        if self.layout.state(PanelId::Playlist).rect != before {
            self.layout_changed(cx);
        }
    }

    /// Remembers whether a menu is open.
    pub fn set_menu_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.menu_open != open {
            self.menu_open = open;
            cx.notify();
        }
    }

    /// Schedules a session save and repaints after a layout change.
    fn layout_changed(&mut self, cx: &mut Context<Self>) {
        crate::app::persist::schedule(cx);
        cx.notify();
    }

    /// Makes skin `id` the active skin and remembers it.
    pub fn set_skin(&mut self, id: &str, cx: &mut Context<Self>) {
        let skin = find_skin(&self.skins, id);
        log::info!("skin {} ({})", skin.id, skin.name);
        self.settings.skin = skin.id.clone();
        skin::set_active(cx, SkinTheme::new(&skin, self.scale()));
        self.settings_changed(cx);
        cx.refresh_windows();
    }

    /// Reads the skins folder again, for skins added while running.
    pub fn reload_skins(&mut self, cx: &mut Context<Self>) {
        self.skins = load_skins();
        let id = self.settings.skin.clone();
        self.set_skin(&id, cx);
    }

    /// Changes the skin scale.
    pub fn set_scale(&mut self, scale: SkinScale, cx: &mut Context<Self>) {
        if self.settings.skin_scale == scale {
            return;
        }
        self.settings.skin_scale = scale;
        let skin = find_skin(&self.skins, &self.settings.skin);
        skin::set_active(cx, SkinTheme::new(&skin, scale.factor()));
        self.settings_changed(cx);
        cx.refresh_windows();
    }

    /// Switches the clock between elapsed and remaining.
    pub fn toggle_time_display(&mut self, cx: &mut Context<Self>) {
        self.settings.time_display = self.settings.time_display.toggled();
        self.settings_changed(cx);
    }

    /// Sets what the clock shows.
    pub fn set_time_display(&mut self, mode: TimeDisplay, cx: &mut Context<Self>) {
        self.settings.time_display = mode;
        self.settings_changed(cx);
    }

    /// Sets what the visualizer draws.
    pub fn set_visualizer(&mut self, mode: VisualizerMode, cx: &mut Context<Self>) {
        self.settings.visualizer = mode;
        self.settings_changed(cx);
    }

    /// Sets the UI language, live.
    pub fn set_language(&mut self, choice: locale::LanguageChoice, cx: &mut Context<Self>) {
        self.settings.language = choice;
        let language = locale::resolve(choice);
        locale::apply(language);
        log::info!("language: {}", language.code());
        self.settings_changed(cx);
        cx.defer(crate::views::relocalize);
        cx.refresh_windows();
    }

    /// Sets light or dark for the chrome.
    pub fn set_theme(&mut self, theme: ThemeChoice, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        self.settings_changed(cx);
        cx.defer(crate::views::apply_chrome_theme);
    }

    /// Sets the log level, live.
    pub fn set_log_level(&mut self, level: LogLevel, cx: &mut Context<Self>) {
        self.settings.log_level = level;
        crate::logging::set_level(level.to_filter());
        self.settings_changed(cx);
    }

    /// Saves the settings and repaints.
    fn settings_changed(&mut self, cx: &mut Context<Self>) {
        save_settings(&self.settings);
        cx.notify();
    }
}
